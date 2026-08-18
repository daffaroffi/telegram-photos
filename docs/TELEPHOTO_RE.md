# Reverse Engineering: Telephoto v69.0.0

> Analisis dari `snipoff/app-release.apk` (256 MB, `com.asrumon.telephoto`, v69.0.0).
> Metode: ekstraksi APK + `aapt2` (manifest) + analisis binary `libapp.so` (string/struktur kode Dart AOT).
> Semua klaim bertanda **terkonfirmasi** berasal dari bukti langsung di APK; sisanya inferensi yang masuk akal.
> Catatan lisensi: repo Telephoto **tanpa lisensi** (all rights reserved) — dokumen ini hanya untuk pembelajaran, **tidak boleh menyalin kode/aset mereka**.

---

## 1. Ringkasan eksekutif

Telephoto adalah aplikasi **Flutter** yang backup foto/video ke **chat Telegram via Bot API** (token bot + chat ID), bukan MTProto. Ini keputusan arsitektur paling fundamental yang membedakannya dari TelegramPhotos:

| Aspek | Telephoto | TelegramPhotos (kita) |
|---|---|---|
| Transport | Bot API (`api.telegram.org`) | MTProto (Grammers) |
| Kredensial | Token bot + chat ID (disimpan di device) | Login akun Telegram sendiri (session tersimpan) |
| Penyimpanan | Chat bot (bisa user ID atau group ID) | Channel privat milik user |
| Enkripsi | **Tidak ada** — file dikirim plaintext | XChaCha20-Poly1305 + Argon2id (zero-knowledge) |
| Framework | Flutter + Dart | Tauri 2 + Rust + WebView |
| UI native | Semua UI di Dart (Material 3) | WebView + React |
| Background | Foreground service + alarm manager | WorkManager + JNI ke Rust |

**Kesimpulan awal:** Telephoto unggul jauh di **cakupan fitur & polish UX** (v69 vs kita masih v1), tapi lemah di **keamanan & privasi** (plaintext, token bot, AI cloud). Keunggulan kita yang harus dipertahankan: enkripsi zero-knowledge + akun sendiri + backend Rust native.

---

## 2. Identitas & distribusi

- Package: `com.asrumon.telephoto`, versionName `69.0.0`, versionCode `1`
- minSdk **24**, targetSdk **36** (compileSdk 36, platformBuild 16)
- APK **universal** (3 ABI: arm64-v8a, armeabi-v7a, x86_64) — 256 MB
- Repo GitHub: hanya README + pubspec.yaml (44 KB, **tanpa lisensi, tanpa source**)
- Ada endpoint update in-app: `https://info.asrumon.workers.dev/telephoto` (terkonfirmasi)
- MainActivity menerima intent: `MAIN/LAUNCHER`, `VIEW` (browserable — buka file langsung), `SEND` (share ke app), `PROCESS_TEXT` (OCR dari seleksi teks), `GET_CONTENT`

## 3. Komposisi ukuran (terkonfirmasi)

| Komponen | Ukuran | Isi |
|---|---|---|
| `lib/` | 146 MB | Flutter engine + app (Dart AOT) ×3 ABI + native ML |
| `assets/flutter_assets/` | 91 MB | **`facenet.tflite` 90 MB** (model face embedding), icon, OCR config |
| `assets/models_bundled/` | 4.2 MB | ML Kit fssd face detection models (4 varian) |
| `assets/mlkit_barcode_models/` | 868 KB | ML Kit barcode SSD model |
| `classes.dex` + `classes2.dex` | 4.3 MB | Kode Java/Kotlin (ter-obfuscate R8) |
| `res/` | 2.8 MB | Resource ter-obfuscate (semua UI sebenarnya di Dart) |

**Pelajaran:** 256 MB itu mahal. `facenet.tflite` (90 MB) + model fssd + tesseract ×3 ABI. Kalau kita mau face recognition, pakai model kecil + ABI split (arm64 saja = ~85 MB). Tapi untuk produk "photo backup", 256 MB masih wajar di era 128–512 GB storage.

