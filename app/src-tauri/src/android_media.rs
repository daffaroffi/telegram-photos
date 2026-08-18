//! Bridge to native Android capabilities (PRD sections 4.3, 4.4).
//!
//! On Android these functions call into the Kotlin plugin (`MediaPlugin`) via
//! JNI to read the real MediaStore, register a ContentObserver for new media,
//! and check the real Wi-Fi / charging state for backup constraints. On
//! desktop they fall back to directory scanning and "always satisfied"
//! constraints.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeMediaEntry {
    pub id: String,
    pub uri: String,
    pub path: Option<String>,
    pub file_name: String,
    pub mime_type: String,
    pub media_type: String,
    pub size_bytes: i64,
    pub date_taken: i64,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub duration_ms: Option<i64>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub device_folder: String,
    pub is_favorite: bool,
}

// ─────────────────────────────────────────────────────────────────────────────
// Constraints (PRD 4.4: Wi-Fi only / charging only)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(not(target_os = "android"))]
pub fn constraints_ok(_wifi_only: bool, _charging_only: bool) -> bool {
    // Desktop: no OS constraint enforcement.
    true
}

#[cfg(target_os = "android")]
pub fn constraints_ok(wifi_only: bool, charging_only: bool) -> bool {
    // Ask the Kotlin plugin for real network + charging state. On failure we
    // fail-open so backups are never silently blocked forever.
    with_env(|env| {
        let class = env
            .find_class("com/telegramphotos/app/MediaPlugin")
            .map_err(|e| e.to_string())?;
        let result = env
            .call_static_method(
                class,
                "checkConstraints",
                "(ZZ)Z",
                &[
                    jni::objects::JValue::Bool(wifi_only as u8),
                    jni::objects::JValue::Bool(charging_only as u8),
                ],
            )
            .map_err(|e| e.to_string())?;
        result.z().map_err(|e| e.to_string())
    })
    .unwrap_or(true)
}

// ─────────────────────────────────────────────────────────────────────────────
// MediaStore scan (PRD 4.3)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(not(target_os = "android"))]
pub fn scan_gallery(_folder: Option<&str>) -> Result<Vec<NativeMediaEntry>, String> {
    // Desktop: scanning is handled by the frontend file picker; nothing to
    // enumerate here.
    Ok(Vec::new())
}

#[cfg(target_os = "android")]
pub fn scan_gallery(folder: Option<&str>) -> Result<Vec<NativeMediaEntry>, String> {
    with_env(|env| {
        let class = env
            .find_class("com/telegramphotos/app/MediaPlugin")
            .map_err(|e| e.to_string())?;
        let arg = env
            .new_string(folder.unwrap_or(""))
            .map_err(|e| e.to_string())?;
        let result = env
            .call_static_method(
                class,
                "scanMediaStore",
                "(Ljava/lang/String;)Ljava/lang/String;",
                &[jni::objects::JValue::Object(&arg)],
            )
            .map_err(|e| e.to_string())?;
        let obj = result.l().map_err(|e| e.to_string())?;
        let jstr = jni::objects::JString::from(obj);
        let s = env.get_string(&jstr).map_err(|e| e.to_string())?;
        let json: String = s.into();
        serde_json::from_str(&json).map_err(|e| format!("Scan galeri gagal: {e}"))
    })
}

/// Copies a MediaStore `content://` URI into app-private storage so Rust can
/// read it (PRD 4.3). Returns the absolute path, or an error on failure.
#[cfg(target_os = "android")]
pub fn materialize_media(uri: &str, dest_dir: &str, file_name: &str) -> Result<String, String> {
    with_env(|env| {
        let class = env
            .find_class("com/telegramphotos/app/MediaPlugin")
            .map_err(|e| e.to_string())?;
        let uri_s = env.new_string(uri).map_err(|e| e.to_string())?;
        let dir_s = env.new_string(dest_dir).map_err(|e| e.to_string())?;
        let name_s = env.new_string(file_name).map_err(|e| e.to_string())?;
        let result = env
            .call_static_method(
                class,
                "materializeMedia",
                "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;",
                &[
                    jni::objects::JValue::Object(&uri_s),
                    jni::objects::JValue::Object(&dir_s),
                    jni::objects::JValue::Object(&name_s),
                ],
            )
            .map_err(|e| e.to_string())?;
        let obj = result.l().map_err(|e| e.to_string())?;
        let jstr = jni::objects::JString::from(obj);
        let s = env.get_string(&jstr).map_err(|e| e.to_string())?;
        let path: String = s.into();
        if path.is_empty() {
            Err("Gagal menyalin media dari MediaStore.".into())
        } else {
            Ok(path)
        }
    })
}

#[cfg(not(target_os = "android"))]
pub fn materialize_media(_uri: &str, _dest_dir: &str, _file_name: &str) -> Result<String, String> {
    Err("MediaStore tidak tersedia di desktop.".into())
}

/// Registers a ContentObserver so the Rust side can be notified of new media
/// (PRD 4.3: real-time gallery sync).
#[cfg(target_os = "android")]
pub fn register_content_observer() -> Result<(), String> {
    with_env(|env| {
        let class = env
            .find_class("com/telegramphotos/app/MediaPlugin")
            .map_err(|e| e.to_string())?;
        let arg = env.new_string("").map_err(|e| e.to_string())?;
        env.call_static_method(
            class,
            "registerContentObserver",
            "(Ljava/lang/String;)Ljava/lang/String;",
            &[jni::objects::JValue::Object(&arg)],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}

#[cfg(not(target_os = "android"))]
pub fn register_content_observer() -> Result<(), String> {
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// JNI helpers (Android only)
// ─────────────────────────────────────────────────────────────────────────────

/// Attaches to the JVM and runs `f` with a usable `JNIEnv`.
#[cfg(target_os = "android")]
fn with_env<T>(
    f: impl FnOnce(&mut jni::JNIEnv) -> Result<T, String>,
) -> Result<T, String> {
    let ctx = ndk_context::android_context();
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm() as *mut jni::sys::JavaVM) }
        .map_err(|e| format!("JVM gagal: {e}"))?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|e| format!("JNI attach gagal: {e}"))?;
    f(&mut env)
}
