# Cakupan Implementasi PRD

Pemetaan setiap bagian `PRD.md` ke implementasi aktual, lengkap dengan lokasi
file dan catatan jujur tentang keterbatasan.

Legenda status:

- ✅ **Nyata** — berfungsi sungguhan sesuai spesifikasi.
- ⚠️ **Sebagian** — inti berfungsi, ada penyederhanaan yang dicatat.
- ❌ **Belum** — tidak diimplementasikan (dengan alasan).

---

## Modul 1 — Import Google Photos (§4.1)

| Persyaratan PRD | Status | Implementasi |
|---|---|---|
| OAuth 2.0 scope `photoslibrary.readonly` | ✅ Nyata | `google.rs` `cmd_google_start_oauth` / `cmd_google_wait_oauth` (loopback 127.0.0.1:18762) |
| Token aman + refresh otomatis | ✅ Nyata | `google.rs` `get_access_token` — refresh 5 menit sebelum kedaluwarsa, token disimpan di DB |
| Streaming tanpa memenuhi memori | ⚠️ Sebagian | Download per item via `baseUrl=d` ke buffer bytes lalu upload (lihat catatan 1) |
| Preservasi metadata & album | ✅ Nyata | Tanggal asli, GPS, kamera, deskripsi disimpan; album dipetakan ke tabel `albums` |
| Dedup SHA-256 + Google Media ID | ✅ Nyata | `db.get_media_by_hash` + cek `google_photos_media_id` |
| Opsi hapus dari Google | ⚠️ Sebagian | Library API **tidak punya endpoint delete** → item diverifikasi aman di Telegram lalu ditandai; UI memberi panduan membersihkan di Google Photos (lihat catatan 2) |

Catatan:
1. Streaming true chunk-to-chunk (tanpa file utuh di disk) belum; saat ini file
   di-download utuh ke memory lalu di-upload. Untuk file >200 MB ini bisa
   diperbaiki dengan `reqwest` stream + `upload_stream` (infrastruktur upload
   streaming sudah ada di `telegram/upload.rs`).
2. `google.rs` `cmd_google_post_import` — keterbatasan resmi API Google
   Photos, bukan kekurangan implementasi.

---

## Modul 2 — Autentikasi & Vault Telegram (§4.2)

| Persyaratan PRD | Status | Implementasi |
|---|---|---|
| Login OTP MTProto langsung | ✅ Nyata | `telegram/mod.rs` `cmd_auth_request_code` + `cmd_auth_sign_in` |
| 2FA cloud password | ✅ Nyata | `cmd_auth_check_password` (PasswordRequired → check_password) |
| Login QR | ✅ Nyata | `cmd_auth_qr_login` (exportLoginToken) + `cmd_auth_qr_poll` |
| Auto-provisioning Private Channel | ✅ Nyata | `telegram/vault.rs` `get_or_create_vault` — buat/deteksi `TelegramPhotos_Vault` |
| Sesi persist + auto-restore (tanpa login ulang) | ✅ Nyata | `cmd_check_connection` cold-start auto-restore dari `telegram.session`; tidak meminta OTP berulang |
| File hingga 2 GB (4 GB Premium) | ✅ Nyata | Upload chunked `saveBigFilePart` tanpa batas ukuran khusus |
| Original quality tanpa kompresi | ✅ Nyata | File di-upload apa adanya (kecuali mode enkripsi yang menambah envelope) |

---

## Modul 3 — Galeri Lokal & Media Scanning (§4.3)

| Persyaratan PRD | Status | Implementasi |
|---|---|---|
| MediaStore Images + Video | ✅ Nyata | `MediaPlugin.kt` `scanMediaStore` (query kedua koleksi, projection per API level) |
| ContentObserver real-time | ✅ Nyata | `MediaPlugin.registerContentObserver` + `android_media.rs` |
| EXIF offline (tanggal, GPS, kamera, ISO, aperture, focal, orientasi) | ✅ Nyata | `media.rs` `extract_exif` (header-only) |
| SHA-256 | ✅ Nyata | `media.rs` `sha256_file` (streaming) |
| Scan anti-OOM (galeri besar) | ✅ Nyata | Tanpa decode penuh di Rust; thumbnail 256 px dari decoder native Android; dimensi via header |
| iOS PHPhotoLibrary | ❌ Belum | Proyek Android-first; struktur `android_media.rs` siap di-mirror ke iOS nanti |