## 4. Library native (terkonfirmasi dari `lib/`)

| Library | Fungsi |
|---|---|
| `libflutter.so` + `libapp.so` + `libdartjni.so` | Flutter engine + kode Dart AOT |
| `libtesseract.so` + `libleptonica.so` + `libjpeg.so` + `libpngx.so` | **OCR offline** (Tesseract 4) |
| `libface_detector_v2_jni.so` + `libbarhopper_v3.so` | **Face detection** (ML Kit face detector JNI) |
| `libtensorflowlite_jni.so` + `libtensorflowlite_gpu_jni.so` | **Face embedding** — jalankan `facenet.tflite` |
| `libdatastore_shared_counter.so` | AndroidX DataStore (dari dependency) |

**Alur face recognition mereka (rekonstruksi):** ML Kit FaceDetector (fssd) → deteksi wajah di foto → crop → `facenet.tflite` via TFLite → embedding 128-d → bandingkan dengan embedding tersimpan (tabel `people`/`person_links`) → group.

**Alur OCR mereka:** Tesseract offline; daftar 100+ bahasa di `OCR.csv` (berisi URL download `traineddata` dari GitHub tesseract-ocr) — artinya **traineddata di-download saat runtime**, hanya bahasa yang dipilih user. `tessdata_config.json` kosong (`"files": []`).

## 5. Struktur arsitektur (rekonstruksi dari string `libapp.so`)

Package utama `_photos_flutter/` — semua file terkonfirmasi:

```
_photos_flutter/
├── main.dart
├── models/          # cloud_photo, photo, video, settings, unified_preview_item,
│                    # ocr_language, widget_payload
├── notifiers/       # update_notifier (state management berbasis notifier)
├── screens/         # 10 layar:
│   ├── gallery_screen           # tab lokal (MediaStore)
│   ├── cloud_preview_screen     # tab cloud (isinya dari Telegram)
│   ├── unified_preview_screen   # preview gabungan foto+video
│   ├── preview_screen           # lightbox per item
│   ├── video_player_screen      # player + reel mode
│   ├── people_screen            # tab People (face recognition)
│   ├── note_detail_screen       # tab Notes (markdown + AI)
│   ├── settings_screen
│   ├── task_progress_screen     # progress hub semua task
│   └── (search/view options)
├── services/        # 30+ service terpisah — lihat §6
└── widgets/         # photo_tile, thumbnail_shimmer, thumbnail_wheel,
                     # tab_reorder_sheet, caption_panel, image_details_panel,
                     # extracted_text_sheet, add_to_collection_sheet,
                     # add_videos_to_collection_sheet
```

### 6. Service map (30+ service — ini arsitektur inti mereka)

**Scan & sinkronisasi lokal:**
- `media_store_image_scanner` / `media_store_video_scanner` — scan MediaStore (dipisah foto/video)
- `video_observer_manager` — observer MediaStore untuk video baru ("observer started/stopped", `observerDurationHours`)
- `background_scan_service` — scan di background
- `gallery_sync_service` — sinkronisasi galeri
- `storage_service` — manajemen file lokal

**Dedup & hash:**
- `background_hash_service` — hash file (sha256, terkonfirmasi) di background/thread terpisah
- `uploaded_service` — status "sudah upload" (tabel `uploads`)

**Pipeline upload:**
- `upload_manager` — antrean upload
- `backup_service` — orchestrasi backup (manual/smart sync)
- `image_conversion_service` — konversi/kompresi sebelum upload (HEIF→JPEG via `heif_converter`, "compressed copy")
- `thumbnail_pipeline_service` + `photo_thumb_cache_service` + `video_thumb_cache_service` — thumbnail pipeline terpisah

