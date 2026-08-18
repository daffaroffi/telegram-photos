# Migrasi Arsitektur: Tauri/WebView → Flutter UI + Rust Core

> **Keputusan (2026-08-18):** UI pindah ke **Flutter native**, backend Rust
> (MTProto/enkripsi/SQLite/backup engine) **dipertahankan** dan di-expose via
> **flutter_rust_bridge (FRB)**. Pola ini sama dengan Telephoto (Flutter + native core)
> dan sesuai rekomendasi PRD Part 1 §7 ("Flutter atau React Native" untuk UI mobile).
>
> **Mengapa:** WebView punya langit-langit untuk kategori app ini — baseline RAM
> Chromium ~100 MB+ (KPI kita 80–150 MB), grid 100k foto + 60fps lebih sulit,
> video 4K butuh ExoPlayer, gesture lebih mulus di native, dan toolchain Android
> Flutter jauh lebih matang (kita sudah merasakan friction Tauri mobile:
> gradle spawn `npm.bat`, assets tidak ter-copy, signing manual).

---

## 1. Target arsitektur

```
┌─────────────────────────────────────────────────────┐
│  Flutter UI (Dart)                                   │
│  ├── 4 tab (Photos/Search/Library/Settings)          │
│  ├── Grid virtualized (GridView.builder)             │
│  ├── Onboarding QR login                             │
│  └── Task Progress Hub, badge status, dll            │
└──────────────┬──────────────────┬───────────────────┘
               │ FRB (FFI, async) │ PlatformChannel
┌──────────────▼──────────────────▼───────────────────┐
│  Rust Core (dipertahankan, 5.2k baris)               │
│  ├── telegram/  (Grammers MTProto, QR login)         │
│  ├── crypto.rs (XChaCha20-Poly1305 + Argon2id)       │
│  ├── db.rs     (SQLite WAL + schema v1→v2)           │
│  ├── backup/   (upload engine, task hub baru)        │
│  ├── media.rs  (hash, EXIF, thumbnail pipeline)      │
│  ├── geo.rs    (reverse geocode offline)             │
│  └── google.rs (Google Photos import)                │
└──────────────────────────┬───────────────────────────┘
                           │ JNI (tetap, untuk Android)
┌──────────────────────────▼───────────────────────────┐
│  Kotlin (diadaptasi)                                  │
│  ├── MediaStore scan + thumbnail native               │
│  ├── ContentObserver (real-time)                      │
│  ├── WorkManager auto-backup                          │
│  └── Notifikasi + (P1) widget                         │
└───────────────────────────────────────────────────────┘
```

- **Rust** tetap di-compile sebagai `cdylib` untuk Android (arm64/armv7/x86_64) — sama seperti sekarang.
- **Kotlin JNI** tetap dipakai untuk MediaStore/WorkManager (sudah terbukti bekerja); jembatan
  Rust→Kotlin tetap `JNI_OnLoad` + GlobalRef (fix force-close yang sudah dikerjakan).
- **Yang berubah:** React/TS (WebView) → Dart (Flutter); `tauri::command` → FRB; Tauri event → Dart stream.

## 2. Kondisi saat ini (fakta)

| Lapisan | Baris | Status | Aksi |
|---|---|---|---|
| Rust core | 5.187 | Jalan, teruji (cargo test) | **Pertahankan**; tambah lapisan FRB |
| commands.rs | ~450+ (Tauri wrapper) | Terikat `tauri::command`/`State` | **Refactor**: pisah core logic vs wrapper |
| React UI | 2.299 | 7 komponen | **Tulis ulang** di Dart |
| Kotlin | 1.857 | Jalan (MediaPlugin, scheduler, worker) | **Adaptasi**: MediaPlugin tetap, tambah channel |
| Android build | gradle + task rust | Jalan dengan workaround | Ganti ke build FRB + Flutter toolchain |

## 3. Rencana kerja (5 fase)

### Fase 1 — Decouple core dari Tauri (1 minggu)
Tujuan: Rust core bisa dipanggil tanpa Tauri runtime.
- Buat `src/core.rs`: struct `AppCore` (Db + TelegramState + VaultState + BackupState + GoogleOAuthState)
  dengan akses global (OnceLock) — pola yang sama dengan fix `MEDIA_PLUGIN_CLASS`.
- Pindahkan logika `commands.rs` ke fungsi core murni (`core_list_timeline`, `core_scan_gallery`, …)
  yang menerima `&AppCore`; `commands.rs` jadi wrapper tipis `tauri::command` (agar versi
  desktop Tauri tetap bisa jalan selama transisi / untuk dev cepat).
- **Verifikasi:** `cargo test` tetap hijau; APK Tauri lama masih build (paralel aman).

