# Build and Installation Guide

## Prerequisites

| Tool | Minimum Version | Notes |
|---|---|---|
| Flutter SDK | 3.22+ | `flutter --version` to check |
| Rust | stable | `rustup update stable` |
| Android SDK | API 34 | Via Android Studio |
| Android NDK | 28.x | Bundled with SDK |
| Java | 17+ | Required by Gradle |

## Development Build

```bash
# 1. Navigate to Flutter project
cd app_flutter

# 2. Install dependencies
flutter pub get

# 3. Generate FRB bindings (after any Rust API change)
flutter_rust_bridge_codegen generate

# 4. Build Rust .so libraries
cd rust
cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 build --release
cd ..

# 5. Copy .so files to jniLibs
# (cargo-ndk outputs to target/, Gradle reads from jniLibs/)
cp -r rust/target/aarch64-linux-android/release/*.so \
      android/app/src/main/jniLibs/arm64-v8a/

# 6. Build debug APK
flutter build apk --debug

# 7. Install on connected device/emulator
adb install build/app/outputs/flutter-apk/app-debug.apk
```

## Release Build

```bash
# Build release APK (split per ABI for smaller size)
flutter build apk --release --split-per-abi

# Output locations:
# build/app/outputs/flutter-apk/app-arm64-v8a-release.apk    (~22 MB)
# build/app/outputs/flutter-apk/app-armeabi-v7a-release.apk   (~18 MB)
# build/app/outputs/flutter-apk/app-x86_64-release.apk        (~24 MB)
```

## Signing

For release builds, configure signing in `android/app/build.gradle.kts`:

```kotlin
android {
    signingConfigs {
        create("release") {
            storeFile = file("path/to/keystore.jks")
            storePassword = "your-store-password"
            keyAlias = "your-key-alias"
            keyPassword = "your-key-password"
        }
    }
    buildTypes {
        release {
            signingConfig = signingConfigs.getByName("release")
        }
    }
}
```

Generate a keystore:

```bash
keytool -genkey -v -keystore keystore.jks -keyalg RSA -keysize 2048 -validity 10000 -alias my-key
```

## Dependencies

The Android project requires these Gradle dependencies:

```kotlin
dependencies {
    // WorkManager for background backup
    implementation("androidx.work:work-runtime-ktx:2.9.0")
    implementation("androidx.core:core-ktx:1.12.0")

    // Flutter embedder (auto-added by Flutter)
    implementation("androidx.appcompat:appcompat:1.6.1")
}
```

## Troubleshooting

### Build fails with "clang.exe not found"

NDK path mismatch. Ensure `ANDROID_NDK_HOME` points to the correct NDK version:

```bash
ls $ANDROID_HOME/ndk/
# Use the version listed there
```

### FRB content hash mismatch

The Dart FRB generated code and the compiled Rust .so must have matching content hashes. If you see:

```
FRB content hash mismatch: Dart=-53940234, Rust=1614285601
```

Fix by removing stale pre-built .so files and rebuilding:

```bash
# 1. Clean stale jniLibs and build artifacts
rm -rf app_flutter/build/
rm -rf app_flutter/android/app/src/main/jniLibs/

# 2. Rebuild APK (cargokit will recompile Rust)
flutter build apk --debug

# 3. Install fresh
adb install -r build/app/outputs/flutter-apk/app-debug.apk
```

If the issue persists, also clean the Rust target:

```bash
cd app_flutter/rust
cargo clean
cd ..
flutter build apk --debug
```

### FRB codegen produces opaque types

If types from `telegram_photos_core` appear as `RustOpaque` in Dart, add a mirror in `app_flutter/rust/src/api/mirror.rs`. See [FRB external types guide](https://cjycode.com/flutter_rust_bridge/guides/third-party/manual/external-types).

### Cargokit does not produce .so files

The cargokit gradle plugin needs `FLUTTER_ROOT` set. When running `flutter build`, this is automatic. When running `./gradlew` directly, ensure `local.properties` has the correct Flutter SDK path.

### core2 yanked error

Grammers depends on `core2 v0.4.0` which is yanked on crates.io. We vendor a stub in `vendor/`. If the stub causes runtime issues, check `app_flutter/rust/Cargo.toml` `[patch.crates-io]` section.

### JNI / libdartjni.so build error

The `lucide_icons_flutter` package pulls in `jni` which needs native `.so` files. If the build fails with missing `libdartjni.so`, ensure `build.gradle.kts` has ABI filters:

```kotlin
ndk {
    abiFilters += listOf("arm64-v8a", "armeabi-v7a", "x86_64")
}
```

### App crashes on install (versionCode downgrade)

If installing over a previous build, the versionCode must be higher. Check `app_flutter/pubspec.yaml` for the current version.

### Impeller screencap shows black screen

Flutter's Impeller rendering engine is incompatible with `adb screencap` on some emulators. Workaround:

```bash
# Disable Impeller for testing
adb shell setprop debug.enable_impeller 0
# Restart the app
adb shell am force-stop com.telegramphotos.app
adb shell am start -n com.telegramphotos.app/.MainActivity
```

## Architecture Notes

- **Core crate** (`core/`) compiles as `rlib` -- no FFI, pure Rust.
- **FRB bridge** (`app_flutter/rust/`) compiles as `cdylib` + `staticlib` -- produces `.so` for Android.
- **Cargokit** handles cross-compilation from Rust to Android ABIs (arm64, armv7, x86_64, x86).
- **Grammers** requires a tokio runtime for async MTProto operations.
- **WorkManager** handles periodic background tasks with battery-aware scheduling.