**Cloud (Telegram):**
- `telegram_service` — semua panggilan Bot API
- `cloud_download_manager` — download dari cloud + `download_service` + `download_cache` (tabel)
- `cloud_delete_manager` — hapus cloud (Bot API `deleteMessage`, terkonfirmasi)
- `cloud_cache_service` — cache item cloud

**Pintar (OCR/face/AI):**
- `ocr_queue_manager` + `ocr_service` + `cloud_ocr_cache_service` + `ocr_models_service` — pipeline OCR antrean per-foto
- `face_recognition_service` + `face_crop_cache_service` — face pipeline
- `ai_caption_service` — caption otomatis (OpenAI + Gemini, terkonfirmasi)

**Sistem:**
- `auto_sync_work_service` — sinkronisasi terjadwal
- `task_progress_hub` — hub progres terpusat (semua task lapor ke sini)
- `progress_navigation_service` — navigasi ke layar progres
- `device_performance_tuning` — tuning berdasarkan device
- `update_service` — cek update in-app
- `widget_data_service` — data untuk home screen widget
- `intent_service` — handle intent (VIEW/SEND/PROCESS_TEXT)
- `settings_service` — preferensi
- `qr_scan_service` + `qr_contact_service` — scan QR + simpan kontak
- `video_library_service` — library video

**Pelajaran arsitektur untuk kita:**
1. **Pemisahan service sangat granular** — tiap concern punya service sendiri (scan, hash, thumbnail, upload, cloud, OCR, face). Mudah di-test & di-maintain.
2. **Task progress hub terpusat** — semua pipeline (upload/delete/OCR/scan) lapor ke satu hub → UI progress konsisten. Kita belum punya ini.
3. **Pipeline terpisah per jenis** — thumbnail pipeline terpisah dari upload; scan terpisah dari hash. Kita sudah mulai (scan anti-OOM), tapi belum segranular ini.

## 7. Model data (terkonfirmasi dari string SQL)

```
uploads          → message_id, chat_id, file_id, hash (sha256), upload_status,
                   is_uploaded, local_path, (status: pending/uploaded/failed/
                   processing/skipped/completed/paused/deleting/downloading)
thumbnails       → (cache thumbnail)
video            → (metadata video)
collections      → collection_id (+ collection_images, collection_videos)
caption          → (+ caption_image_links) — captions & hashtags (kolom `hashtags`)
cloud_ocr        → hasil OCR per foto cloud
ocr_queue        → antrean OCR per foto
people           → person_id (+ person_links) — grouping wajah
detected_faces   → face_scan_state — state scan wajah
upload_errors    → error upload
download_cache   → cache download cloud
```

**Pelajaran:**
- Dedup berbasis **hash sha256** (kita juga, tapi mereka pakai `background_hash_service` agar tidak nge-block UI — kita perlu cek thread kita).
- **`upload_status` state machine eksplisit** dengan banyak state — kita perlu ini untuk resume/pause.
- **Tabel `upload_errors` terpisah** — retry & diagnosa. Kita belum punya.
- Caption + hashtags sebagai tabel relasi — bukan kolom di foto.

## 8. Perilaku upload (terkonfirmasi + inferensi kuat)

- Method Bot API terkonfirmasi di binary: `sendPhoto`, `sendVideo`, `sendDocument`, `sendMediaGroup`, `sendMessage`, `getFile`, `getUpdates`, `deleteMessage`
- **Limit:** "Skipped doc >48MB" — **file >48 MB di-skip** (batas Bot API 50 MB, mereka ambil margin). "exceeds 10MB limit" — ada batas 10 MB untuk jalur tertentu (kemungkinan preview/kompresi via `sendPhoto`).
- **Konversi sebelum upload:** `image_conversion_service` — "compressed copy", HEIF→JPEG (lib `heif_converter`). README menyebut "Image pre-conversion before upload — improves compatibility and upload reliability".
- **Dedup:** `background_hash_service` (sha256) → hanya file baru/missing yang di-upload ("Smart Sync").
- Download: `api.telegram.org/file/bot<TOKEN>/<file_path>` (terkonfirmasi).