### Fase 2 — Setup Flutter + flutter_rust_bridge (3–5 hari)
- Install Flutter SDK (Windows, mis. `D:\flutter`) + `flutter doctor` (Android toolchain sudah ada).
- Scaffold: `flutter create app_flutter --platforms=android` (nama paket `com.telegramphotos.app`
  — sama, supaya update dari APK lama tanpa uninstall).
- FRB: `flutter_rust_bridge_codegen` — definisikan `src/bridge/api.rs` (fungsi async yang
  memanggil `core_*`), generate binding Dart + C.
- Build script: cargo build `--target aarch64-linux-android` (dst) → `.so` di-copy ke
  `jniLibs` (pola yang sudah kita pakai); FRB `rust` crate menghasilkan `libapp_core.so`.
- **Verifikasi:** Hello-world Flutter app di emulator memanggil `core_count_media()` → angka nyata dari DB.

### Fase 3 — Port UI (2 minggu) — sesuai PRD Part 2
- Struktur Dart: `lib/main.dart`, `lib/screens/` (photos/search/library/settings),
  `lib/widgets/` (grid tile, status badge, progress hub, caption panel, shimmer), `lib/services/`
  (FRB wrapper + platform channels), `lib/models/`.
- Urutan port (P0): Onboarding QR login → Photos grid (GridView.builder virtualized +
  keyset paging) → badge/banner → Progress Hub → Settings → Search → Library.
- Semua string EN (i18n-ready via `flutter_localizations`/ARB).
- Gesture: pinch-zoom grid (InteractiveViewer / custom), long-press drag-select, scrubber.

### Fase 4 — Platform channels (1 minggu)
- Kotlin tetap: MediaPlugin (scan + thumbnail native), ContentObserver, WorkManager,
  notifikasi — dipanggil via **MethodChannel** dari Dart (menggantikan panggilan FRB ke
  `android_media.rs` untuk hal yang memang harus native).
- `android_media.rs` (Rust) dirapikan: tetap JNI untuk worker Rust (auto-backup),
  tapi panggilan UI ke Kotlin lewat MethodChannel langsung.
- `onTrimMemory` → MethodChannel → Dart → flush image cache.

### Fase 5 — Build, verifikasi, rilis (3–5 hari)
- Build APK release Flutter (`flutter build apk --release --split-per-abi` → arm64 saja ≈ target ≤35 MB).
- Verifikasi checklist performa PRD Part 2 §7.10 (synthetic 100k, scan, cold start, RAM, upload).
- Update `docs/BUILD.md` (proses build baru), `docs/ARCHITECTURE.md` (diagram baru).
- **Total estimasi: ~5–6 minggu** (F1–F5), P0 fitur dikerjakan paralel di F3.

## 4. Yang TIDAK berubah (penting)

- **Schema DB** tetap (migrasi v1→v2 di PRD Part 2 §6.7 tetap berlaku).
- **Enkripsi zero-knowledge** tetap di Rust (crypto.rs) — Flutter hanya lihat ciphertext.
- **MTProto/Grammers + QR login** tetap di Rust — sesi SQLite sama.
- **Scan anti-OOM** (thumbnail native Android + dimensi header-only) tetap di Kotlin/Rust.
- **Auto-backup WorkManager + notifikasi** tetap di Kotlin.
- **Fix force-close** (JNI_OnLoad + GlobalRef) tetap dipertahankan.

## 5. Risiko & mitigasi

| Risiko | Mitigasi |
|---|---|
| FRB async + Grammers (tokio) tidak cocok | FRB mendukung async Rust; pastikan runtime tokio di spawn thread sendiri (bukan main thread) |
| Rust core terkunci ke Tauri (plugin, app_data_dir) | Fase 1 memisahkan; `app_data_dir` diganti path konteks Flutter via FRB |
| Migration lama → user nunggu | Fase 1–2 paralel aman: APK Tauri lama tetap bisa dirilis selama transisi |
| FRB ukuran binary | `--split-per-abi` + strip symbols; target ≤ 35 MB (KPI) |
| JNI + MethodChannel dobel jembatan | Rule: UI→Kotlin = MethodChannel; Rust worker→Kotlin = JNI (tidak tumpang tindih) |

## 6. Keputusan yang perlu dikonfirmasi saat Fase 2

- Lokasi Flutter SDK (usulan `D:\flutter`) & versi (stable terbaru).
- Package name tetap `com.telegramphotos.app` (agar bisa update dari APK lama tanpa uninstall).
- Apakah desktop (Tauri) dipertahankan untuk dev cepat — usulan: ya, sampai Fase 3 selesai.
