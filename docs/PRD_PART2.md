# 📱 PRD Part 2: Awam-First UX & Telephoto Parity
**Telegram Photos v2 — "Simple Enough for Anyone, Private by Default"**

> Dokumen ini adalah lanjutan dari `PRD.md` (Part 1). Part 1 membangun fondasi teknis
> (MTProto, MediaStore, enkripsi, Google import). Part 2 mendefinisikan **ulang
> pengalaman pengguna dari nol** agar ramah pengguna awam, dengan meniru pola
> kerja aplikasi **Telephoto v69** (hasil reverse engineering: `docs/TELEPHOTO_RE.md`)
> dan mempertahankan keunggulan teknis kita.
>
> **Keputusan produk (disepakati):**
> 1. **AI cloud (OpenAI/Gemini) di-SKIP** — posisi privasi zero-knowledge adalah pembeda utama.
> 2. **Login: QR code dari app Telegram sebagai alur utama** (2 ketukan, tanpa ketik nomor/OTP), OTP sebagai fallback. Tetap MTProto (2 GB/file, akun sendiri).
> 3. **Bahasa UI: Inggris** (target pasar global; awam Indonesia tetap terbantu ikon + alur minimalis).

---

## 1. Ringkasan Eksekutif

PRD Part 1 menghasilkan aplikasi yang **fungsional tapi belum enak dipakai awam**:
onboarding OTP bertele-tele (risiko flood), 4 tab abstrak ("Galeri / Backup / Google / Atur"),
tidak ada umpan balik progres yang jelas, dan tidak ada cara mudah untuk "memahami" apa
yang terjadi dengan foto mereka.

PRD Part 2 memperbaiki ini dengan satu prinsip: **aplikasi harus bisa dipakai penuh
oleh orang yang tidak pernah membaca instruksi apa pun**. Semua fitur dari Telephoto yang
memperbaiki pengalaman (progress hub, collections, captions/hashtags, memories, reel,
view options, tab reorder, widget, OCR/face lokal) ditiru **polanya**; semua fitur yang
membuat kita unggul (enkripsi zero-knowledge, MTProto 2 GB, auto-backup WorkManager,
import Google Photos) **dipertahankan**.

### 1.1 Posisi vs Telephoto

| Dimensi | Telephoto | Telegram Photos v2 |
|---|---|---|
| Privasi | Plaintext + token bot di device + AI cloud | **Zero-knowledge**: enkripsi klien, tanpa AI cloud, tanpa token bocor |
| Upload | Bot API, **skip >48 MB** | MTProto, **2 GB/file** (4 GB premium) — video 4K muat |
| Backup | Foreground service + alarm + battery exemption | **WorkManager** ramah baterai + notifikasi progres |
| UX | Canggih tapi penuh fitur (69 versi) | **Awam-first**: alur Google Photos yang sudah dikenal, fitur bertahap |
| Source | Closed, tanpa lisensi | Open (MIT), bisa diaudit |

### 1.2 Prinsip desain (Awam-First)

1. **Zero-setup success**: dari install sampai foto pertama ter-backup ≤ 3 langkah, semua tanpa mengetik.
2. **Satu sumber kebenaran**: foto = satu timeline (lokal + cloud digabung), status ditandai badge, bukan tab terpisah.
3. **Selalu ada umpan balik**: setiap proses (backup, restore, delete, OCR) terlihat progresnya di satu tempat.
4. **Bahasa manusia**: "Your photos are safe" bukan "3 items queued (status: PENDING)".
5. **Privasi default, bukan fitur**: enkripsi aktif otomatis, bukan tersembunyi di settings.
6. **Konsisten dengan Google Photos**: pola yang sudah dikenal jutaan orang (tab Photos/Search/Library, pinch-zoom, swipe).

---

## 2. Personas (Awam)

| Persona | Usia | Karakteristik | Momen krusial |
|---|---|---|---|
| **"Ibu"** — pengguna biasa | 40–60 | HP penuh, takut kehilangan foto anak, tidak paham istilah teknis | Setup, auto-backup, "kosongkan ruang" |
| **"Remaja"** — pengguna aktif | 16–25 | Ribuan foto/video, butuh cepat & keren (reels, memories) | Backup massal, browsing, sharing |
| **"Pengkhawatir Privasi"** | 25–45 | Tidak percaya cloud biasa | Enkripsi, apa yang dikirim ke mana |
| **"Migran Google Photos"** | Semua | Kuota 15 GB penuh | Import Google, free up space |

