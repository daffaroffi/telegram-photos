# Build & Installation Guide

## Prerequisites

| Tool | Minimum Version | Notes |
|---|---|---|
| Flutter SDK | 3.22+ | `flutter --version` to check |
| Rust | stable | `rustup update stable` |
| Android SDK | API 34 | Via Android Studio |
| Android NDK | 28.x | Bundled with SDK |
| Java | 17+ | Required by Gradle |
| Node.js | 20+ | Only for old Tauri build (deprecated) |

## Development Build

```bash
# 1. Navigate to Flutter project
cd app_flutter

# 2. Install dependencies
flutter pub get

# 3. Generate FRB bindings (after any Rust API change)
flutter_rust_bridge_codegen generate

# 4. Build debug APK
flutter build apk --debug

# 5. Install on connected device/emulator
adb install build/app/outputs/flutter-apk/app-debug.apk
```

## Release Build

```bash
# Build release APK (split per ABI for smaller size)
flutter build apk --release --split-per-abi

# Output locations:
# build/app/outputs/flutter-apk/app-arm64-v8a-release.apk  (~15-25 MB)
# build/app/outputs/flutter-apk/app-armeabi-v7a-release.apk
# build/app/outputs/flutter-apk/app-x86_64-release.apk
```

## Signing

For release builds, configure signing in `android/app/build.gradle`:

```groovy
android {
    signingConfigs {
        release {
            storeFile file('path/to/keystore.jks')
            storePassword 'your-store-password'
            keyAlias 'your-key-alias'
            keyPassword 'your-key-password'
        }
    }
    buildTypes {
        release {
            signingConfig signingConfigs.release
        }
    }
}
```

Generate a keystore:

```bash
keytool -genkey -v -keystore keystore.jks -keyalg RSA -keysize 2048 -validity 10000 -alias my-key
```

## Troubleshooting

### Build fails with "clang.exe not found"

NDK path mismatch. Ensure `ANDROID_NDK_HOME` points to the correct NDK version:

```bash
ls $ANDROID_HOME/ndk/
# Use the version listed there
```

### FRB codegen produces opaque types

If types from `telegram_photos_core` appear as `RustOpaque` in Dart, add a mirror in `app_flutter/rust/src/api/mirror.rs`. See [FRB external types guide](https://cjycode.com/flutter_rust_bridge/guides/third-party/manual/external-types).

### Cargokit doesn't produce .so files

The cargokit gradle plugin needs `FLUTTER_ROOT` set. When running `flutter build`, this is automatic. When running `./gradlew` directly, ensure `local.properties` has the correct Flutter SDK path.

### core2 yanked error

Grammers depends on `core2 v0.4.0` which is yanked on crates.io. We vendor a stub in `vendor/core2/`. If the stub causes runtime issues, check `app_flutter/rust/Cargo.toml` `[patch.crates-io]` section.

### App crashes on install (versionCode downgrade)

If installing over a previous Tauri build, the versionCode must be higher. Current versionCode is `1002` (version `0.3.0+1002`).

## Architecture Notes

- **Core crate** (`core/`) compiles as `rlib` — no FFI, pure Rust.
- **FRB bridge** (`app_flutter/rust/`) compiles as `cdylib` + `staticlib` — produces `.so` for Android.
- **Cargokit** handles cross-compilation from Rust to Android ABIs (arm64, armv7, x86_64, x86).
- **Grammers** requires a tokio runtime for async MTProto operations.
