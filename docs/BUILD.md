# Panduan Build & Instalasi

## Prasyarat

| Tool | Versi minimum | Catatan |
|---|---|---|
| Node.js | 20+ | `npm install` untuk frontend |
| Rust | stable (1.80+) | `rustup` + target Android |
| Java | 17+ (disarankan 21) | Untuk Gradle Android |
| Android SDK | platform 36 | `ANDROID_HOME` harus disetel |
| Android NDK | 28.x | Digunakan toolchain clang untuk `cargo` |

Target Rust Android yang dibutuhkan:

```bash
rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
```

---

## 1. Instalasi dependensi

```bash
cd app
npm install
```

## 2. Build APK Android

```bash
export ANDROID_HOME="$HOME/Android/Sdk"        # Windows: "$LOCALAPPDATA/Android/Sdk"
export ANDROID_SDK_ROOT="$ANDROID_HOME"

# Release, ABI arm64 saja (paling umum untuk perangkat modern)
npx tauri android build --target aarch64

# Semua ABI (universal) — lebih besar tapi jalan di semua perangkat
npx tauri android build
```

Hasil:

```
app/src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release-unsigned.apk
app/src-tauri/gen/android/app/build/outputs/bundle/universalRelease/app-universal-release.aab
```

> **Frontend assets**: `app/dist` (hasil `vite build`, dijalankan otomatis oleh
> `beforeBuildCommand`) didaftarkan sebagai asset source dir di
> `gen/android/app/build.gradle.kts` — ikut ter-bundle ke APK otomatis.
> `gen/android/app/src/main/jniLibs/` (hasil build `.so`) di-ignore git dan
> dihasilkan kembali saat build.

> **Workaround — gradle gagal spawn `npm` di Windows** (error
> `A problem occurred starting process 'command 'npm.bat''`): build `.so`
> manual lalu assemble dengan skip task rust:
> ```bash
> cd app/src-tauri
> # set NDK toolchain di PATH + CC_aarch64_linux_android / AR_aarch64_linux_android
> cargo build --release --target aarch64-linux-android
> mkdir -p gen/android/app/src/main/jniLibs/arm64-v8a
> cp target/aarch64-linux-android/release/libtelegram_photos_lib.so \
>    gen/android/app/src/main/jniLibs/arm64-v8a/
> cd gen/android
> ./gradlew :app:assembleUniversalRelease \
>   -x rustBuildArm64Release -x rustBuildArmRelease \
>   -x rustBuildX86Release -x rustBuildX86_64Release -x rustBuildUniversalRelease
> ```

### Signing (wajib agar bisa diinstal)

Tauri tidak menandatangani APK release secara otomatis. Buat keystore sekali:

```bash
keytool -genkeypair -v \
  -keystore ~/telegramphotos.keystore \
  -alias telegramphotos \
  -keyalg RSA -keysize 2048 -validity 10000 \
  -storepass GANTI_DENGAN_PASSWORD_ANDAMU -keypass GANTI_DENGAN_PASSWORD_ANDAMU \
  -dname "CN=TelegramPhotos, O=TelegramPhotos, C=ID"
```

Lalu sign APK unsigned:

```bash
BT="$ANDROID_HOME/build-tools/37.0.0"
"$BT/apksigner" sign \
  --ks ~/telegramphotos.keystore \
  --ks-key-alias telegramphotos \
  --ks-pass pass:GANTI_DENGAN_PASSWORD_ANDAMU --key-pass pass:GANTI_DENGAN_PASSWORD_ANDAMU \
  --out TelegramPhotos-release.apk \
  app/src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release-unsigned.apk

"$BT/apksigner" verify TelegramPhotos-release.apk
```

> Simpan keystore & password di tempat aman (jangan commit password ke repo
> publik). Keystore yang sama harus dipakai untuk semua update agar aplikasi
> bisa di-update di perangkat.

### APK debug (tanpa signing manual)

```bash
npx tauri android build --debug --target aarch64
# hasil: .../apk/universal/debug/app-universal-debug.apk (tertanda debug)
```

## 3. Menjalankan di perangkat

```bash
# Dengan perangkat terhubung via USB (USB debugging aktif)
npx tauri android dev

# Atau instal APK hasil build
adb install -r TelegramPhotos-release.apk
```

Izin yang diminta saat pertama dijalankan:

- **Foto & video** (`READ_MEDIA_IMAGES` / `READ_MEDIA_VIDEO`) — untuk memindai galeri.
- **Notifikasi** (`POST_NOTIFICATIONS`) — untuk progres backup background.

## 4. Setup pertama di aplikasi

1. **Kredensial API Telegram** — buat di <https://my.telegram.org/apps> (gratis),
   tempel API ID + API Hash.
2. **Login** — pilih nomor telepon (OTP) atau QR code. Jika akun memakai 2FA,
   masukkan cloud password.
3. Vault channel privat `TelegramPhotos_Vault` dibuat otomatis.
4. Buka tab **Galeri** → **Pindai galeri** untuk memuat media perangkat.
5. Buka tab **Backup** → **Mulai backup sekarang**.

## 5. Build desktop (Windows/macOS/Linux)

```bash
cd app
npm run tauri dev      # mode pengembangan dengan hot reload
npm run tauri build    # bundle desktop
```

> Fitur MediaStore & WorkManager khusus Android; di desktop gunakan
> **📁 Tambah folder** di tab Galeri untuk memilih direktori foto.

## 6. Perintah penting lainnya

| Perintah | Fungsi |
|---|---|
| `npx tauri android init` | (Re)generate proyek Android di `gen/android` |
| `cargo test` (di `app/src-tauri`) | Menjalankan unit test backend (crypto, geo, media) |
| `npx tsc --noEmit` (di `app`) | Typecheck frontend |
| `cargo check --target aarch64-linux-android` | Validasi compile backend untuk Android |
