# 📱 PRD Part 2: Awam-First UX & Telephoto Parity
**Telegram Photos v2 — "Simple Enough for Anyone, Private by Default, Fast at 100k Photos"**

> Dokumen ini adalah lanjutan dari `PRD.md` (Part 1). Part 1 membangun fondasi teknis
> (MTProto, MediaStore, enkripsi, Google import) dan blueprint optimasi di §11.
> Part 2 mendefinisikan ulang **pengalaman pengguna dari nol** agar ramah pengguna awam
> (meniru pola kerja Telephoto v69 — `docs/TELEPHOTO_RE.md`) **dan** menetapkan
> **Performance Engineering** yang terukur sekelas Telephoto.
>
> **Keputusan produk (disepakati):**
> 1. **AI cloud (OpenAI/Gemini) di-SKIP** — posisi privasi zero-knowledge adalah pembeda utama.
> 2. **Login: QR code dari app Telegram sebagai alur utama** (2 ketukan, tanpa ketik nomor/OTP), OTP fallback. Tetap MTProto (2 GB/file, akun sendiri).
> 3. **Bahasa UI: Inggris** (target global). Struktur string dibuat i18n-ready sejak awal agar bahasa lain murah ditambahkan.

---

## 1. Ringkasan Eksekutif

PRD Part 1 menghasilkan aplikasi yang **fungsional tapi belum enak dipakai awam**:
onboarding OTP bertele-tele (risiko flood), 4 tab abstrak ("Galeri / Backup / Google / Atur"),
tidak ada umpan balik progres yang jelas, dan tidak ada tolok ukur performa yang terukur.

PRD Part 2 memperbaiki ini dengan dua pilar:
1. **Awam-First UX** — aplikasi bisa dipakai penuh oleh orang yang tidak pernah membaca instruksi.
   Semua fitur Telephoto yang memperbaiki pengalaman ditiru **polanya**; keunggulan teknis kita dipertahankan.
2. **Performance Engineering terukur** — setiap fitur baru punya **budget angka** (waktu, RAM, FPS,
   baterai) dan **strategi implementasi** (pipeline thumbnail, caching, device-tier tuning, throttling).
   Performa bukan sesudahnya — **didefinisikan di PRD, diverifikasi di tiap milestone**.

### 1.1 Posisi vs Telephoto

| Dimensi | Telephoto | Telegram Photos v2 |
|---|---|---|
| Privasi | Plaintext + token bot di device + AI cloud | **Zero-knowledge**: enkripsi klien, tanpa AI cloud, tanpa token bocor |
| Upload | Bot API, **skip >48 MB** | MTProto, **2 GB/file** (4 GB premium) |
| Backup | Foreground service + alarm + battery exemption | **WorkManager** ramah baterai + notifikasi progres |
| UX | Canggih tapi penuh fitur (69 versi) | **Awam-first**: pola Google Photos yang dikenal, fitur bertahap |
| Source | Closed, tanpa lisensi | Open (MIT), bisa diaudit |

### 1.2 Prinsip desain (Awam-First)

1. **Zero-setup success**: install → foto pertama ter-backup ≤ 3 langkah, tanpa mengetik.
2. **Satu sumber kebenaran**: satu timeline (lokal + cloud), status ditandai badge — bukan tab terpisah.
3. **Selalu ada umpan balik**: semua proses terlihat di satu Task Progress Hub.
4. **Bahasa manusia**: "Your photos are safe" bukan "3 items queued (PENDING)".
5. **Privasi default, bukan fitur**: enkripsi aktif otomatis.
6. **Konsisten dengan Google Photos**: pola yang sudah dikenal (tab Photos/Search/Library, pinch-zoom).

---

## 2. Personas, KPI & Tolok Ukur Keberhasilan

### 2.1 Personas

| Persona | Usia | Karakteristik | Momen krusial |
|---|---|---|---|
| **"Ibu"** — pengguna biasa | 40–60 | HP penuh, takut kehilangan foto anak, tidak paham istilah teknis | Setup, auto-backup, free up space |
| **"Remaja"** — pengguna aktif | 16–25 | Ribuan foto/video, butuh cepat & keren (reels, memories) | Backup massal, browsing, sharing |
| **"Pengkhawatir Privasi"** | 25–45 | Tidak percaya cloud biasa | Enkripsi, transparansi data |
| **"Migran Google Photos"** | Semua | Kuota 15 GB penuh | Import Google, free up space |

### 2.2 KPI Produk (UX)