**Ukuran kesuksesan (awam):**
- Waktu setup (install → foto pertama ter-backup): **< 3 menit**, tanpa mengetik.
- % user yang berhasil backup di sesi pertama: **> 90%**.
- Tanpa onboarding tutorial: user langsung mengerti cara melihat status backup.

---

## 3. Arsitektur Informasi (IA) Baru

**Keputusan: meniru tab Google Photos (yang sudah dikenal awam), bukan tab Telephoto
yang abstrak.** Telephoto memakai tab Gallery/Cloud/People/Notes — membingungkan karena
"Cloud" dan "Gallery" terpisah. Google Photos menggabungkan semuanya di satu timeline.

```
┌─────────────────────────────────────────────────────┐
│  Bottom Nav (4 tab, ikon + label)                   │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌─────────┐ │
│  │  Photos  │ │  Search  │ │ Library  │ │Settings │ │
│  │ (timeline│ │ (find)   │ │(collections│ │(gear)   │ │
│  │  + backup│ │          │ │ + folders)│ │         │ │
│  │  banner) │ │          │ │          │ │         │ │
│  └──────────┘ └──────────┘ └──────────┘ └─────────┘ │
└─────────────────────────────────────────────────────┘
```

### 3.1 Tab Photos (utama)
- **Timeline gabungan**: semua foto/video (lokal + cloud) dalam satu grid, diurutkan tanggal.
- **Status badge per foto**: 🔵 cloud (backed up), ⏳ uploading (animasi), ⚠ failed (tap untuk retry), 📱 local only.
- **Backup banner** (atas, hanya saat ada progres): "Backing up 12 photos… 45%" → tap buka progress hub.
- Gesture: pinch-zoom grid (1/3/5/8 kolom), sticky date header, fast scrubber, long-press select + drag.