---

## Modul 4 — Auto-Backup Background (§4.4)

| Persyaratan PRD | Status | Implementasi |
|---|---|---|
| WorkManager background job | ✅ Nyata | `BackupScheduler.kt` periodic 15 menit + `BackgroundWorker.kt` → JNI → Rust |
| Foreground notification saat backup | ✅ Nyata | `MediaPlugin.reportProgress` / `notifyBackupDone` (channel notifikasi) |
| Constraints Wi-Fi / charging | ✅ Nyata | `MediaPlugin.checkConstraints` (ConnectivityManager + BatteryManager) via JNI |
| Whitelist folder | ✅ Nyata | `settings.folderBackupSettings`, dicek di `backup.rs` |
| State machine status | ✅ Nyata | `NOT_BACKED_UP → QUEUED → UPLOADING → BACKED_UP → CLOUD_ONLY` di `backup.rs` |
| Jeda 300–500 ms antar file | ✅ Nyata | `backup.rs` `sleep(300 + rand%201)` |
| FLOOD_WAIT resilience | ✅ Nyata | `telegram/upload.rs` — sleep `X + 2` detik, lanjut otomatis |
| iOS BGTaskScheduler | ❌ Belum | Android-first |

---

## Modul 5 — Free Up Space (§4.5)

| Persyaratan PRD | Status | Implementasi |
|---|---|---|
| Kalkulasi ruang aman | ✅ Nyata | `backup.rs` `calculate_free_up_space` |
| Verifikasi SHA-256 sebelum hapus | ✅ Nyata | `cmd_execute_free_up_space` — hash ulang, mismatch → antre ulang |
| Status CLOUD_ONLY + thumbnail tetap | ✅ Nyata | `mark_media_cloud_only`; thumbnail WebP/BlurHash tidak dihapus |

---

## Modul 6 — Timeline Grid & Gestur (§4.6)

| Persyaratan PRD | Status | Implementasi |
|---|---|---|
| Grid 1/3/5/8 kolom | ✅ Nyata | Tombol "N kolom" (cycle) di `Gallery.tsx` — **bukan pinch gesture** (lihat catatan) |
| Sticky date header | ✅ Nyata | `.group-header` sticky per bulan |
| Fast date scrubber | ✅ Nyata | `.scrubber` kanan, scroll ke bulan |
| Drag/long-press multi-select | ✅ Nyata | Long-press 450 ms → mode seleksi; batch aksi |

Catatan: pinch-to-zoom belum; grid scale via tombol (PRD menyebut pinch, tapi
tombol lebih aksesibel; bisa ditambah gesture nanti).

---

## Modul 7 — Pengorganisasian & Pencarian (§4.7)

| Persyaratan PRD | Status | Implementasi |
|---|---|---|
| Pencarian kota/negara/kamera/nama file | ✅ Nyata | `cmd_search_media` (non-AI, offline) |
| Reverse geocoding offline | ⚠️ Sebagian | `geo.rs` — ≈280 kota dunia radius 50 km (bukan DB GeoNames/OSM ~15 MB, lihat catatan) |
| Filter tipe media / favorit / 30 hari | ⚠️ Sebagian | Favorit & video tersedia di UI; chip lengkap bisa ditambah |
| Sampah retensi 30 hari + purge | ✅ Nyata | `cmd_batch_trash` + `cmd_purge_trash` (cutoff 30 hari) |
| Album | ✅ Nyata | Tabel `albums`; struktur album Google dipetakan saat impor |

Catatan: dataset geocode dikurasi (major world + seluruh kota besar Indonesia).
Penggantian ke GeoNames/OSM SQLite 15 MB tinggal mengganti `KNOWN_PLACES` di
`geo.rs` — antarmuka `reverse_geocode(lat, lon)` sudah final.

---

## Modul 8 — Keamanan & Enkripsi (§4.8)

| Persyaratan PRD | Status | Implementasi |
|---|---|---|
| Mode standar (file asli di channel privat) | ✅ Nyata | Default; upload tanpa transformasi |
| XChaCha20-Poly1305 / AES-256-GCM | ✅ Nyata | `crypto.rs` — XChaCha20-Poly1305 streaming |
| Kunci dari passphrase (Argon2id) | ✅ Nyata | `derive_key` — Argon2id 64 MiB, t=3, p=1 |
| Zero-knowledge | ✅ Nyata | Passphrase tidak disimpan; kunci hanya di memory (`VaultState`) |