**Pelajaran untuk kita:**
- Bot API terbatas 50 MB/file → mereka skip >48MB. **Kita pakai MTProto: limit file Telegram = 2 GB (premium 4 GB)** — keunggulan nyata kita untuk video besar. Ini selling point.
- Konversi/kompresi sebelum upload = keputusan UX yang bagus (hemat kuota, upload lebih andal) — tapi konflik dengan "backup original". Kita bisa tawarkan opsi: original (MTProto, tanpa batas 50MB) vs compressed.

## 9. Fitur pintar (rekonstruksi)

| Fitur | Implementasi mereka | Catatan |
|---|---|---|
| Face recognition | ML Kit detect + facenet.tflite embedding + tabel people | Offline penuh, model 90 MB |
| OCR | Tesseract, traineddata di-download per bahasa | Offline, antrean per foto, hasil di `cloud_ocr` |
| AI caption | `api.openai.com/v1/chat/completions` + Gemini `gemini-3.1-flash-lite` | **Cloud** — kontradiksi dengan "privacy-first" |
| Barcode/QR | ML Kit barcode | Dipakai untuk QR contact |
| Notes | Markdown + AI assistance | Fitur unik, di luar scope photo backup |
| Memories | Kartu "kenangan" | Sama konsepnya dengan Google Photos |
| Reel mode | Video vertikal continuous | `video_player_screen` + gesture (seek, volume, brightness) |

**Catatan penting:** fitur AI mereka (caption/notes) **cloud-based** — foto dikirim ke OpenAI/Gemini. Ini justru memperkuat posisi kita: TelegramPhotos bisa jadi alternatif "benar-benar privat" dengan AI lokal atau opt-in.

## 10. Pola UI/UX yang layak ditiru

Dikonfirmasi dari nama file/widget + string:

1. **`tab_reorder_sheet`** — user bisa **reorder tab** navigasi bawah. Ini fleksibilitas UX jarang ada; murah untuk ditiru.
2. **`thumbnail_wheel`** — wheel thumbnail untuk scrub cepat di preview (seperti timeline Google Photos).
3. **`unified_preview_screen`** — satu preview untuk semua jenis media (foto+video), bukan screen terpisah.
4. **`caption_panel` + `image_details_panel`** — panel caption/hashtag + info EXIF di satu tempat.
5. **Grid control 2–6 kolom** — user mengatur kepadatan grid.
6. **Cloud view options** — group/sort/filter lengkap untuk tab cloud (`CloudViewOptions`, `GalleryViewMode`, `GallerySortBy`, `GalleryGroupBy`).
7. **`thumbnail_shimmer`** — skeleton loading (UX halus saat scroll ribuan foto).
8. **`task_progress_screen`** — layar progres terpusat untuk semua task background.
9. **Widget home screen** (Android 16+, `TelephotoCollectionWidgetProvider` + `WidgetRefreshReceiver`) — memories & foto terbaru di home.
10. **Gesture video player** — swipe untuk seek/volume/brightness, `playbackSpeed`.
11. **Encrypted DB backup** — backup database dengan password + proteksi "Bot ID does not match the encrypted database backup" (validasi pemilik).

## 11. Background & sistem

- **Foreground service** (`flutter_foreground_task`) + **exact alarm** (`androidalarmmanager`) + **reboot receiver** — upload/OCR jalan terus walau app tertutup.
- `REQUEST_IGNORE_BATTERY_OPTIMIZATIONS` — minta dikecualikan dari doze (agresif tapi efektif untuk auto-backup).
- `SCHEDULE_EXACT_ALARM` + `RECEIVE_BOOT_COMPLETED` — jadwal & resume setelah reboot.
- WorkManager juga ada (`androidx.work`) tapi peran utamanya foreground service.
- `MANAGE_EXTERNAL_STORAGE` ("All files access") + `READ_MEDIA_VISUAL_USER_SELECTED` + `CAMERA` + `ACCESS_MEDIA_LOCATION` — izin sangat agresif (semua file + lokasi EXIF). Ini trade-off UX: lebih mudah, tapi menakutkan di mata user privacy-aware.

