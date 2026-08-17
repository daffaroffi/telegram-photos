# Arsitektur Teknis

Dokumen ini menjelaskan arsitektur Telegram Photos: bagaimana backend Rust,
plugin Android Kotlin, dan frontend React saling terhubung, beserta alur data
untuk setiap fitur inti.

---

## 1. Gambaran Umum

```
┌───────────────────────────── Android / Desktop ─────────────────────────────┐
│                                                                             │
│  ┌──────────────────────┐        ┌────────────────────────────────────────┐ │
│  │  React (WebView)     │ invoke │  Rust backend (Tauri command)          │ │
│  │  - Galeri grid       │───────▶│  - telegram/  (MTProto Grammers)       │ │
│  │  - Backup UI         │  event │  - db.rs      (SQLite)                 │ │
│  │  - Google import     │◀───────│  - crypto.rs  (XChaCha20 + Argon2id)   │ │
│  │  - Pengaturan        │        │  - backup.rs  (state machine)          │ │
│  └──────────────────────┘        │  - google.rs  (OAuth2 + API Photos)    │ │
│                                  │  - media.rs   (EXIF/thumbnail/hash)    │ │
│                                  │  - geo.rs     (reverse geocode)        │ │
│                                  └──────────────┬─────────────────────────┘ │
│                                                 │ JNI                       │
│  ┌──────────────────────┐        ┌──────────────▼─────────────────────────┐ │
│  │ WorkManager Worker   │ JNI    │  Kotlin (MediaPlugin)                  │ │
│  │ (BackgroundWorker)   │───────▶│  - MediaStore scan                    │ │
│  └──────────────────────┘        │  - ContentObserver                    │ │
│                                  │  - constraints Wi-Fi/charging         │ │
│                                  │  - notifikasi progres                 │ │
│                                  └────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Prinsip kunci**: semua logika bisnis (upload, enkripsi, database, impor)
berada di Rust dan dipakai bersama oleh UI (via Tauri command) dan background
worker (via JNI). Tidak ada duplikasi logika.

---

## 2. Modul Backend Rust (`app/src-tauri/src`)

### 2.1 `telegram/mod.rs` — Koneksi & autentikasi MTProto

- `ensure_client_initialized` / `ensure_client_initialized_with_dir`:
  membangun client Grammers dengan `SenderPool`, sesi SQLite
  (`telegram.session`), dan runner tokio ter-spawn. Versi `_with_dir`
  dipakai background worker (tanpa `AppHandle`).
- Alur login: `request_login_code` (OTP) → `sign_in` → jika
  `PasswordRequired` maka `check_password` (2FA). Login QR via
  `auth.exportLoginToken` + polling `is_authorized`.
- Semua kesalahan Telegram dipetakan ke pesan ramah pengguna
  (`PHONE_NUMBER_BANNED`, `FLOOD_WAIT`, `API_ID_INVALID`, …).

### 2.2 `telegram/vault.rs` — Channel penyimpanan

- `get_or_create_vault`: mencari channel `TelegramPhotos_Vault` di dialog
  (cache `vault_peer`), atau membuatnya privat jika belum ada.
- `peer_from_vault`: mengembalikan `Peer` channel (dari cache state, atau
  scan dialog sebagai fallback).

### 2.3 `telegram/upload.rs` — Upload chunked

- Loop part manual (`messages.saveBigFilePart`) untuk kontrol penuh:
  - Progress callback per part.
  - `FLOOD_WAIT_X` → sleep `X + 2` detik lalu lanjut otomatis.
  - Pembatalan via `AtomicBool` (dibagikan ke UI & worker).
- `InputMedia::UploadedDocument` dengan atribut (nama file, MIME, dimensi,
  durasi video). `message_id` dikembalikan dan disimpan di database.
- `download_message_to_path` untuk fitur restore.

### 2.4 `db.rs` — SQLite

- Skema mengikuti PRD §5 (`media_items`, `albums`, `album_media`,
  `google_import_sessions`, `settings`, `vault_meta`).
- Memakai crate `sqlite` (bundled) — sengaja dipilih karena kompatibel
  dengan `grammers-session` (menghindari dua implementasi SQLite).
- WAL diaktifkan untuk performa tulis saat banyak file.

### 2.5 `crypto.rs` — Enkripsi zero-knowledge

- `derive_key`: Argon2id (64 MiB, t=3, p=1) dari passphrase + salt acak.
- `encrypt_file` / `decrypt_file`: XChaCha20-Poly1305 streaming
  (`EncryptorBE32`/`DecryptorBE32`, chunk 4 KB) dengan header
  `TPENC1v1` + nonce 19 byte. MAC diverifikasi saat dekripsi (passphrase
  salah → error verifikasi).
- `VaultState`: kunci hanya di memory; lock = drop kunci.

### 2.6 `media.rs` — Pipeline media lokal

- `sha256_file` — integritas & dedup.
- `extract_exif` — tanggal, GPS, kamera, ISO, aperture, focal length.
- `generate_thumbnails` — WebP mikro (grid) + sedang (preview).
- `encode_blurhash` — placeholder saat thumbnail belum siap.

### 2.7 `backup.rs` — State machine & Free Up Space

- `BackupContext` memisahkan dependensi Tauri dari logika inti:
  - `on_event` callback → UI (emit `backup-progress`) atau worker (notifikasi).
  - `cache_dir`, `cancel`, `db`, `tg_state`, `vault_state`.
- `run_backup_core`:
  1. Cek `auto_backup_enabled` + constraints Wi-Fi/charging (JNI di Android).
  2. Dapatkan client + vault channel.
  3. Iterasi item `NOT_BACKED_UP / QUEUED / FAILED`:
     - Skip folder yang dimatikan di whitelist.
     - Jika enkripsi aktif & vault terkunci → tetap `QUEUED` (aman).
     - Jika tidak ada path lokal → materialisasi dari `content://` MediaStore
       (Android), hapus setelah selesai.
     - Enkripsi opsional ke file temp `.tdenc`.
     - Upload dengan progress; sukses → `BACKED_UP` + simpan `tg_message_id`;
       gagal → `FAILED` + pesan error.
     - Jeda 300–500 ms antar file (anti-FloodWait).