---

## Skema Database (§5)

| Persyaratan PRD | Status | Implementasi |
|---|---|---|
| SQLite lokal `telegram_photos.db` | ✅ Nyata | `db.rs` (crate `sqlite` bundled, WAL) |
| Tabel `media_items` (kolom sesuai PRD) | ✅ Nyata | `db.rs` `CREATE TABLE media_items` — mencakup EXIF, GPS, status sync, tg_message_id, google fields, trash, album |
| Tabel pendukung (albums, sessions, settings, vault_meta) | ✅ Nyata | `db.rs` |

---

## Arsitektur Rekayasa (§7)

| Persyaratan PRD | Status | Implementasi |
|---|---|---|
| Tech stack mobile-native | ⚠️ Sebagian | Tauri 2 (Rust native + WebView), bukan Flutter/RN — lihat `ARCHITECTURE.md` §7 |
| SQLite | ✅ Nyata | Ya |
| Enkripsi | ✅ Nyata | Ya |
| MTProto tanpa server perantara | ✅ Nyata | Grammers langsung ke DC Telegram |

---

## Optimasi Ekstrem (§11)

| Persyaratan PRD | Status | Implementasi |
|---|---|---|
| Rendering 120 FPS untuk 50–100k foto | ⚠️ Sebagian | Grid di-render per bulan (bukan seluruh dataset); virtualisasi penuh + ImageDecoder native belum (lihat catatan) |
| RAM minimal / anti-OOM | ✅ Nyata | Scan tanpa decode penuh (dimensi header-only, thumbnail native Android, hash streaming); decode penuh hanya di WebView saat tampil |
| SQLite untuk 100k+ baris | ✅ Nyata | Index pada kolom kunci (tanggal, hash, status); keyset pagination tersedia (`cmd_list_timeline` before_timestamp) |
| Streaming MTProto efisien | ✅ Nyata | Upload chunked 512 KB parts, progress, FloodWait |
| Efisiensi baterai | ⚠️ Sebagian | WorkManager (OS-scheduled) + constraints; throttling berbasis suhu belum |

Catatan §11: ini area tersulit dari PRD dan sebagian bergantung pada engine
WebView. Untuk skala 100k+ foto, langkah berikutnya adalah virtualisasi grid
(`@tanstack/react-virtual`) dan decode thumbnail native. Infrastruktur data
(keyset pagination, index, thumbnail tier) sudah siap.

---

## Roadmap 4 Fase (§9)

Seluruh fitur inti PRD sudah terimplementasi dalam satu siklus (fase 1–4
digabung). Yang tersisa sebagai iterasi berikutnya:

1. Pinch-to-zoom grid + virtualisasi 100k foto.
2. Streaming chunk-to-chunk Google→Telegram tanpa buffer penuh.
3. Database geocode GeoNames/OSM lengkap.
4. iOS (PHPhotoLibrary + BGTaskScheduler) — struktur sudah siap.
5. Penjadwalan backup saat media baru terdeteksi (ContentObserver → enqueue
   OneTimeWorkRequest langsung, bukan hanya menunggu interval 15 menit).
6. UI chip filter lengkap (RAW, 30 hari, tangkapan layar).

---

## Ringkasan

| Bagian PRD | Status |
|---|---|
| §4.1 Import Google Photos | ✅ (⚠ streaming penuh & delete API) |
| §4.2 Auth & Vault Telegram | ✅ |
| §4.3 Galeri & MediaScan | ✅ (⚠ iOS) |
| §4.4 Auto-backup background | ✅ (⚠ iOS) |
| §4.5 Free Up Space | ✅ |
| §4.6 Timeline & Gestur | ✅ (⚠ pinch) |
| §4.7 Pencarian & Organisasi | ✅ (⚠ dataset geocode) |
| §4.8 Enkripsi zero-knowledge | ✅ |
| §5 Skema database | ✅ |
| §7 Tech stack | ⚠️ Tauri vs Flutter/RN (alasan di ARCHITECTURE.md) |
| §11 Optimasi ekstrem | ⚠️ bertahap |
