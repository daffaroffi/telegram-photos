# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.1] - 2026-08-19

### Fixed
- Release APK crash: Rust `.so` not bundled in APK (missing Cargokit release build).
- Tokio reactor panic in `check_connection` on release builds.
- FRB type mismatches: `AuthCodeResult` fields `code_length` (u32) and `resend_after_seconds` (u64).
- Unused imports in onboarding, settings, and upload screens.

### Changed
- `check_connection` is now async (returns `Future<bool>`).
- `.gitignore` updated to exclude JNI libs and screenshots.

## [0.3.0] - 2026-08-19

### Added
- MTProto engine port: Telegram auth (QR code, phone OTP, 2FA) via Grammers.
- Upload pipeline: 512 KB chunked upload with FLOOD_WAIT resilience.
- Vault channel management: auto-create `TelegramPhotos_Vault` private channel.
- Backup state machine: queue → upload → done with retry and cancel.
- Onboarding screen: API credentials → QR login or phone number flow.
- FRB bridge for Telegram functions (auth, upload, vault).
- Crypto module: XChaCha20-Poly1305 encryption + Argon2id KDF.
- Upload screen: per-file progress, retry failed uploads, empty/error states.
- Settings screen: auto-backup toggle, WiFi/charging constraints, encryption setup, grid columns, logout.
- `getAppDataDir` method in Kotlin MediaPlugin.
- Vendored `core2` stub (yanked on crates.io).

### Changed
- Version bumped to `0.3.0+1002` (pre-release MTProto port).
- All documentation rewritten in English.
- Backup banner now navigates to Upload screen (replaced bottom sheet).

## [0.2.0] - 2026-08-18

### Added
- Thumbnail pipeline (G1): generate 256px JPEG thumbnails from MediaStore.
- Auto-generate thumbnails on startup when DB has items.
- `save_thumbnail_paths` bridge function (core + Rust + FRB).
- `set_thumbnail_path` method in core DB for bulk thumbnail updates.

### Fixed
- MediaPlugin thumbnail generation: parse numeric ID from stored format (`${uri}_${id}`).
- Thumbnail content URI construction for Android 29+ `loadThumbnail` API.

## [0.1.1] - 2026-08-18

### Added
- MediaStore scan via Kotlin MethodChannel.
- `importScanResults` bridge function: parse JSON → upsert media in core DB.
- Auto-scan on first launch when DB is empty.
- 29 test images detected on emulator (26 images + 3 videos).

### Fixed
- JSON contract mismatch: Kotlin sent snake_case, Rust expected camelCase.

## [0.1.0] - 2026-08-18

### Added
- Flutter app scaffold with 4-tab shell (Photos / Search / Library / Settings).
- Core Rust crate extracted from Tauri (`core/` — DB, models, media, geo).
- FRB bridge with real DB queries (no mocks).
- SQLite schema v2 migration: uploads, captions, collections tables.
- Backup banner with upload summary.
- Status badge per photo (NOT_BACKED_UP, BACKED_UP, etc.).
- Timeline grid with keyset pagination.
- Settings screen with auto-backup and encryption toggles.

## [0.0.1] - 2026-08-18

### Added
- Initial project setup.
- PRD Part 2 document.
- Telephoto reverse engineering analysis.
- Migration plan from Tauri/WebView to Flutter + Rust core.