- `cmd_execute_free_up_space`: verifikasi SHA-256 ulang sebelum hapus fisik;
  status → `CLOUD_ONLY`; thumbnail tetap.
- `cmd_restore_media`: download dari vault + dekripsi otomatis bila perlu.

### 2.8 `google.rs` — Importer Google Photos

- OAuth 2.0 loopback (`http://127.0.0.1:18762/callback`) dengan listener TCP
  di background; token + refresh token disimpan di `settings`/DB.
- `get_access_token`: auto-refresh 5 menit sebelum kedaluwarsa.
- Discovery: `mediaItems.list` (paginasi) + `albums.list`.
- Import: untuk setiap item, download `baseUrl + "=d"` → `probe_size` →
  upload ke vault dengan progress, dedup SHA-256 / Google Media ID, metadata
  (tanggal asli, GPS, kamera) dipertahankan, struktur album dipetakan.
- Pasca-import: Library API tidak menyediakan endpoint hapus → item ditandai
  aman (`DELETED_FROM_GOOGLE`) dan UI memberi panduan.

### 2.9 `geo.rs` — Reverse geocoding offline

- Dataset ≈280 kota dunia (Indonesia lengkap, Asia, Eropa, Amerika, dst.)
  di-bundle di binary; nearest-neighbor dalam radius 50 km.
- Tanpa jaringan, tanpa AI. (Pengganti database GeoNames/OSM 15 MB —
  lihat catatan di `PRD_COVERAGE.md`.)

### 2.10 `android_media.rs` — Jembatan JNI

- `scan_gallery` → `MediaPlugin.scanMediaStore` (JSON array).
- `constraints_ok` → `MediaPlugin.checkConstraints` (Wi-Fi/charging nyata).
- `materialize_media` → `MediaPlugin.materializeMedia` (salin `content://`
  ke penyimpanan privat agar Rust bisa baca).
- `register_content_observer` → observer media baru.
- Semua via `with_env` (attach JVM + `ndk_context`).

### 2.11 `bg_worker.rs` — Entry JNI untuk WorkManager

- Export `Java_com_telegramphotos_app_BackgroundWorker_runBackup`:
  1. Parse `data_dir` (filesDir aplikasi).
  2. Tokio runtime → buka DB + sesi + client (fungsi `_with_dir`).
  3. Guard: auto-backup aktif? constraints? sesi ada? API ID ada? ter-authorize?
  4. Jalankan `run_backup_core` dengan callback notifikasi (`notify_progress`,
     `notify_done`) yang attach JVM sendiri (tidak menangkap `JNIEnv`).
- Aman secara desain: vault terkunci → item terenkripsi tidak pernah terkirim
  dari background.

---

## 3. Plugin Kotlin (`app/src-tauri/gen/android/...`)

| File | Tanggung jawab |
|---|---|
| `MediaPlugin.kt` | Static bridge JNI: scan MediaStore (images+video), materialisasi URI, constraints, ContentObserver, channel notifikasi, update notifikasi progres |
| `BackgroundWorker.kt` | `CoroutineWorker` — load library Rust + panggil `runBackup(dataDir)` |
| `BackupScheduler.kt` | Jadwalkan `PeriodicWorkRequest` 15 menit (ExistingPeriodicWorkPolicy.UPDATE) |
| `MainActivity.kt` | Init plugin, minta izin media/notifikasi, daftarkan observer, jadwalkan backup |