| KPI | Target | Cara ukur |
|---|---|---|
| Waktu setup → foto pertama ter-backup | **< 3 menit**, tanpa mengetik | Instrumentasi flow onboarding |
| % user berhasil backup di sesi pertama | **> 90%** | Analytics funnel (anonymized, opt-in) |
| Pengguna paham status backup tanpa tutorial | 0 langkah tutorial | Usability test 5 awam |
| Foto yang hilang (data loss) | **0** | Verifikasi hash end-to-end |

### 2.3 KPI Performa (teknis — wajib diverifikasi tiap milestone)

| Metrik | Target | Alat ukur |
|---|---|---|
| Cold start → grid tampil | **< 500 ms** (mid-range) | `adb shell am start -W`, Perfetto |
| Scroll grid 100k foto | **60 fps stabil** (120 fps high-end), 0 jank >16 ms | Perfetto / `dumpsys gfxinfo` |
| RAM steady-state | **80–150 MB** (≤ 256 MB heap) | `dumpsys meminfo` |
| Scan 1.000 foto baru | **< 30 detik** (tanpa decode penuh) | Instrumentasi `cmd_scan` |
| Upload 1 GB (5G/Wi-Fi) | **> 80% kecepatan link**, tanpa FloodWait | Log throughput + flood counter |
| Thumbnail grid 100 foto muncul | **< 1 detik** dari DB (BlurHash instan) | Perfetto |
| Baterai idle (background) | **< 1% / jam** | `dumpsys batterystats` |
| APK release (arm64) | **≤ 35 MB** | Ukuran artefak |
| Video 4K stream dari cloud | start < 1 s, seek < 500 ms | Instrumentasi player |
| Crash-free sessions | **> 99.5%** | Crash reporting (opt-in) |

---

## 3. Arsitektur Informasi (IA) Baru

**Keputusan: meniru tab Google Photos (dikenal awam), bukan tab Telephoto
(Gallery/Cloud/People/Notes yang abstrak & memisah "Cloud" dari "Gallery").**

```
┌─────────────────────────────────────────────────────┐
│  Bottom Nav (4 tab, ikon + label, ≥48dp hit target) │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌─────────┐ │
│  │  Photos  │ │  Search  │ │ Library  │ │Settings │ │
│  │ (timeline│ │ (find)   │ │(collections│ │(gear)   │ │
│  │  + banner│ │          │ │ + folders)│ │         │ │
│  └──────────┘ └──────────┘ └──────────┘ └─────────┘ │
└─────────────────────────────────────────────────────┘
```

### 3.1 Tab Photos (utama)
- **Timeline gabungan**: lokal + cloud dalam satu grid virtualized, urut tanggal.
- **Status badge per foto**: 🔵 backed up · ⏳ uploading (animasi) · ⚠ failed (tap = retry) · 📱 local only.
- **Backup banner** (hanya saat ada progres): "Backing up 12 photos… 45%" → tap buka Progress Hub.
- Gesture: pinch-zoom grid (1/3/5/8), sticky date header, fast scrubber, long-press + drag select.