**Pelajaran:** strategi background mereka = **foreground service** (pasti jalan) + alarm (terjadwal) + battery exemption. Kita pakai WorkManager 15 menit yang lebih ramah baterai & privasi — tapi kalau auto-backup dianggap "tidak pernah jalan", opsi foreground service dengan notifikasi progres layak dipertimbangkan (dengan izin eksplisit user).

## 12. Kelemahan yang terkonfirmasi

1. **Tidak ada enkripsi** — foto dikirim plaintext ke Telegram via Bot API. Telegram bisa baca; siapa pun dengan token bot bisa baca.
2. **Token bot di device** — single point of failure; token bocor = semua foto bisa diakses. Tidak ada proteksi passphrase.
3. **Batasan 50 MB Bot API** — video besar di-skip (">48MB skipped").
4. **Izin agresif** — MANAGE_EXTERNAL_STORAGE, CAMERA, WRITE_SETTINGS, lokasi — over-permission untuk app backup.
5. **AI cloud** — caption/notes kirim foto ke OpenAI/Gemini (kontradiksi privacy-first; user tidak tahu kapan).
6. **Ukuran 256 MB** — model face 90 MB ×3 ABI tanpa split.
7. **versionCode 1 di v69** — kemungkinan rilis ulang/rebuild, bukan increment (atau mereka sengaja reset; tidak masalah teknis).
8. **Closed source, tanpa lisensi** — tidak bisa diaudit, tidak bisa fork.

## 13. Rekomendasi untuk TelegramPhotos (prioritas)

**Tiru (pola, bukan kode):**
1. **Task progress hub + layar progres terpusat** — semua pipeline (upload/delete/scan/restore) lapor ke satu tempat. Kita belum punya; ini pengaruh UX besar.
2. **`upload_errors` + state machine upload eksplisit** (pending/uploading/failed/skipped/completed/paused) — dasar resume & diagnosa.
3. **Dedup hash di thread terpisah** (background_hash_service) — jangan nge-block UI.
4. **Unified preview + caption panel + image details panel** — satu tempat untuk semua info & aksi.
5. **Tab reorder + grid column control** — UX fleksibilitas murah.
6. **Encrypted DB backup** — kita sudah enkripsi media; backup DB settings juga layak.
7. **Thumbnail shimmer / skeleton** — polish scroll ribuan item.
8. **Konversi opsional sebelum upload** — opsi "kompres" vs "original" (kita bisa original penuh via MTProto — keunggulan).

**Jangan tiru:**
- Bot API (kita sudah lebih baik dengan MTProto: 2 GB/file, akun sendiri, tanpa token bocor).
- AI caption cloud default (kontradiksi privasi; jadikan opt-in eksplisit kalau kita tambah).
- Izin agresif (MANAGE_EXTERNAL_STORAGE dsb) — pertahankan permission minimal + MediaStore.
- Model face 90 MB tanpa ABI split (kalau kita tambah face recognition, pakai model kecil / download opsional).

**Roadmap kandidat (pembeda kita):**
1. Enkripsi zero-knowledge (sudah) + **DB backup terenkripsi** + verifikasi integritas.
2. **Progress hub** + resume/pause upload.
3. **Captions & hashtags** (pencarian #tag).
4. **Memories** (kartu kenangan).
5. **Reel mode** video.
6. (Opsional, jangka panjang) OCR offline — model kecil, download per bahasa seperti mereka.

---

*Dokumen ini dibuat dari analisis statis APK v69.0.0. Untuk verifikasi perilaku runtime (mis. alur upload), bisa diuji di emulator dengan mitm/network log.*