Alur: UI/sistem → WorkManager → `BackgroundWorker.doWork()` →
`System.loadLibrary("telegram_photos_lib")` → JNI → Rust `run_backup_core`
→ callback JNI → `MediaPlugin.reportProgress` (notifikasi).

> Catatan nama library: RustPlugin/BuildTask menghasilkan
> `libtelegram_photos_lib.so` (nama sesuai `[lib] name` di `Cargo.toml`).

---

## 4. Frontend React (`app/src`)

- `api.ts` — satu-satunya pintu ke backend: semua `invoke` + listener event
  (`backup-progress`, `google-import-progress`) + helper format.
- `types.ts` — tipe cermin model Rust (camelCase).
- Komponen:
  - `Onboarding.tsx` — wizard kredensial → login → vault.
  - `Gallery.tsx` — grid timeline (1/3/5/8 kolom), sticky month header, date
    scrubber, pencarian, multi-select, sampah, scan galeri/folder.
  - `Lightbox.tsx` — viewer layar penuh (foto/video).
  - `BackupScreen.tsx` — jalankan/batalkan backup, progres live, vault
    (setup/unlock/lock), free up space, restore.
  - `GoogleImport.tsx` — OAuth, discovery, import, cleanup pasca-import.
  - `SettingsScreen.tsx` — kredensial, whitelist folder, grid, tema, logout.
- Thumbnail disajikan lewat `convertFileSrc` (asset protocol, scope cache/data
  aplikasi).

---

## 5. Alur data per fitur

### Backup manual (dari UI)
```
Galeri → pilih → "▲ Backup" (batch queue)  →  tab Backup → "Mulai backup"
  → cmd_run_backup → run_backup_core
  → setiap file: (materialize URI) → (encrypt opsional) → upload MTProto
  → DB sync_status=BACKED_UP, tg_message_id tersimpan
  → event backup-progress → UI progress bar
```

### Auto-backup background
```
MainActivity.onCreate → BackupScheduler.schedule
  → WorkManager periodic 15 menit → BackgroundWorker
  → JNI runBackup(dataDir) → Rust (DB + sesi sama)
  → constraints JNI (Wi-Fi/charging) → run_backup_core
  → notifikasi progres → notifyBackupDone(count)
```

### Import Google Photos
```
Pengaturan → isi Client ID/Secret → "Hubungkan Google"
  → cmd_google_start_oauth (auth URL + listener loopback)
  → buka browser → callback → cmd_google_wait_oauth → token tersimpan
  → "Hitung item" (cmd_google_discover)
  → "Mulai migrasi" → cmd_google_start_import
  → per item: download baseUrl=d → upload ke vault → metadata & album disimpan
  → dialog pasca-import (hapus dari Google / biarkan)
```

---

## 6. Keamanan

| Aspek | Implementasi |
|---|---|
| Kredensial Telegram | Disimpan sebagai sesi MTProto lokal (`telegram.session`), dipakai langsung — tidak ada server perantara |
| Kredensial Google | Client ID/Secret di database lokal; token akses + refresh disimpan, refresh otomatis |
| Enkripsi file | XChaCha20-Poly1305 streaming, kunci Argon2id dari passphrase — hanya di memory |
| Integritas | SHA-256 direkam saat ingest & diverifikasi ulang sebelum Free Up Space |
| Background | Vault terkunci → item terenkripsi tidak pernah di-upload tanpa kunci |
| Permintaan izin | Minimal: media, notifikasi, jaringan |

## 7. Catatan arsitektur & trade-off

1. **WebView (Tauri) dipilih, bukan Flutter/RN**: backend Rust memberi kontrol
   native (MTProto, SQLite, crypto) sambil UI tetap ringan. Performa grid besar
   bergantung pada rendering browser — untuk 100rb+ foto, pertimbangkan
   virtualisasi tambahan (lihat `PRD_COVERAGE.md` §11).
2. **`sqlite` crate, bukan `rusqlite`**: satu-satunya yang kompatibel dengan
   `grammers-session` tanpa konflik dua implementasi SQLite.
3. **Materialisasi URI**: Android scoped storage tidak memberi path fisik untuk
   media milik aplikasi lain; file disalin ke penyimpanan privat hanya saat
   dibutuhkan (thumbnail di scan, file penuh di backup), lalu dibersihkan.
4. **core2 0.4.0 di-vendor** (`app/src-tauri/vendor/`) karena crate tersebut
   di-yank dari crates.io padahal masih dibutuhkan `glass_pumpkin` (grammers).
