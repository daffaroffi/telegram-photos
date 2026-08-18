# Telegram Photos

Back up your photo gallery to a private Telegram channel with zero-knowledge encryption.

Built with **Flutter UI** + **Rust core** via [flutter_rust_bridge](https://fzyzcjy.github.io/flutter_rust_bridge/). Telegram MTProto integration uses [Grammers](https://github.com/Lonami/grammers) — a pure Rust Telegram client library.

| | |
|---|---|
| **Platform** | Android (Flutter), desktop (Windows/macOS/Linux) |
| **UI** | Flutter 3.x + Material 3 |
| **Core** | Rust — Grammers MTProto, SQLite, XChaCha20-Poly1305 encryption |
| **Bridge** | flutter_rust_bridge 2.12 (codegen Rust ↔ Dart) |
| **Android Native** | Kotlin — MediaStore scan, thumbnail generation, MethodChannel |

## Features

### Backup to Telegram
- Login via **QR code** or **phone number** (OTP + 2FA).
- Auto-creates a private channel `TelegramPhotos_Vault`.
- Chunked upload (512 KB) with real-time progress and FLOOD_WAIT handling.
- Backup state machine: `NOT_BACKED_UP → QUEUED → UPLOADING → BACKED_UP`.

### Local Gallery (Android MediaStore)
- Scan `MediaStore.Images` + `MediaStore.Video` via Kotlin MethodChannel.
- EXIF extraction: date, GPS, camera model, ISO, aperture.
- SHA-256 hashing per file for deduplication.
- Thumbnail generation (256 px JPEG) — no full decode (anti-OOM).

### Zero-Knowledge Encryption
- XChaCha20-Poly1305 streaming encryption (4 KB chunks).
- Key derived from user passphrase via Argon2id (64 MiB memory cost).
- Passphrase never stored — only salt + KDF parameters in local DB.

### Backup Progress
- Per-file progress tracking with retry and cancel.
- Failed upload queue with error details.
- WiFi-only and charging-only constraints.

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
│   │       └── platform/          # MediaScan MethodChannel
│   ├── rust/                      # FRB bridge crate
│   │   ├── src/
│   │   │   ├── api/               # FRB-exposed functions (db, telegram, mirror)
│   │   │   ├── telegram/          # MTProto auth, upload, vault
│   │   │   └── backup.rs          # Backup state machine
│   │   └── Cargo.toml
│   └── android/                   # Android project (Kotlin)
│       └── .../MediaPlugin.kt     # MediaStore scan, thumbnails, getAppDataDir
├── vendor/core2/                  # Vendored core2 stub (yanked on crates.io)
└── docs/                          # Documentation
    ├── ARCHITECTURE.md
    ├── BUILD.md
    ├── CHANGELOG.md
    └── PRD_PART2.md
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

See **[docs/BUILD.md](docs/BUILD.md)** for detailed instructions including release builds, signing, and troubleshooting.

## Tech Stack

| Layer | Choice | Why |
|---|---|---|
| UI | Flutter 3.x + Material 3 | Cross-platform, hot reload, native performance |
| Core | Rust (grammers, sqlite, chacha20poly1305) | Safety, performance, MTProto compatibility |
| Bridge | flutter_rust_bridge 2.12 | Type-safe Rust ↔ Dart codegen |
| Android | Kotlin MethodChannel | MediaStore access, thumbnail generation |
| Database | SQLite (bundled via rusqlite) | Compatible with grammers-session, WAL mode |
| Encryption | XChaCha20-Poly1305 + Argon2id | Streaming encryption, memory-hard KDF |

## Documentation

- **[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)** — System architecture and data flow
- **[docs/BUILD.md](docs/BUILD.md)** — Build and installation guide
- **[docs/CHANGELOG.md](docs/CHANGELOG.md)** — Version history
- **[docs/PRD_PART2.md](docs/PRD_PART2.md)** — Product requirements (UX, features)
- **[docs/TELEPHOTO_RE.md](docs/TELEPHOTO_RE.md)** — Reverse engineering of competitor app

## Versioning

This project uses [Semantic Versioning](https://semver.org/): `MAJOR.MINOR.PATCH+buildNumber`.

Current version: **0.3.0+1002** (pre-release, MTProto port in progress).

## License

[MIT](LICENSE)
