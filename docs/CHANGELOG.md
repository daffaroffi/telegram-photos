# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.7.0] - 2026-08-19

### Added
- Lucide icons replacing all Material Icons across every screen.
- Progressive disclosure on onboarding: credentials step, then method selection.
- Touch targets minimum 48dp on all interactive elements (Fitts's Law).
- Haptic feedback on photo select/deselect (selectionClick).
- Bottom sheet for upload action replacing fullscreen dialog.
- Section headers with Lucide icons in Settings (Backup, Encryption, Display, Performance, Account).
- Semantic labels for screen reader accessibility on key widgets.
- Better empty states with Lucide icon, title, description, and CTA button on all screens.
- Error states with retry button on upload and photos screens.
- Type hierarchy consistency: headline/title/body/caption across all screens.
- Color semantic fix: colorScheme.* used everywhere instead of hardcoded colors.

### Changed
- Removed FAB from Photos tab; scan action moved to AppBar and empty state.
- StatusBadge uses semantic Material 3 colors (colorScheme.error, colorScheme.outline).
- BackupBanner uses colorScheme.primaryContainer.
- Settings uses _copyWith helper for cleaner state management.

### Fixed
- FRB content hash mismatch: fresh .so copied from target/ to jniLibs/.
- Impeller screencap incompatibility documented (emulator limitation, not app bug).

## [0.6.0] - 2026-08-19

### Added
- Encrypt-before-upload: files automatically encrypted with XChaCha20-Poly1305 before vault upload when encryption is enabled.
- Auto-backup toggle wired to WorkManager: enable/disable periodic background backup from Settings.
- Multi-select photo picker: long-press to enter selection mode, select all, bulk upload.
- Performance benchmark: measures DB query time, timeline load time, RSS memory usage.
- Loading skeleton grid during first load.
- Error retry button on upload failure.
- Upload success indicator (cloud_done icon) in preview dialog.
- Select all button in selection mode toolbar.

### Improved
- Empty state on Photos screen: larger icon, descriptive text, CTA button.
- Empty state on Search screen: clearer messaging.
- Upload preview dialog: retry button, success state, better error display.

## [0.5.0] - 2026-08-19

### Added
- XChaCha20-Poly1305 encryption for vault files (client-side, zero-knowledge).
- Argon2id key derivation from user passphrase.
- FRB bridge for crypto: vaultSetup, vaultUnlock, vaultLock, deriveKey, encryptFile, decryptFile.
- Encryption setup dialog in Settings: passphrase input with show/hide toggle, confirm, strength indicator.

### Fixed
- AUTH_RESTART: robust retry loop (up to 3 attempts, exponential backoff 500ms/1000ms).
- Extracted reset_client_state helper for clean session cleanup on auth errors.
- Remaining Indonesian error messages translated to English.

## [0.4.1] - 2026-08-19

### Added
- BackupWorker: WorkManager-based periodic background backup (every 15 min).
- Notification channel "Photo Backup" with progress/completion/failure notifications.
- startBackup MethodChannel: queue items for background upload.
- cancelBackup MethodChannel: cancel all pending backup work.
- BackupService Dart bridge for triggering background backup.

## [0.4.0] - 2026-08-19

### Added
- Single photo upload: tap photo, upload to vault, encrypt, upload to Telegram vault channel.
- readFileToTemp Kotlin bridge: reads files from content URI via ContentResolver.
- uploadPhoto FRB bridge: creates vault channel if needed, uploads file via 512KB chunks.
- setMediaStatus FRB bridge: update sync status after upload.
- Media ID format fix: convert media_123 to media/123 for valid content URI.

### Verified
- Upload flow E2E: content URI, temp file, upload.saveFilePart, messages.sendMedia, updates confirmation.
- Vault channel auto-created (TelegramPhotos_Vault).

## [0.3.3] - 2026-08-19

### Verified
- E2E test complete: Login (API + Phone OTP), Photo grid, 4 tabs all functional.
- Photos tab: grid with thumbnails, Refresh, Scan gallery FAB, photo detail view.
- Search tab: filter chips (All/Videos/Screenshots/Last 30 days), search bar.
- Library tab: Collections (0), Favorites/Memories/Trash/Device folders sections.
- Settings tab: Auto backup, WiFi only, While charging, Original quality toggles.
- Encryption: Set up encryption button, Client-side encryption toggle.
- Vault: TelegramPhotos_Vault channel shown (0 files, 0 KB).
- Display: Grid columns setting.
- Final APK sizes: armeabi-v7a 17.6MB, arm64-v8a 21.3MB, x86_64 22.9MB.

## [0.3.2] - 2026-08-19

### Fixed
- AUTH_RESTART error: delete stale session file and re-initialize client on retry.
- DroppableDisposedException: all Telegram functions now take &TelegramHandle (reference).
- Tokio reactor panic: check_connection is now async.
- Onboarding Continue button: onChanged callback triggers rebuild.
- Onboarding flow: onAuthenticated callback switches from onboarding to main app.
- Session restore on cold start: save .telegram_creds file + read real appDataDir.
- All error messages translated from Indonesian to English.
- PHONE_MIGRATE error handling added.

### Verified
- Full E2E test on emulator: API credentials, Phone OTP, Login, Photo grid.
- 4-tab shell (Photos/Search/Library/Settings) renders correctly.
- Photo grid displays 12+ items with "On this device" badge.

## [0.3.1] - 2026-08-19

### Fixed
- Release APK crash: Rust .so not bundled in APK (missing Cargokit release build).
- Tokio reactor panic in check_connection on release builds.
- FRB type mismatches: AuthCodeResult fields code_length (u32) and resend_after_seconds (u64).
- Unused imports in onboarding, settings, and upload screens.

### Changed
- check_connection is now async (returns Future<bool>).
- .gitignore updated to exclude JNI libs and screenshots.

## [0.3.0] - 2026-08-19

### Added
- MTProto engine port: Telegram auth (QR code, phone OTP, 2FA) via Grammers.
- Upload pipeline: 512 KB chunked upload with FLOOD_WAIT resilience.
- Vault channel management: auto-create TelegramPhotos_Vault private channel.
- Backup state machine: queue, upload, done with retry and cancel.
- Onboarding screen: API credentials, QR login or phone number flow.
- FRB bridge for Telegram functions (auth, upload, vault).
- Crypto module: XChaCha20-Poly1305 encryption + Argon2id KDF.
- Upload screen: per-file progress, retry failed uploads, empty/error states.
- Settings screen: auto-backup toggle, WiFi/charging constraints, encryption setup, grid columns, logout.
- getAppDataDir method in Kotlin MediaPlugin.
- Vendored core2 stub (yanked on crates.io).

### Changed
- Version bumped to 0.3.0+1002 (pre-release MTProto port).
- All documentation rewritten in English.
- Backup banner now navigates to Upload screen (replaced bottom sheet).

## [0.2.0] - 2026-08-18

### Added
- Thumbnail pipeline: generate 256px JPEG thumbnails from MediaStore.
- Auto-generate thumbnails on startup when DB has items.
- save_thumbnail_paths bridge function (core + Rust + FRB).
- set_thumbnail_path method in core DB for bulk thumbnail updates.

### Fixed
- MediaPlugin thumbnail generation: parse numeric ID from stored format.
- Thumbnail content URI construction for Android 29+ loadThumbnail API.

## [0.1.1] - 2026-08-18

### Added
- MediaStore scan via Kotlin MethodChannel.
- importScanResults bridge function: parse JSON, upsert media in core DB.
- Auto-scan on first launch when DB is empty.
- 29 test images detected on emulator (26 images + 3 videos).

### Fixed
- JSON contract mismatch: Kotlin sent snake_case, Rust expected camelCase.

## [0.1.0] - 2026-08-18

### Added
- Flutter app scaffold with 4-tab shell (Photos / Search / Library / Settings).
- Core Rust crate extracted from Tauri (core/ with DB, models, media, geo).
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
