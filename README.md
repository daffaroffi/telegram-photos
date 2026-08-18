# Telegram Photos

Backup galeri foto & video ke channel privat Telegram — lengkap dengan migrasi
Google Photos, enkripsi zero-knowledge, dan auto-backup background.

Dibangun ulang dari nol berdasarkan `PRD.md` dengan **backend Rust asli**
(bukan simulasi): MTProto Grammers untuk upload sungguhan, SQLite lokal,
enkripsi XChaCha20-Poly1305, MediaStore + WorkManager Android native, dan
importer Google Photos via OAuth2 + Library API.

| Ringkasan | Nilai |
|---|---|
| Platform | Android (Tauri 2 + WebView), desktop (Windows/macOS/Linux) |
| Backend | Rust — Grammers MTProto, rusqlite-style `sqlite`, reqwest |
| Frontend | React 19 + TypeScript + Vite |
| Native Android | Kotlin — MediaStore, ContentObserver, WorkManager, notifikasi |
| Ukuran APK release | ±13 MB (`libtelegram_photos_lib.so` ±10 MB) |
| Enkripsi | XChaCha20-Poly1305 + Argon2id (opsional, zero-knowledge) |

---

## Fitur

### 1. Backup ke Telegram (nyata, bukan simulasi)
- Login MTProto resmi: **OTP via SMS/Telegram**, **2FA cloud password**, atau **QR code**.
- Auto-provisioning **Private Channel** `TelegramPhotos_Vault` (dibuat otomatis saat pertama login).
- Upload chunked dengan **progress real-time**, dukungan file besar, dan
  penanganan `FLOOD_WAIT` (jeda otomatis sesuai instruksi Telegram).
- State machine backup: `NOT_BACKED_UP → QUEUED → UPLOADING → BACKED_UP → CLOUD_ONLY`.

### 2. Galeri lokal Android (MediaStore asli)
- Scan `MediaStore.Images` + `MediaStore.Video` via plugin Kotlin (bukan file picker).
- `ContentObserver` untuk deteksi media baru secara real-time.
- Ekstraksi EXIF offline: tanggal, GPS, model kamera, ISO, aperture, focal length.
- Hash SHA-256 per file untuk deduplikasi & verifikasi integritas.
- Scan galeri **tanpa decode penuh** (anti-OOM): thumbnail dibuat decoder
  native Android, dimensi dibaca dari header, EXIF tetap diekstrak.
- Thumbnail WebP bertingkat + BlurHash untuk alur berbasis path (desktop).

### 3. Auto-backup background (WorkManager)
- Periodic worker 15 menit memanggil mesin backup Rust via JNI (database &
  sesi Telegram yang sama dengan UI).
- Constraints: **hanya Wi-Fi**, **hanya saat charging** — diperiksa dari state
  nyata perangkat.
- Notifikasi progres selama backup berjalan.
- Whitelist folder (Kamera/WhatsApp/Instagram/… bisa di-on/off).

### 4. "Bebaskan Ruang Perangkat" (Free Up Space)
- Hitung ruang yang bisa dibebaskan dari file yang sudah terverifikasi `BACKED_UP`.
- Sebelum hapus, **verifikasi SHA-256 ulang** terhadap hash yang tercatat.
- Thumbnail tetap tersimpan agar galeri tetap bisa dilihat offline.

### 5. Migrasi Google Photos (cloud-to-cloud)
- OAuth 2.0 (`photoslibrary.readonly`) dengan token refresh otomatis.
- Discovery: total item, ukuran, daftar album.
- Import streaming: download dari Google (`baseUrl=d`) → upload ke Telegram,
  metadata (tanggal asli, GPS, kamera, album) dipertahankan.
- Dedup berdasarkan hash SHA-256 + Google Media ID.
- Dialog pasca-import: hapus dari Google (dengan panduan) atau biarkan ganda.

### 6. Keamanan & Vault (zero-knowledge)
- Enkripsi opsional **XChaCha20-Poly1305** (streaming, 4 KB chunk) dengan kunci
  turunan **Argon2id** dari passphrase pengguna.
- Passphrase tidak pernah disimpan; hanya salt + parameter KDF di database.
- Vault terkunci saat background worker berjalan → item terenkripsi menunggu
  dibuka oleh pengguna (file tidak pernah terkirim tanpa kunci).

### 7. UI mobile-first
- Grid timeline 1/3/5/8 kolom, sticky header bulan, fast date scrubber.
- Multi-select long-press, batch favorite / trash / antre backup.
- Pencarian non-AI: kota, negara, model kamera, nama file.
- Sampah dengan retensi 30 hari + purge otomatis.
- Reverse geocoding offline (≈280 kota dunia, radius 50 km).