### 3.2 Tab Search
- Search bar besar + hasil instan (nama file, caption, **hashtag #tag**, lokasi, tanggal).
- Chips cepat: Videos, Favorites, Screenshots, RAW, Last 30 days.
- Kalender picker (pilih tanggal/rentang) + peta lokasi (offline geocode).

### 3.3 Tab Library
- **Collections** (album user, bisa cloud) — ditiru dari Telephoto.
- **Folders** (folder device: Camera, WhatsApp, Screenshots…).
- **Favorites** ⭐, **Memories** 🕰, **Trash** (30 hari).

### 3.4 Tab Settings
- **Backup status** besar di atas (jumlah ter-backup, ukuran, tombol "Back up now").
- **Free Up Space** (kalkulasi + konfirmasi + hasil).
- Akun (profil Telegram, vault channel, logout).
- Enkripsi (status aktif + ganti passphrase).
- Auto-backup (Wi-Fi only toggle, charging toggle, folder include/exclude).
- Import Google Photos, Encrypted DB backup, About.

### 3.5 (Opsional, P1) Tab reorder
Telephoto punya `tab_reorder_sheet` — user bisa susun ulang tab. Murah & fleksibel; masuk P1.

---

## 4. Alur Pengguna (User Flows) — Awam-First

### 4.1 First-run: Onboarding 3 langkah, tanpa mengetik
```
Install → Buka app
  1. "Welcome to Telegram Photos" [Get Started]
  2. [Log in with Telegram]  → buka QR screen
     - QR code tampil → user scan dengan app Telegram
       (Telegram → Settings → Devices → Scan QR)
     - QR login MTProto: konfirmasi 1 ketukan di Telegram, app langsung masuk.
  3. [Allow access to photos] → permission dialog (READ_MEDIA_IMAGES/VIDEO)
     → app langsung scan & mulai backup otomatis.
  ✅ "Your photos are being backed up securely." (banner + notifikasi)
```
- **Fallback OTP**: link kecil "Use phone number instead" → flow OTP lama (diperbaiki UX-nya).
- **Tanpa onboarding tutorial** — alur sudah cukup jelas; tooltip satu baris saat pertama melihat badge.
- Error state (mis. Telegram butuh verifikasi): pesan manusiawi + tombol "Try again", **tidak pernah minta OTP ulang** jika sesi sudah authorized (auto-restore sesi — sudah ada di v1.2).

### 4.2 Daily use: Auto-backup tanpa interaksi
```
User foto → ContentObserver mendeteksi → status "local" muncul di grid
  → WorkManager (15 mnt / Wi-Fi / charging sesuai settings) → upload via MTProto
  → badge berubah 🔵 → notifikasi ringkas "12 photos backed up"
  → user bisa lihat semua di tab Photos (badge cloud) — TANPA tab terpisah
```
- Gagal? Badge ⚠ di foto + entri di Progress Hub → tap = retry. **Exponential backoff otomatis**.
- **Koneksi hilang** → pause otomatis, resume saat koneksi balik (state machine, §6.2).

### 4.3 Browsing & preview (Unified Preview)
```
Tap foto → fullscreen (streaming dari Telegram jika cloud-only)
  → swipe kiri/kanan antar foto (virtualized, buffer 1 layar)
  → swipe atas (foto) / swipe atas (video = reel) 
  → tap info (i) → panel: caption + hashtags, EXIF (date, camera, location),
    collection add, favorite, share, delete
```
- Video: player inline + gesture seek/volume/brightness (dari Telephoto `video_player_screen`).

### 4.4 Free Up Space (awam-friendly)
```
Settings → Free Up Space
  → "You can free up 12.4 GB" (kalkulasi aman: hanya BACKED_UP + terverifikasi hash)
  → [Free up space] → progress → "Done! 12.4 GB freed"
  → foto tetap terlihat (thumbnail + BlurHash), tap = stream dari Telegram
  → tombol undo/redo selama 5 detik setelah eksekusi
```
- **Verifikasi SHA-256 sebelum hapus** (sudah ada) — jangan pernah hapus yang belum pasti aman.

### 4.5 Restore (ke HP baru / setelah hapus)
```
HP baru → login QR → pilih "Restore from cloud"
  → pilih rentang (All / Last 30 days / Collection) 
  → progress hub → file download asli → masuk galeri lokal
```
- (P1) Restore selektif per foto/collection dari menu konteks.

---

## 5. Fitur Part 2 — "Telephoto Parity" (apa yang ditiru, apa yang dipertahankan)

Legenda: ✅ **TIRU** (adaptasi pola) · 🛡 **PERTAHANKAN** (fitur kita) · ⏸ **TUNDA** (fase berikutnya) · ❌ **SKIP** (bertentangan dengan posisi kita)

### 5.1 Ditiru dari Telephoto

| # | Fitur | Keputusan | Detail implementasi |
|---|---|---|---|
| T1 | **Task Progress Hub** | ✅ | Satu tempat semua proses: upload, download, delete, scan, restore, OCR. Progress bar + status per item + retry. UI: layar + badge notifikasi. Backend: event stream Rust→JS. |
| T2 | **Upload state machine + resume** | ✅ | Status eksplisit: `pending → uploading → backed_up`, plus `failed / skipped / paused`; tabel `upload_errors` dengan pesan + retry count; resume dari posisi chunk. |
| T3 | **Dedup hash di background** | ✅ | SHA-256 sudah ada; pindahkan ke thread/queue terpisah agar tidak blok UI (Telephoto: `background_hash_service`). |
| T4 | **Captions & hashtags** | ✅ | Panel caption per foto (markdown ringan), hashtag #tag, pencarian #tag, "apply to multiple". Tabel relasi `captions`/`caption_tags`. |
| T5 | **Collections** | ✅ | Album user (lokal & cloud), foto+video, add/remove cepat dari panel foto. `collections` + `collection_items`. |
| T6 | **Memories** | ✅ | Kartu "On this day" di Library + widget; query tanggal sama tahun lalu. |
| T7 | **Reel mode** | ✅ | Video vertical continuous (auto-next), gesture seek/volume/brightness, playback speed. |
| T8 | **Cloud view options** | ✅ | Group by (date/collection/folder), sort, filter, grid column control (2–6). |
| T9 | **Unified preview** | ✅ | Satu preview untuk foto+video (sudah sebagian ada; disempurnakan jadi unified). |
| T10 | **Encrypted DB backup/restore** | ✅ | Backup settings+metadata DB terenkripsi (passphrase), validasi pemilik (bind ke akun), restore di HP baru. |
| T11 | **Home screen widget** (Android 16+) | ⏸ P1 | Widget memories + recent photos; data via Kotlin `AppWidgetProvider` + JNI. |
| T12 | **OCR offline** (Tesseract) | ⏸ P2 | Pipeline antrean per foto, traineddata di-download per bahasa pilihan, hasil di DB + pencarian teks. Tanpa AI cloud. |
| T13 | **Face recognition offline** | ⏸ P2 | ML Kit deteksi + model embedding kecil (pilih model < 20 MB, bukan facenet 90 MB), group People. Wajib ABI-split. |
| T14 | **Tab reorder + grid control** | ⏸ P1 | `tab_reorder_sheet` (drag untuk susun tab), slider kolom grid. |
| T15 | **Thumbnail shimmer** | ✅ | Skeleton loading saat scroll cepat (sudah sebagian via BlurHash; tambah shimmer frame). |
| T16 | **Image conversion optional** | ✅ | Opsi "Original" (default, MTProto 2 GB) vs "Compact" (kompres HEIC→JPEG, hemat kuota) — beda dari Telephoto yang paksa konversi. |

### 5.2 Dipertahankan (keunggulan kita)

| # | Fitur | Alasan dipertahankan |
|---|---|---|
| K1 | **Enkripsi zero-knowledge** (XChaCha20-Poly1305 + Argon2id) | Pembeda #1 vs Telephoto; default ON, bukan opsional tersembunyi. |
| K2 | **MTProto + akun sendiri + vault channel privat** | 2 GB/file, tanpa token bot yang bisa bocor, sesi tersimpan (tidak minta OTP ulang). |
| K3 | **Auto-backup WorkManager** (15 mnt, Wi-Fi/charging constraint, notifikasi) | Ramah baterai & privasi vs foreground service agresif Telephoto. |
| K4 | **Import Google Photos 1-klik** | Fitur yang tidak dimiliki Telephoto; tetap di tab Settings + banner satu kali. |
| K5 | **Scan anti-OOM** (thumbnail native Android, header-only dimension, hash streaming) | Hasil reverse-engineering fix kita sendiri; dipertahankan. |
| K6 | **Free Up Space dengan verifikasi hash** | Jangan pernah hapus yang belum terverifikasi. |
| K7 | **Galeri real-time** (ContentObserver) | Auto-detect foto/video baru seketika. |
| K8 | **Sesi auto-restore saat cold start** | Anti-flood OTP; langsung masuk tanpa login ulang. |

### 5.3 Di-skip / ditunda

| Fitur Telephoto | Keputusan | Alasan |
|---|---|---|
| AI caption/notes (OpenAI/Gemini) | ❌ SKIP | Bertentangan dengan posisi zero-knowledge (keputusan user). |
| Notes tab (markdown + AI) | ⏸ TUNDA | Di luar inti "photo backup"; tanpa AI nilainya rendah. |
| QR contact / barcode | ⏸ P2 | ML Kit barcode kecil; nilai marginal. |
| Battery optimization exemption | ❌ SKIP | Agresif & menakutkan; WorkManager cukup. |
| MANAGE_EXTERNAL_STORAGE | ❌ SKIP | Izin over-permission; MediaStore + READ_MEDIA cukup. |
| Bot token mode | ❌ SKIP | Tidak aman (token bocor = semua foto terbaca), batas 50 MB. |

---

## 6. Spesifikasi Teknis Part 2

### 6.1 Task Progress Hub (T1)
```
Rust (core)                          JS (UI)
  backup/restore/delete/scan/ocr ──► events ──► ProgressHub store ──► UI (badge + layar)
        ▲                                                              │
        └─────────────── ack/retry/pause ◄─────────────────────────────┘
```
- Event: `{task_id, kind, total, done, current_item, status, message}` via Tauri event channel.
- UI: tombol badge "2 uploads running" di topbar → layar `TaskProgress` (daftar task + progress bar + tombol pause/resume/retry/cancel).
- Notifikasi: ringkas saat selesai ("12 photos backed up"), hanya error yang mengganggu.
- Backend: `task_hub.rs` — registry task, atomic counter, persist state (resume setelah app mati).

### 6.2 Upload state machine (T2) — perluasan dari PRD Part 1 §4.4
```
NOT_BACKED_UP → QUEUED → UPLOADING ⇄ PAUSED
                    ↘ FAILED_RETRY → (exponential backoff, max 5) → QUEUED
                    ↘ SKIPPED (file > limit / invalid / user skip)
UPLOADING → BACKED_UP (message_id + file_id + hash tersimpan)
BACKED_UP → CLOUD_ONLY (free up space)
```
- Tabel `uploads` baru (terpisah dari `media_items`): `media_id, message_id, file_id, hash_sha256, status, retry_count, last_error, uploaded_bytes, total_bytes, created_at, updated_at`.
- Tabel `upload_errors`: `upload_id, error_code, message, at`.
- Resume: `uploaded_bytes` disimpan → MTProto `upload.saveBigFilePart` lanjut dari part terakhir.
- Konflik dengan DB part 1: migrasi schema v1→v2 dengan `PRAGMA user_version` (tidak perlu reset data user).

### 6.3 Captions & hashtags (T4)
- Tabel `captions(id, media_id UNIQUE, text, updated_at)` + `caption_tags(id, media_id, tag, UNIQUE(media_id, tag))`.
- Panel caption: textarea + tag chips + "Apply to selected" (multi-select).
- Pencarian: `LIKE '%#tag%'` + indeks FTS5 untuk caption text (opsional P1).
- Caption ikut ter-upload sebagai caption pesan Telegram (menggunakan `editMessageCaption` MTProto) — sinkron cloud.

### 6.4 Collections (T5)
- Tabel `collections(id, name, cover_media_id, is_cloud, sort_order, created_at)` + `collection_items(collection_id, media_id, added_at)`.
- Cloud collection: folder virtual yang disimpan di metadata DB (bukan struktur Telegram) — sinkron antar device via encrypted DB sync (P1) atau backup DB.
- UI: Library → Collections → grid; add dari panel foto; drag-reorder.

### 6.5 QR Login (keputusan produk)
- Grammers mendukung QR login (`qr_login`): tampilkan `token` sebagai QR di layar, poll status sampai user konfirmasi dari app Telegram.
- Flow: `login screen → QR (auto-refresh 60s) → authorized → auto-provision vault channel → masuk`.
- Fallback: "Use phone number instead" → OTP flow (sudah ada).
- Security: QR hanya valid 1x & pendek umurnya; sesi disimpan SQLite (auto-restore).

### 6.6 Encrypted DB backup (T10)
- Ekspor: `media_items`, `uploads`, `captions`, `collections`, settings → satu file JSON → enkripsi XChaCha20-Poly1305 (passphrase Argon2id) → simpan ke Telegram (vault) sebagai file `.tphotos-backup`.
- Validasi pemilik: simpan hash akun di header file; saat restore, cocokkan dengan akun yang login.
- Restore: pilih file → masukkan passphrase → verifikasi → impor ke DB (merge, bukan replace).

### 6.7 Database — skema tambahan Part 2
```sql
-- (ditambahkan ke schema part 1; migrasi via PRAGMA user_version)
CREATE TABLE uploads (...);          -- state machine §6.2
CREATE TABLE upload_errors (...);    -- retry & diagnosa
CREATE TABLE captions (...);
CREATE TABLE caption_tags (...);
CREATE TABLE collections (...);
CREATE TABLE collection_items (...);
CREATE TABLE ocr_queue (...);        -- P2
CREATE TABLE ocr_results (...);      -- P2
CREATE TABLE people (...);           -- P2
CREATE TABLE detected_faces (...);   -- P2
-- Index: uploads(status), uploads(media_id), caption_tags(tag),
--        collection_items(collection_id), media_items(date_taken)
```

### 6.8 Arsitektur service (meniru granularitas Telephoto)
```
app/src-tauri/src/
├── commands.rs            # API surface (existing)
├── task_hub.rs            # NEW: progress hub terpusat
├── upload_manager.rs      # NEW: antrean + state machine + resume
├── hash_worker.rs         # NEW: dedup hash thread terpisah
├── captions.rs            # NEW
├── collections.rs         # NEW
├── qr_login.rs            # NEW: QR auth via Grammers
├── db_backup.rs           # NEW: encrypted DB export/restore
├── telegram/              # existing (MTProto)
├── media.rs, android_media.rs, geo.rs ...  # existing
```
- UI: React Router ringan (state `tab` di-extend), store ProgressHub, komponen baru (`ProgressHub`, `CaptionPanel`, `CollectionSheet`, `MemoryCard`, `ReelViewer`, `Shimmer`).

---

## 7. Design System (untuk awam)

- **Bahasa UI: Inggris** (keputusan). Copy pendek, kata sehari-hari: "Back up", "Free up space", "Your photos are safe".
- **Warna**: Material You (dynamic color di Android 12+, fallback palette hijau/telegram). Status: 🔵 backed up, 🟠 uploading, 🔴 failed, ⚪ local.
- **Tipografi**: Roboto/System, ukuran nyaman (16+), kontras tinggi (WCAG AA).
- **Ikon**: outline 24dp konsisten (Material Symbols), label teks selalu ada (bukan ikon saja).
- **Empty states**: ilustrasi + satu kalimat + tombol aksi (mis. "No photos yet — take your first photo!").
- **Loading**: shimmer/skeleton (bukan spinner tak berujung); progress selalu punya angka.
- **Error**: pesan manusiawi + solusi ("Check your connection and try again"), tombol Retry, tidak pernah error code mentah.
- **Safe area & gestur**: `env(safe-area-inset-*)` (sudah), hit target ≥ 48dp, swipe konsisten (back = sistem).

---

## 8. Roadmap Part 2 (3 fase)

| Fase | Isi | Estimasi |
|---|---|---|
| **P0 — v2.0 "Awam Core"** | IA baru (4 tab Google Photos-style) · Onboarding QR login · backup banner + badge status · **Task Progress Hub** · upload state machine + resume + upload_errors · dedup background · captions & hashtags · collections (dasar) · unified preview polish · encrypted DB backup · migrasi schema v1→v2 | 2–3 minggu |
| **P1 — v2.1 "Delight"** | Memories · Reel mode · cloud view options (group/sort/filter + grid control) · tab reorder · widget home screen · restore selektif · FTS5 caption search | 2 minggu |
| **P2 — v2.2 "Smart & Offline"** | OCR offline (Tesseract, per-bahasa) · face recognition offline (model ringan, ABI-split) · QR contact · people grouping · video thumbnails pipeline penuh | 3 minggu |

**Urutan P0 wajib** (tidak boleh ditukar): IA → QR login → progress hub → state machine → badge/banner (badge tanpa state machine = status bohong).

---

## 9. Non-Goals (v2)

- ❌ AI cloud (caption/notes via OpenAI/Gemini) — skip permanen.
- ❌ Notes tab.
- ❌ Bot token mode.
- ❌ Izin over-permission (MANAGE_EXTERNAL_STORAGE, battery exemption).
- ❌ iOS — tetap Android-first (PRD part 1 menyebut iOS; ditunda sampai v2 stabil).
- ❌ Face recognition model besar (facenet 90 MB) — wajib model ringan jika masuk P2.

## 10. Risiko & Mitigasi

| Risiko | Mitigasi |
|---|---|
| Migrasi schema v1→v2 merusak DB user | `PRAGMA user_version` + migrasi bertahap + backup DB sebelum migrasi |
| QR login gagal di device lama (minSdk) | Fallback OTP penuh; QR hanya di Android 7+ (minSdk 24 aman) |
| Badge status memperlambat grid 100k foto | Badge di-render dari kolom status DB (sudah ada index), virtualisasi grid |
| Progress hub event overload (ribuan event/detik) | Throttle event ke UI (50ms batch), update atomik di Rust |
| Resume upload kompleks (part offset) | Simpan `uploaded_bytes` per item; uji mati-listrik di emulator |
| User awam tidak paham enkripsi | Default ON + satu baris penjelasan di settings ("Only you can read your photos") |

---

## 11. Ringkasan Keputusan

1. IA = 4 tab gaya Google Photos (Photos / Search / Library / Settings) — bukan tab Telephoto.
2. Fitur Telephoto ditiru **polanya** (progress hub, collections, captions, memories, reel, view options, OCR/face lokal bertahap) — bukan kodenya (tanpa lisensi).
3. Keunggulan kita (enkripsi, MTProto 2 GB, WorkManager, Google import, scan anti-OOM) dipertahankan.
4. AI cloud di-skip; UI Inggris; QR login utama + OTP fallback.
5. Eksekusi berurutan: P0 (awam core) → P1 (delight) → P2 (smart offline).