### 3.2 Tab Search
- Search bar besar + hasil instan (file name, caption, **#hashtag**, lokasi, tanggal).
- Chips cepat: Videos · Favorites · Screenshots · RAW · Last 30 days.
- Kalender picker + daftar lokasi (offline geocode).

### 3.3 Tab Library
- **Collections** (album user, bisa cloud) · **Folders** (device) · **Favorites** ⭐ ·
  **Memories** 🕰 · **Trash** (30 hari).

### 3.4 Tab Settings
- **Backup status** besar di atas (jumlah, ukuran, "Back up now").
- **Free Up Space** (kalkulasi → konfirmasi → hasil + undo 5 detik).
- Akun (profil, vault channel, logout) · Enkripsi (status + ganti passphrase) ·
  Auto-backup (Wi-Fi only, charging, folder include/exclude) · Import Google ·
  Encrypted DB backup · About.

### 3.5 Tab reorder (P1)
Telephoto punya `tab_reorder_sheet` — user susun ulang tab. Murah & fleksibel; masuk P1.

---

## 4. Alur Pengguna (User Flows) — Awam-First

### 4.1 First-run: onboarding 3 langkah, tanpa mengetik
```
Install → Buka app
  1. "Welcome to Telegram Photos"  [Get Started]
  2. [Log in with Telegram]  → QR screen
     - Scan dengan app Telegram (Settings → Devices → Scan QR)
     - Konfirmasi 1 ketukan di Telegram → app langsung masuk (MTProto QR login).
  3. [Allow access to photos] → permission dialog → auto scan + auto backup.
  ✅ "Your photos are being backed up securely." (banner + notifikasi ringkas)
```
- **Fallback OTP**: link kecil "Use phone number instead" → flow OTP yang dirapikan UX-nya.
- **Tanpa tutorial** — tooltip 1 baris saat pertama lihat badge.
- Error state: pesan manusiawi + "Try again"; **tidak pernah minta OTP ulang** jika sesi sudah authorized (auto-restore sesi v1.2).

### 4.2 Daily use: auto-backup tanpa interaksi
```
User foto → ContentObserver → status "local" muncul di grid
  → WorkManager (constraint sesuai settings) → upload MTProto (state machine §6.2)
  → badge 🔵 → notifikasi ringkas "12 photos backed up"
  → semua terlihat di satu timeline — TANPA tab terpisah
```
- Gagal → badge ⚠ + entri Progress Hub; tap = retry; **exponential backoff**.
- Koneksi hilang → **pause otomatis**, resume dari posisi chunk saat kembali.

### 4.3 Browsing & preview (Unified Preview)
```
Tap foto → fullscreen (stream dari Telegram jika cloud-only; BlurHash dulu, <1s)
  → swipe kiri/kanan antar foto (virtualized, buffer 1 layar)
  → video: swipe atas = reel mode · gesture seek/volume/brightness
  → info (i): caption + hashtags, EXIF (date/camera/location), collection add,
    favorite, share, delete
```

### 4.4 Free Up Space (awam-friendly)
```
Settings → Free Up Space → "You can free up 12.4 GB" (hanya BACKED_UP + verifikasi hash)
  → [Free up space] → progress → "Done! 12.4 GB freed" → undo 5 detik
  → foto tetap terlihat (thumbnail + BlurHash); tap = stream dari Telegram
```

### 4.5 Restore (HP baru / setelah hapus)
```
HP baru → login QR → "Restore from cloud"
  → pilih scope (All / Last 30 days / Collection) → Progress Hub → file asli masuk galeri
```
- (P1) Restore selektif per foto/collection dari menu konteks.

---

## 5. Fitur Part 2 — "Telephoto Parity"

Legenda: ✅ **TIRU** (adaptasi pola) · 🛡 **PERTAHANKAN** · ⏸ **TUNDA** · ❌ **SKIP**

### 5.1 Ditiru dari Telephoto

| # | Fitur | Keputusan | Detail |
|---|---|---|---|
| T1 | **Task Progress Hub** | ✅ | Satu tempat semua proses (upload/download/delete/scan/restore/OCR): progress bar per task, status per item, retry/pause/resume/cancel. Backend: event stream Rust→JS (throttle §7.9). |
| T2 | **Upload state machine + resume** | ✅ | `pending → uploading → backed_up` + `failed/skipped/paused`; tabel `upload_errors`; resume dari chunk terakhir (§6.2). |
| T3 | **Dedup hash di background** | ✅ | SHA-256 di worker terpisah (`hash_worker`), bukan di thread UI/scan. |
| T4 | **Captions & hashtags** | ✅ | Panel caption (markdown ringan), #tag, pencarian #tag, apply ke multi-item. Caption sinkron sebagai caption pesan Telegram. |
| T5 | **Collections** | ✅ | Album user (lokal & cloud), foto+video, add/remove dari panel foto. |
| T6 | **Memories** | ✅ | Kartu "On this day" (Library + widget P1); query tanggal sama tahun lalu. |
| T7 | **Reel mode** | ✅ | Video vertical continuous + gesture seek/volume/brightness + speed. |
| T8 | **Cloud view options** | ✅ | Group by (date/collection/folder), sort, filter, grid column control (2–6). |
| T9 | **Unified preview** | ✅ | Satu preview untuk foto+video (disempurnakan). |
| T10 | **Encrypted DB backup/restore** | ✅ | Backup metadata+settings terenkripsi (passphrase), validasi pemilik (bind akun), restore merge. |
| T11 | **Home screen widget** | ⏸ P1 | Widget memories + recent photos (Kotlin `AppWidgetProvider` + JNI). |
| T12 | **OCR offline** (Tesseract) | ⏸ P2 | Antrean per foto, traineddata per bahasa (download pilihan), hasil di DB + pencarian teks. |
| T13 | **Face recognition offline** | ⏸ P2 | ML Kit detect + model embedding **ringan (< 20 MB)** + ABI-split wajib. Bukan facenet 90 MB. |
| T14 | **Tab reorder + grid control** | ⏸ P1 | `tab_reorder_sheet` (drag), slider kolom grid. |
| T15 | **Thumbnail shimmer** | ✅ | Skeleton frame saat scroll cepat (tambahan BlurHash). |
| T16 | **Image conversion optional** | ✅ | Opsi "Original" (default, MTProto 2 GB) vs "Compact" (HEIC→JPEG) — beda dari Telephoto yang paksa konversi. |

### 5.2 Dipertahankan (keunggulan kita)

| # | Fitur | Alasan |
|---|---|---|
| K1 | Enkripsi zero-knowledge (XChaCha20-Poly1305 + Argon2id) | Pembeda #1; default ON. |
| K2 | MTProto + akun sendiri + vault channel privat | 2 GB/file, tanpa token bocor, sesi auto-restore (anti-flood OTP). |
| K3 | Auto-backup WorkManager (15 mnt, Wi-Fi/charging, notifikasi) | Ramah baterai & privasi. |
| K4 | Import Google Photos 1-klik | Tidak dimiliki Telephoto. |
| K5 | Scan anti-OOM (thumbnail native Android, dimensi header-only, hash streaming) | Hasil fix kita sendiri. |
| K6 | Free Up Space verifikasi hash | Jangan hapus yang belum terverifikasi. |
| K7 | Galeri real-time (ContentObserver) | Deteksi baru seketika. |
| K8 | Sesi auto-restore cold start | Anti-flood OTP. |

### 5.3 Di-skip / ditunda

| Fitur Telephoto | Keputusan | Alasan |
|---|---|---|
| AI caption/notes (OpenAI/Gemini) | ❌ SKIP | Bertentangan dengan zero-knowledge (keputusan user). |
| Notes tab (markdown + AI) | ⏸ TUNDA | Di luar inti; tanpa AI nilainya rendah. |
| QR contact / barcode | ⏸ P2 | Nilai marginal. |
| Battery optimization exemption | ❌ SKIP | Agresif & menakutkan; WorkManager cukup. |
| MANAGE_EXTERNAL_STORAGE | ❌ SKIP | Over-permission. |
| Bot token mode | ❌ SKIP | Tidak aman, batas 50 MB. |

---

## 6. Spesifikasi Teknis Part 2

### 6.1 Task Progress Hub (T1)
```
Rust (core)                          JS (UI)
  backup/restore/delete/scan/ocr ──► events ──► ProgressHub store ──► UI (badge + layar)
        ▲                                                              │
        └─────────────── ack/retry/pause/resume/cancel ◄───────────────┘
```
- Event: `{task_id, kind, total, done, current_item, status, message}` via Tauri event channel.
- UI: badge "2 uploads running" di topbar → layar `ProgressHub` (daftar task + bar + pause/resume/retry/cancel).
- Backend: `task_hub.rs` — registry task, atomic counter, **persist state** (resume setelah app mati).
- Throttle event ke UI 50 ms/batch (§7.9).

### 6.2 Upload state machine + resume (T2)
```
NOT_BACKED_UP → QUEUED → UPLOADING ⇄ PAUSED
                    ↘ FAILED_RETRY → (exponential backoff, max 5) → QUEUED
                    ↘ SKIPPED (file > limit / invalid / user skip)
UPLOADING → BACKED_UP (message_id + file_id + hash tersimpan)
BACKED_UP → CLOUD_ONLY (free up space)
```
- Tabel `uploads`: `media_id, message_id, file_id, hash_sha256, status, retry_count,
  last_error, uploaded_bytes, total_bytes, created_at, updated_at`.
- Tabel `upload_errors`: `upload_id, error_code, message, at`.
- Resume: simpan `uploaded_bytes` → MTProto `upload.saveBigFilePart` lanjut dari part terakhir.
- **Catatan enkripsi (vault)**: stream cipher XChaCha20 tidak bisa "lompat" ke posisi
  tengah file tanpa menyimpan state cipher. Untuk item terenkripsi, resume = ulangi dari
  awal part-0 (murah karena file lokal masih ada); `uploaded_bytes` tetap dipakai untuk
  progres & dedup part yang sudah dikirim.
- Konflik DB part 1 → migrasi `PRAGMA user_version` (tidak reset data user; backup DB
  otomatis sebelum migrasi).

### 6.3 Captions & hashtags (T4)
- Tabel `captions(id, media_id UNIQUE, text, updated_at)` + `caption_tags(id, media_id, tag, UNIQUE(media_id, tag))`.
- Panel caption: textarea + tag chips + "Apply to selected".
- Pencarian: `LIKE '%#tag%'` (FTS5 caption text di P1).
- Sinkron: caption ikut sebagai caption pesan Telegram (`editMessageCaption` MTProto).

### 6.4 Collections (T5)
- `collections(id, name, cover_media_id, is_cloud, sort_order, created_at)` + `collection_items(collection_id, media_id, added_at)`.
- Cloud collection = folder virtual di metadata DB (bukan struktur Telegram); sinkron antar device via encrypted DB backup/sync (P1).
- UI: Library → Collections → grid; add dari panel foto; drag-reorder.

### 6.5 QR Login (keputusan produk)
- Grammers `qr_login`: QR token ditampilkan, poll status sampai konfirmasi dari app Telegram.
- QR auto-refresh 60 s, valid 1×; sesi disimpan SQLite (auto-restore).
- Fallback: "Use phone number instead" → OTP flow (sudah ada, UX dirapikan).

### 6.6 Encrypted DB backup (T10)
- Ekspor: `media_items` + `uploads` + `captions` + `collections` + settings → JSON → enkripsi
  XChaCha20-Poly1305 (passphrase Argon2id) → simpan ke vault Telegram (`.tphotos-backup`).
- Header file berisi hash akun → saat restore, validasi cocok dengan akun yang login.
- Restore = **merge** (bukan replace): baris baru ditambah, konflik di-resolve via `sha256_hash`.

### 6.7 Database — skema tambahan Part 2 (+ index)
```sql
CREATE TABLE uploads (...);          -- §6.2
CREATE TABLE upload_errors (...);
CREATE TABLE captions (...);
CREATE TABLE caption_tags (...);
CREATE TABLE collections (...);
CREATE TABLE collection_items (...);
CREATE TABLE ocr_queue (...);        -- P2
CREATE TABLE ocr_results (...);      -- P2
CREATE TABLE people (...);           -- P2
CREATE TABLE detected_faces (...);   -- P2

CREATE INDEX idx_uploads_status ON uploads(status);
CREATE INDEX idx_uploads_media ON uploads(media_id);
CREATE INDEX idx_caption_tags_tag ON caption_tags(tag);
CREATE INDEX idx_collection_items_coll ON collection_items(collection_id);
CREATE INDEX idx_media_date_taken ON media_items(date_taken_timestamp DESC);  -- sudah ada Part 1
CREATE INDEX idx_media_sync_status ON media_items(sync_status);               -- sudah ada Part 1
-- Semua query galeri wajib keyset pagination (Part 1 §11.3), bukan OFFSET.
```

### 6.8 Arsitektur service (meniru granularitas Telephoto)
```
app/src-tauri/src/
├── commands.rs            # API surface (existing)
├── task_hub.rs            # NEW: progress hub terpusat (§6.1)
├── upload_manager.rs      # NEW: antrean + state machine + resume (§6.2)
├── hash_worker.rs         # NEW: dedup SHA-256 thread terpisah (§7.2)
├── thumb_pipeline.rs      # NEW: pipeline thumbnail multi-tier (§7.3)
├── device_tier.rs         # NEW: deteksi tier performa device (§7.8)
├── captions.rs            # NEW
├── collections.rs         # NEW
├── qr_login.rs            # NEW: QR auth via Grammers
├── db_backup.rs           # NEW: encrypted DB export/restore
├── telegram/  media.rs  android_media.rs  geo.rs ...  # existing
```
UI: store `ProgressHub`, komponen baru (`ProgressHub`, `CaptionPanel`, `CollectionSheet`,
`MemoryCard`, `ReelViewer`, `Shimmer`, `BackupBanner`, `StatusBadge`), virtualized grid (§7.4).

---

## 7. Performance Engineering (baru — sekelas Telephoto)

> Prinsip: **setiap fitur punya budget angka; setiap budget punya alat ukur; setiap
> milestone wajib verifikasi** (target di §2.3). Menggabungkan blueprint §11 Part 1
> dengan pola performa Telephoto (`thumbnail_pipeline_service`, `photo_thumb_cache_service`,
> `background_hash_service`, `device_performance_tuning`, `task_progress_hub`).

### 7.1 Thumbnail pipeline multi-tier (menyatukan Part 1 §11.1 + pola Telephoto)

Tier thumbnail — **semua hasil decoder native Android / Rust, tanpa decode penuh**:

| Tier | Ukuran | Format/ukuran file | Dipakai untuk | Sumber |
|---|---|---|---|---|
| 0 | — | **BlurHash** (16–32 char, di DB) | Placeholder 0 ms saat scroll | Part 1 (sudah) |
| 1 | 96 px | JPEG ≈3–6 KB | Grid 5–8 kolom (heatmap/bulanan) | decoder native |
| 2 | 256 px | JPEG ≈15–25 KB | Grid 1–3 kolom (standar) | decoder native (sudah dipakai scan) |
| 3 | 1200 px | JPEG ≈100–200 KB | Lightbox / preview (generated on demand, di-cache) | decode sekali, bukan tiap buka |

Aturan pipeline:
- **Lazy generation**: tier 2 dibuat saat scan (sekali); tier 1 & 3 dibuat **on-demand**
  saat grid butuh / foto dibuka — bukan semua di depan (hemat waktu scan & disk).
- **Queue + worker**: `thumb_pipeline.rs` antrean berprioritas (visible > prefetch > idle),
  concurrency dibatasi oleh device tier (§7.8), **tidak pernah** di thread UI.
- **Cache disk** `thumbs/`: nama file = `sha256[:16] + tier`, dedup otomatis; LRU eviction;
  cleanup berkala (P0: saat app idle; P2: ukuran maks cache konfigurable).
- **Video**: thumbnail dari frame pertama (MediaMetadataRetriever native), pipeline terpisah
  (`video_thumb_cache`) — sama seperti Telephoto yang pisah `photo_thumb_cache` & `video_thumb_cache`.

### 7.2 Dedup hash background (`hash_worker`)
- SHA-256 streaming (buffer 1 MB) di worker terpisah dari scan & UI.
- Prioritas antrean: file baru (harus di-hash sebelum upload) > file belum di-hash.
- **Cache hash** di DB (index `idx_media_sha256`): file yang sama (hash sama) tidak di-hash ulang.
- Budget: hash 1 GB < 10 s di mid-range (native, streaming).

### 7.3 Memori (menyempurnakan Part 1 §11.2 untuk stack kita)
- **Image cache LRU terikat**: 64–128 MB RAM; hook `TRIM_MEMORY_RUNNING_LOW` → flush.
- **Hardware downsampling**: tidak pernah decode > ukuran yang dibutuhkan tier.
- **WebView/DOM budget**: node grid ≤ ~200 tile ter-render (virtualisasi §7.4); image
  ditampilkan via object URL / `createImageBitmap` (decode off main thread), direvoke saat tile lepas.
- Heap Android default cukup (256 MB): scan anti-OOM v1.2 dipertahankan (tanpa decode penuh).
- **BlurHash sebagai placeholder instan** — grid tidak pernah kosong/putih saat scroll cepat.

### 7.4 Virtualisasi grid (spesifik Tauri/WebView + React)
- **Windowed rendering** (pola react-window): render hanya item visible + buffer 1 layar
  atas/bawah; recycle DOM node (bukan create/destroy) saat scroll.
- Row/kolom dihitung dari `date_taken_timestamp` via **keyset pagination** (Part 1 §11.3) —
  bukan `OFFSET`.
- **Sticky header + scrubber** dari tabel agregasi `date_groups_summary` (Part 1 §11.3) —
  tanpa full scan.
- Badge status diambil dari kolom DB (sudah indexed), **bukan** fetch per item.
- Scroll guard: saat user scroll, prioritas render = visible tiles; thumbnail tier 1
  di-prefetch hanya 1 layar (2 layar di high-end).

### 7.5 Database (Part 1 §11.3 + tabel baru)
- PRAGMA: WAL, `synchronous=NORMAL`, `cache_size=-8000`, `temp_store=MEMORY`,
  `mmap_size=256 MB` (sudah diterapkan v1).
- Keyset pagination wajib di semua query galeri, uploads, collections.
- Index baru tabel Part 2 (§6.7). `uploads(status)` untuk query antrean O(1).
- Tabel `date_groups_summary` di-maintain incremental (bukan rebuild tiap scan).
- Migrasi v1→v2 via `PRAGMA user_version`, bertahap, backup dulu.

### 7.6 Jaringan & streaming (Part 1 §11.4 + cache cloud)
- Adaptive chunk MTProto: 512 KB–1 MB (5G/Wi-Fi), 128–256 KB (seluler) — sudah ada.
- Pipelined upload window 3–4 chunk; concurrency diatur §7.8.
- **Cloud preview cache** (`download_cache`): LRU disk, maks ~512 MB, evict by recency;
  preview 1200 px di-cache setelah streaming pertama.
- Video: range-request buffer 5 s; seek < 500 ms (target §2.3).
- Upload throughput target > 80% link tanpa FloodWait (log flood counter di `upload_errors`).

### 7.7 Background & baterai (WorkManager)
- Satu run worker = pipeline utuh: **scan → hash → thumb tier2 → upload** (batch), bukan job terpisah.
- Constraint: Wi-Fi only (default) + charging opsional — sudah ada; **thermal/baterai <20%
  pause otomatis** (Part 1 §11.5).
- Upload concurrency: **1 stream default** (flood-safe), 2 di high-end + Wi-Fi.
- Notifikasi hanya ringkas saat batch selesai / error (bukan per item) — §8.
- Budget idle: < 1%/jam (§2.3).

### 7.8 Device performance tiers (meniru `device_performance_tuning` Telephoto)
Deteksi via `ActivityManager.MemoryInfo` + jumlah core + API level, di-cache `device_tier.rs`:

| Tier | RAM | Perilaku |
|---|---|---|
| **Low** | ≤ 3 GB | Micro thumb only di grid cepat; 1 worker upload; reel autoplay OFF; prefetch 0 layar; tanpa animasi berat |
| **Mid** | 4–6 GB | Tier 1+2; 1–2 worker; prefetch 1 layar; animasi standar |
| **High** | ≥ 8 GB | Semua tier; 2 worker; prefetch 2 layar; GPU decode; reel autoplay ON |

- Tier dipakai: concurrency upload, kedalaman prefetch, ukuran image cache, ambang animasi.
- Override manual di Settings → Advanced (opsional, tersembunyi).

### 7.9 Event & UI throttling
- Progress hub events di-batch 50 ms sebelum dikirim ke JS (ribuan event/detik → ≤ 20/s di UI).
- Update DOM pakai microtask/`requestAnimationFrame`; tidak pernah render progres > 10×/detik.
- Logging hanya di debug build; release zero log hot-path (perf & privasi).

### 7.10 Verifikasi performa (wajib tiap milestone)
1. **Synthetic 100k**: generator data uji (100k baris + file dummy) → ukur first paint,
   scroll fps, RAM (Perfetto/dumpsys).
2. **Scan massal**: 1.000 foto asli → < 30 s, tanpa OOM (emulator + device RAM rendah).
3. **Upload**: 1 GB via 5G sim (network conditioner) → throughput, flood counter, resume
   saat koneksi putus (adb `svc data disable` di tengah upload).
4. **Baterai**: idle 24 jam (batterystats) → < 1%/jam.
5. **Cold start**: `am start -W` di device mid-range → < 500 ms.

---

## 8. Notifikasi & Komunikasi (spesifikasi copy)

| Situasi | Notifikasi | Copy (EN) |
|---|---|---|
| Batch backup selesai | 1 notifikasi ringkas | "12 photos backed up" (tap → Photos) |
| Ada item gagal | 1 notifikasi (bukan per item) | "3 photos couldn't back up — tap to retry" |
| Backup berjalan lama | Status di Progress Hub saja (tanpa spam notif) | — |
| Free up space selesai | 1 notifikasi | "12.4 GB freed — your photos are still in the cloud" |
| Restore selesai | 1 notifikasi | "Your photos are back on this device" |
| Auto-backup off + foto baru | 1×/hari maks | "New photos are waiting — turn on auto backup" |

- Channel notifikasi: `backup` (default), `errors`, `system`.
- Semua copy pendek, kata sehari-hari, tanpa error code mentah.

---

## 9. Screen Specs (wireframe ringkas)

### 9.1 Photos (timeline)
```
┌───────────────────────────────┐
│ [☰]  Photos          [⚙] [⏣] │  topbar: menu, progress badge
│ ┌───────────────────────────┐ │
│ │ ⬆ Backing up 12 photos 45%│ │  banner (tap → Progress Hub)
│ └───────────────────────────┘ │
│ 📅 August 2026        (sticky)│
│ [🔵][⏳][🔵][📱][🔵][🔵]      │  grid virtualized, badge status
│ [🔵][⚠][🔵][🔵][🔵][🔵]      │
│ 📅 July 2026         (sticky)│
│ [🔵][🔵][🔵][🔵][🔵][🔵]      │
│            [scrubber ▓]      │  fast date scrubber
└───────────────────────────────┘
```

### 9.2 Progress Hub
```
┌───────────────────────────────┐
│ ← Tasks                       │
│ Uploading — 12/345 (45%)  [⏸] │  ████████░░░░
│   IMG_0123.jpg  [retry]       │
│   IMG_0124.jpg  [retry]       │
│ Downloading — 3/10 (30%)  [⏸] │  ███░░░░░░░░░
│ Free up space — 2.1/12.4 GB   │  ██░░░░░░░░░░
└───────────────────────────────┘
```

### 9.3 Search
```
┌───────────────────────────────┐
│ 🔍 Search photos, #tags…      │
│ [Videos][⭐Fav][🖼 Screenshots]│
│ [RAW][📅 Last 30 days]        │
│ ─── Results (grouped) ───     │
│ #beach  (23)  #family (41)    │
│ 📍 Bali (12)   📍 Jakarta (8) │
└───────────────────────────────┘
```

### 9.4 Collections / Library
```
┌───────────────────────────────┐
│ Library                       │
│ [➕ New collection]            │
│ ┌────┐ ┌────┐ ┌────┐ ┌────┐  │
│ │Coll│ │Fav │ │Mem │ │Trsh│  │
│ └────┘ └────┘ └────┘ └────┘  │
│ Folders: Camera, WhatsApp, …  │
└───────────────────────────────┘
```

### 9.5 Settings — backup section
```
┌───────────────────────────────┐
│ ← Settings                    │
│ Backup status                 │
│  ██████████░░  3,420 backed up │
│  12.4 GB in cloud             │
│  [Back up now]  [Free up space│
│ ────────────────────────────  │
│ Auto backup        [ON]       │
│   Wi-Fi only       [ON]       │
│   While charging   [OFF]      │
│   Folders: Camera[ON] WhatsApp│
│ Encryption: Active 🔒         │
└───────────────────────────────┘
```

---

## 10. Testing & QA Plan

| Area | Metode |
|---|---|
| Matrix device | Emulator API 24 / 29 / 34 (ARM + x86); device nyata RAM 2–8 GB |
| Data skala | Generator synthetic 100k foto (P0 tooling, dipakai juga utk §7.10) |
| Jaringan | Network conditioner: flaky, offline tengah upload, 3G/4G/5G/Wi-Fi, FloodWait simulasi |
| Migrasi DB | v1→v2 dengan data lama; verifikasi tidak ada data hilang (hash count sama) |
| Crash/stability | Monkey test 30 mnt + crash-free sessions > 99.5% |
| Performa | §7.10 checklist wajib tiap milestone |
| UX awam | Usability test 5 awam: setup tanpa instruksi; paham status badge |
| Keamanan | Tidak ada token/passphrase di log; enkripsi verifikasi (decrypt round-trip test) |

---

## 11. Roadmap Part 2 (3 fase)

| Fase | Isi | Estimasi |
|---|---|---|
| **P0 — v2.0 "Awam Core"** | IA baru (4 tab) · QR login onboarding · backup banner + badge · **Task Progress Hub** · upload state machine + resume + upload_errors · hash worker · **device_tier** · **thumb pipeline lazy** · captions & hashtags · collections dasar · unified preview polish · encrypted DB backup · migrasi schema v1→v2 · **perf baseline (7.10: scan, cold start, RAM)** | 3 minggu |
| **P1 — v2.1 "Delight"** | Memories · Reel mode · cloud view options + grid control · tab reorder · widget home screen · restore selektif · FTS5 caption search · **perf re-verify (scroll 100k)** | 2 minggu |
| **P2 — v2.2 "Smart & Offline"** | OCR offline (Tesseract per-bahasa) · face recognition (model < 20 MB, ABI-split) · people grouping · QR contact · **perf re-verify (OCR/face tidak mengganggu scroll)** | 3 minggu |

**Urutan P0 wajib**: IA → QR login → progress hub → state machine → badge/banner
(badge tanpa state machine = status bohong) → perf baseline di akhir P0.

---

## 12. Non-Goals (v2)

- ❌ AI cloud (OpenAI/Gemini) — skip permanen.
- ❌ Notes tab. ❌ Bot token mode.
- ❌ Izin over-permission (MANAGE_EXTERNAL_STORAGE, battery exemption).
- ❌ iOS — Android-first; ditunda sampai v2 stabil.
- ❌ Face recognition model besar (facenet 90 MB) — wajib model ringan di P2.

## 13. Risiko & Mitigasi

| Risiko | Mitigasi |
|---|---|
| Migrasi schema v1→v2 merusak DB | `PRAGMA user_version` + backup DB otomatis sebelum migrasi + uji data lama |
| QR login gagal di device tertentu | Fallback OTP penuh; QR minSdk 24 aman |
| Badge memperlambat grid 100k | Badge dari kolom indexed, virtualisasi, render hanya tile visible |
| Progress hub event overload | Batch 50 ms, update ≤10×/detik |
| Resume upload enkripsi (cipher stream) | Restart part-0 untuk item terenkripsi (§6.2); uji mati-listrik |
| User awam tidak paham enkripsi | Default ON + satu baris: "Only you can read your photos" |
| Thumbnail disk membengkak | LRU eviction + cleanup idle + ukuran maks konfigurable |
| Scroll jank di device low-end | Device tier: micro-thumb only, prefetch 0, animasi dikurangi |

## 14. Ringkasan Keputusan

1. IA = 4 tab gaya Google Photos (Photos/Search/Library/Settings).
2. Fitur Telephoto ditiru **polanya** (progress hub, collections, captions, memories,
   reel, view options, OCR/face lokal bertahap) — bukan kodenya (tanpa lisensi).
3. Keunggulan kita (enkripsi, MTProto 2 GB, WorkManager, Google import, scan anti-OOM)
   dipertahankan.
4. AI cloud di-skip; UI Inggris (i18n-ready); QR login utama + OTP fallback.
5. **Performa = persyaratan, bukan harapan**: budget §2.3, strategi §7, verifikasi §7.10
   di tiap milestone P0→P1→P2.