---

## Struktur Proyek

```
.
├── docs/
│   ├── ARCHITECTURE.md             # Arsitektur teknis detail
│   ├── PRD_COVERAGE.md             # Pemetaan PRD → implementasi
│   └── BUILD.md                    # Panduan build & instalasi
├── app/
│   ├── src/                        # Frontend React (TypeScript)
│   │   ├── api.ts                  # Wrapper seluruh command Tauri
│   │   ├── types.ts                # Tipe data (cermin model Rust)
│   │   └── components/             # Onboarding, Galeri, Backup, Google, Setelan
│   └── src-tauri/
│       ├── src/                    # Backend Rust
│       │   ├── telegram/           # MTProto: auth, vault channel, upload
│       │   ├── db.rs               # SQLite (skema PRD §5)
│       │   ├── crypto.rs           # XChaCha20-Poly1305 + Argon2id
│       │   ├── media.rs            # EXIF, thumbnail, BlurHash, SHA-256
│       │   ├── backup.rs           # State machine backup + free up space
│       │   ├── google.rs           # OAuth2 + importer Google Photos
│       │   ├── geo.rs              # Reverse geocoding offline
│       │   ├── android_media.rs    # Jembatan JNI ke plugin Kotlin
│       │   └── bg_worker.rs        # Export JNI untuk WorkManager
│       └── gen/android/            # Proyek Android (Kotlin)
│           └── app/src/main/java/com/telegramphotos/app/
│               ├── MediaPlugin.kt      # MediaStore, constraints, notifikasi
│               ├── BackgroundWorker.kt # Worker WorkManager → Rust via JNI
│               └── BackupScheduler.kt  # Jadwal periodic 15 menit
```

---

## Mulai Cepat

Persyaratan: Node.js 20+, Rust (stable), Android SDK + NDK, Java 17+.

```bash
# 1. Install dependensi frontend
cd app
npm install

# 2. Android: build APK (aarch64; butuh ANDROID_HOME)
export ANDROID_HOME="$HOME/Android/Sdk"
npx tauri android build --target aarch64

# APK: app/src-tauri/gen/android/app/build/outputs/apk/universal/release/
# (signing: lihat docs/BUILD.md)

# 3. Desktop: jalankan mode dev
npm run tauri dev
```

Detail lengkap (prasyarat, signing keystore, struktur perintah): lihat
**[docs/BUILD.md](docs/BUILD.md)**.

## Referensi riset

- **[docs/TELEPHOTO_RE.md](docs/TELEPHOTO_RE.md)** — hasil reverse engineering
  aplikasi pesaing Telephoto v69 (Flutter + Bot API): arsitektur, model data,
  pola UI/UX yang layak ditiru, dan kelemahan yang menjadi pembeda kita.

## Lisensi

[MIT](LICENSE)

---

## Cakupan PRD

Semua modul inti PRD terimplementasi dengan fungsi nyata (bukan mock):

- Import Google Photos (OAuth + API + streaming) — **nyata**
- Autentikasi & vault Telegram (MTProto) — **nyata**
- Galeri lokal MediaStore + EXIF — **nyata**
- Auto-backup background (WorkManager) — **nyata**
- Free Up Space dengan verifikasi hash — **nyata**
- Timeline grid + gestur + pencarian — **nyata**
- Enkripsi zero-knowledge — **nyata**

Catatan jujur tentang keterbatasan (Google Library API tidak punya endpoint
hapus; iOS belum didukung; database geocode berupa dataset ~280 kota bukan
GeoNames 15 MB) dijelaskan per-bagian di
**[docs/PRD_COVERAGE.md](docs/PRD_COVERAGE.md)**.

---

## Teknologi

| Lapisan | Pilihan | Alasan |
|---|---|---|
| Runtime aplikasi | Tauri 2 | Backend Rust native + WebView; ukuran kecil |
| Telegram | Grammers (MTProto) | Client Telegram resmi di Rust; sesi SQLite; upload chunked |
| Database | `sqlite` (bundled) | Satu-satunya yang kompatibel dengan `grammers-session`; PRAGMA WAL |
| Enkripsi | `chacha20poly1305` + `argon2` | XChaCha20-Poly1305 streaming; Argon2id KDF |
| Gambar | `image` (WebP) + `exifr`-style `kamadak-exif` | Thumbnail & metadata offline |
| HTTP | `reqwest` | Google Photos API |
| Android | Kotlin + WorkManager | MediaStore, ContentObserver, background job, notifikasi |
