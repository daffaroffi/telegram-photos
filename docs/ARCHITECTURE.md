# Architecture

Telegram Photos uses a layered architecture: **Flutter UI** -> **FRB bridge** -> **Rust core** -> **SQLite**. Android-native operations (MediaStore scan, thumbnails) go through **MethodChannel**. Background tasks use **WorkManager**.

## System Diagram

```
+---------------------------------------------------+
|                  Flutter UI                        |
|  PhotosScreen  SearchScreen  LibraryScreen         |
|  SettingsScreen  UploadScreen  OnboardingScreen    |
|  Widgets: StatusBadge, BackupBanner                |
+--------------------+------------------------------+
                     | flutter_rust_bridge (codegen)
+--------------------v------------------------------+
|              FRB Bridge (Rust)                     |
|  api/db.rs  api/telegram.rs  api/crypto.rs         |
|  api/mirror.rs                                    |
|  telegram/ (auth, upload, vault)                  |
|  backup.rs (state machine)                        |
+--------------------+------------------------------+
                     | Rust crate dependency
+--------------------v------------------------------+
|              Core Crate (Rust)                     |
|  db.rs (SQLite CRUD, migrations)                  |
|  models.rs (MediaItem, Upload, Settings, etc.)    |
|  media.rs (EXIF, thumbnail, SHA-256)              |
|  geo.rs (offline reverse geocoding)               |
|  crypto.rs (XChaCha20-Poly1305, Argon2id)         |
+--------------------+------------------------------+
                     | sqlite (bundled)
+--------------------v------------------------------+
|              SQLite Database                       |
|  Schema v2: media, uploads, captions,             |
|  collections, settings, vault_meta                |
+---------------------------------------------------+

+---------------------------------------------------+
|              Android Native (Kotlin)               |
|  MediaPlugin.kt -- MethodChannel handler           |
|  +-- scanMediaStore() -> JSON array                |
|  +-- generateThumbnails() -> JSON map              |
|  +-- readFileToTemp() -> temp file path            |
|  +-- getAppDataDir() -> String                     |
|                                                    |
|  BackupWorker.kt -- WorkManager                    |
|  +-- Periodic backup (every 15 min)                |
|  +-- Notification channel "Photo Backup"           |
|  +-- Progress/completion/failure notifications     |
+---------------------------------------------------+
```

## Data Flow

### Photo Scan

```
User taps "Scan gallery"
  -> Flutter calls MediaScan.scanGalleryJson()
  -> MethodChannel -> Kotlin MediaPlugin.scanMediaStore()
  -> Queries MediaStore.Images + MediaStore.Video
  -> Returns JSON array of media items
  -> Flutter calls core.importScanResults(json)
  -> Rust core upserts into SQLite
  -> Flutter calls MediaScan.generateThumbnails(ids)
  -> MethodChannel -> Kotlin generates 256px JPEGs
  -> Flutter calls core.saveThumbnailPaths(json)
  -> Grid updates with real photos
```

### Telegram Login

```
OnboardingScreen (Step 1: Credentials)
  -> User enters API ID + API Hash
  -> Tap Continue -> Step 2: Method Selection
  -> Phone: authRequestCode() -> OTP input -> authSignIn()
  -> QR: authQrLogin() -> tg://login?token=... -> poll authQrPoll()
  -> (Optional) 2FA: authCheckPassword()
  -> On success: session saved to .telegram_creds -> AppShell
```

### Upload to Vault

```
PhotosScreen -> tap photo -> BottomSheet
  -> "Upload to vault" button
  -> Check encryption: if enabled + unlocked, encrypt file first
  -> core.readFileToTemp(contentUri) -> temp file
  -> tg.uploadPhoto(filePath, fileName, mimeType)
  -> 512 KB chunked upload via Grammers MTProto
  -> FLOOD_WAIT: auto-retry with X+2s delay
  -> On success: core.setMediaStatus(id, BACKED_UP)
  -> UI shows success indicator
```

### Background Backup

```
WorkManager triggers BackupWorker (every 15 min)
  -> Check constraints (WiFi, charging)
  -> Poll pending items from SQLite
  -> For each item:
     -> (Optional) encrypt file
     -> Upload via Grammers MTProto
     -> Update sync status
  -> Show progress notification
  -> Show completion/failure notification
```

### Multi-Select Bulk Upload

```
PhotosScreen -> long-press photo -> selection mode
  -> Toolbar: "X selected", Select All, Upload button
  -> Tap Upload -> Bottom sheet confirmation
  -> Process each selected item sequentially
  -> Show progress (X of Y)
  -> Update status badges on completion
```

## Key Decisions

### Why Flutter + Rust (not Tauri WebView)?

The original app used Tauri 2 with a WebView frontend. This caused:
- Poor UI performance (WebView overhead)
- Forced close on Android due to WebView lifecycle issues
- No native Android feel

Flutter provides native performance, hot reload, and a mature widget system. Rust core stays unchanged -- only the UI layer was replaced.

### Why grammers (not tdlib)?

Grammers is a pure Rust Telegram client using MTProto directly. It provides:
- No C/C++ dependency (unlike TDLib)
- SQLite session storage (compatible with our core crate)
- Chunked upload with progress callbacks
- FLOOD_WAIT handling built-in

### Why vendored core2?

`core2 v0.4.0` is yanked on crates.io. Grammers-crypto depends on it via glass_pumpkin. We vendor a minimal stub that re-exports `std::error::Error` -- sufficient for glass_pumpkin's usage.

### Why XChaCha20-Poly1305 + Argon2id?

- XChaCha20-Poly1305: streaming encryption with 4 KB chunks, 192-bit nonce for uniqueness.
- Argon2id: memory-hard KDF (64 MiB) resistant to GPU/ASIC attacks.
- Zero-knowledge: server (Telegram) never sees plaintext files.

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
| `frb::api::telegram` | `app_flutter/rust/src/api/telegram.rs` | FRB bridge for MTProto auth and upload |
| `frb::api::crypto` | `app_flutter/rust/src/api/crypto.rs` | FRB bridge for encryption functions |
| `frb::telegram` | `app_flutter/rust/src/telegram/` | MTProto client, upload, vault |
| `frb::backup` | `app_flutter/rust/src/backup.rs` | Backup state machine |
| `MediaPlugin.kt` | `android/.../MediaPlugin.kt` | Android MediaStore, thumbnails, file read |
| `BackupWorker.kt` | `android/.../BackupWorker.kt` | WorkManager periodic backup |
| `MainActivity.kt` | `android/.../MainActivity.kt` | FlutterActivity + MethodChannel registration |

## UI Design Principles

The UI follows principles from the design-intelligence skill:

- **Anti-Slop**: No gratuitous FABs, gradients, or shadows. Every element has a purpose.
- **Hick's Law**: Progressive disclosure on onboarding (credentials, then method).
- **Fitts's Law**: Touch targets minimum 48dp on all interactive elements.
- **Jakob's Law**: Lucide icons for universal familiarity.
- **Von Restorff**: Primary CTA visible on empty states.
- **Peak-End Rule**: Error states with retry buttons, success indicators.
- **Thumb-Friendly**: Bottom sheets for actions, buttons in lower screen half.
