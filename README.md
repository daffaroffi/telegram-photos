# Telegram Photos

Back up your photo gallery to a private Telegram channel with zero-knowledge encryption.

Built with **Flutter UI** + **Rust core** via [flutter_rust_bridge](https://fzyzcjy.github.io/flutter_rust_bridge/). Telegram MTProto integration uses [Grammers](https://github.com/Lonami/grammers) -- a pure Rust Telegram client library.

| | |
|---|---|
| **Platform** | Android (primary), desktop (planned) |
| **UI** | Flutter 3.x + Material 3 + Lucide Icons |
| **Core** | Rust -- Grammers MTProto, SQLite, XChaCha20-Poly1305 |
| **Bridge** | flutter_rust_bridge 2.12 (codegen Rust <-> Dart) |
| **Android Native** | Kotlin -- MediaStore scan, thumbnail generation |
| **Background** | WorkManager periodic backup with notifications |

## Features

### Login and Session

- Phone number login with OTP and 2FA password support.
- QR code login (scan with Telegram on another device).
- Session persistence -- no re-login after app restart.
- AUTH_RESTART recovery with exponential backoff (3 attempts).
- PHONE_MIGRATE handling for DC migration.

### Photo Gallery

- Scan MediaStore.Images + MediaStore.Video via Kotlin MethodChannel.
- Photo grid with timeline layout, thumbnails, and status badges.
- Multi-select: long-press to enter selection mode, select all, bulk upload.
- Photo detail view with file info, EXIF data, and upload action.
- Loading skeleton grid during first load.
- Empty state with icon, description, and CTA button.

### Backup to Telegram

- Auto-creates private channel TelegramPhotos_Vault.
- Chunked upload (512 KB) with real-time progress.
- FLOOD_WAIT handling with auto-retry and exponential delay.
- Upload screen with per-file progress, retry, and summary.
- Encrypt-before-upload: XChaCha20-Poly1305 encryption when enabled.

### Background Backup

- WorkManager periodic backup (every 15 minutes).
- Notification channel "Photo Backup" with progress/completion/failure.
- WiFi-only and charging-only constraints.

### Zero-Knowledge Encryption

- XChaCha20-Poly1305 streaming encryption (4 KB chunks).
- Key derived from user passphrase via Argon2id (64 MiB memory cost).
- Passphrase never stored -- only salt + KDF parameters in local DB.
- Vault lock/unlock with passphrase protection.

### Search and Library

- Instant search across filename, caption, and hashtags.
- Filter chips: All, Videos, Screenshots, Last 30 days.
- Collections with CRUD operations.
- Favorites, Memories, Trash (planned).

### Settings

- Auto-backup toggle wired to WorkManager.
- WiFi-only and while-charging constraints.
- Client-side encryption setup with passphrase.
- Grid column count (3/4/5/6).
- Performance benchmark: DB query time, timeline load, RSS memory.

## Project Structure

```
.
├── core/                          # Rust core crate (pure business logic)
│   ├── src/
│   │   ├── db.rs                  # SQLite schema, CRUD, migrations
│   │   ├── models.rs              # Data models (MediaItem, Upload, etc.)
│   │   ├── media.rs               # EXIF, thumbnail, SHA-256 hashing
│   │   ├── geo.rs                 # Offline reverse geocoding
│   │   └── crypto.rs              # XChaCha20-Poly1305 + Argon2id
│   └── Cargo.toml
├── app_flutter/                   # Flutter app + FRB bridge
│   ├── lib/
│   │   ├── main.dart              # Entry point, auth check, onboarding
│   │   └── src/
│   │       ├── screens/           # Photos, Search, Library, Settings, Upload, Onboarding
│   │       ├── widgets/           # StatusBadge, BackupBanner
│   │       └── platform/          # MediaScan, BackupService MethodChannels
│   ├── rust/                      # FRB bridge crate
│   │   ├── src/
│   │   │   ├── api/               # FRB-exposed functions (db, telegram, crypto, mirror)
│   │   │   ├── telegram/          # MTProto auth, upload, vault
│   │   │   └── backup.rs          # Backup state machine
│   │   └── Cargo.toml
│   └── android/                   # Android project (Kotlin)
│       └── .../
│           ├── MediaPlugin.kt     # MediaStore scan, thumbnails
│           ├── MainActivity.kt    # FlutterActivity + MethodChannels
│           └── BackupWorker.kt    # WorkManager periodic backup
├── vendor/                         # Vendored dependencies
└── docs/                          # Public documentation
    ├── ARCHITECTURE.md            # System architecture and data flow
    ├── BUILD.md                   # Build and installation guide
    └── CHANGELOG.md               # Version history
```

## Quick Start

**Prerequisites:** Flutter SDK, Rust (stable), Android SDK + NDK, Java 17+.

```bash
# 1. Install Flutter dependencies
cd app_flutter
flutter pub get

# 2. Generate FRB bindings
flutter_rust_bridge_codegen generate

# 3. Build debug APK
flutter build apk --debug

# 4. Install on emulator/device
adb install build/app/outputs/flutter-apk/app-debug.apk
```

See **[docs/BUILD.md](docs/BUILD.md)** for detailed instructions including release builds and signing.

## Tech Stack

| Layer | Choice | Why |
|---|---|---|
| UI | Flutter 3.x + Material 3 + Lucide Icons | Native performance, modern icon set |
| Core | Rust (grammers, sqlite, chacha20poly1305) | Safety, performance, MTProto compatibility |
| Bridge | flutter_rust_bridge 2.12 | Type-safe Rust <-> Dart codegen |
| Android | Kotlin MethodChannel | MediaStore access, thumbnail generation |
| Database | SQLite (bundled) | Compatible with grammers-session, WAL mode |
| Encryption | XChaCha20-Poly1305 + Argon2id | Streaming encryption, memory-hard KDF |
| Background | WorkManager | Reliable periodic backup on Android |

## Documentation

- **[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)** -- System architecture and data flow
- **[docs/BUILD.md](docs/BUILD.md)** -- Build and installation guide
- **[docs/CHANGELOG.md](docs/CHANGELOG.md)** -- Version history

## Versioning

This project uses [Semantic Versioning](https://semver.org/): `MAJOR.MINOR.PATCH+buildNumber`.

Current version: **0.7.1+5100**

## License

[MIT](LICENSE)
