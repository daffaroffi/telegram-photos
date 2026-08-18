# Architecture

Telegram Photos uses a layered architecture: **Flutter UI** → **FRB bridge** → **Rust core** → **SQLite**. Android-native operations (MediaStore scan, thumbnails) go through **MethodChannel**.

## System Diagram

```
┌─────────────────────────────────────────────────┐
│                  Flutter UI                      │
│  PhotosScreen · SearchScreen · LibraryScreen     │
│  SettingsScreen · UploadScreen · OnboardingScreen│
└──────────────┬──────────────────────────────────┘
               │ flutter_rust_bridge (codegen)
┌──────────────▼──────────────────────────────────┐
│              FRB Bridge (Rust)                    │
│  api/db.rs · api/telegram.rs · api/mirror.rs     │
│  telegram/ (auth, upload, vault)                 │
│  backup.rs (state machine)                       │
└──────────────┬──────────────────────────────────┘
               │ Rust crate dependency
┌──────────────▼──────────────────────────────────┐
│              Core Crate (Rust)                    │
│  db.rs (SQLite CRUD, migrations)                 │
│  models.rs (MediaItem, Upload, Settings, etc.)   │
│  media.rs (EXIF, thumbnail, SHA-256)             │
│  geo.rs (offline reverse geocoding)              │
│  crypto.rs (XChaCha20-Poly1305, Argon2id)        │
└──────────────┬──────────────────────────────────┘
               │ sqlite (bundled)
┌──────────────▼──────────────────────────────────┐
│              SQLite Database                      │
│  Schema v2: media, uploads, captions,            │
│  collections, settings, vault_meta               │
└─────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────┐
│              Android Native (Kotlin)              │
│  MediaPlugin.kt — MethodChannel handler          │
│  ├── scanMediaStore() → JSON array               │
│  ├── generateThumbnails() → JSON map             │
│  └── getAppDataDir() → String                    │
└─────────────────────────────────────────────────┘
```

## Data Flow

### Photo Scan

```
User taps "Scan gallery"
  → Flutter calls MediaScan.scanGalleryJson()
  → MethodChannel → Kotlin MediaPlugin.scanMediaStore()
  → Queries MediaStore.Images + MediaStore.Video
  → Returns JSON array of media items
  → Flutter calls core.importScanResults(json)
  → Rust core upserts into SQLite
  → Flutter calls MediaScan.generateThumbnails(ids)
  → MethodChannel → Kotlin generates 256px JPEGs
  → Flutter calls core.saveThumbnailPaths(json)
  → Grid updates with real photos
```

### Telegram Login

```
OnboardingScreen → API credentials → Choose method
  → QR: authQrLogin() → tg://login?token=... → poll authQrPoll()
  → Phone: authRequestCode() → authSignIn() → (optional) authCheckPassword()
  → On success: Navigator.pop(true) → AppShell
```

### Backup

```
PhotosScreen → BackupBanner → tap → UploadScreen
  → Shows queue, progress, failures
  → Backup engine: list pending → resolve file → (optional encrypt) → upload
  → Upload: 512 KB chunks via Grammers MTProto
  → FLOOD_WAIT: auto-retry with X+2s delay
  → On success: update sync_status → BACKED_UP
```

## Key Decisions

### Why Flutter + Rust (not Tauri WebView)?

The original app used Tauri 2 with a WebView frontend. This caused:
- Poor UI performance (WebView overhead)
- Forced close on Android due to WebView lifecycle issues
- No native Android feel

Flutter provides native performance, hot reload, and a mature widget system. Rust core stays unchanged — only the UI layer was replaced.

### Why grammers (not tdlib)?

Grammers is a pure Rust Telegram client using MTProto directly. It provides:
- No C/C++ dependency (unlike TDLib)
- SQLite session storage (compatible with our core crate)
- Chunked upload with progress callbacks
- FLOOD_WAIT handling built-in

### Why vendored core2?

`core2 v0.4.0` is yanked on crates.io. Grammers-crypto depends on it via glass_pumpkin. We vendor a minimal stub that re-exports `std::error::Error` — sufficient for glass_pumpkin's usage.

### Schema Versioning

The database uses `PRAGMA user_version` for migrations:
- **v1**: Original schema (media, settings)
- **v2**: Added uploads, upload_errors, captions, caption_tags, collections, collection_items

Migrations run automatically on `Db::open()`. Old data is preserved.

## Module Responsibilities

| Module | Location | Responsibility |
|---|---|---|
| `core::db` | `core/src/db.rs` | SQLite schema, CRUD, migrations |
| `core::models` | `core/src/models.rs` | Data structures, serialization |
| `core::media` | `core/src/media.rs` | EXIF extraction, thumbnail, SHA-256 |
| `core::geo` | `core/src/geo.rs` | Offline reverse geocoding (~280 cities) |
| `core::crypto` | `core/src/crypto.rs` | Encryption/decryption, key derivation |
| `frb::api::db` | `app_flutter/rust/src/api/db.rs` | FRB bridge for core DB functions |
| `frb::api::telegram` | `app_flutter/rust/src/api/telegram.rs` | FRB bridge for MTProto auth |
| `frb::telegram` | `app_flutter/rust/src/telegram/` | MTProto client, upload, vault |
| `frb::backup` | `app_flutter/rust/src/backup.rs` | Backup state machine |
| `MediaPlugin.kt` | `android/.../MediaPlugin.kt` | Android MediaStore, thumbnails |
