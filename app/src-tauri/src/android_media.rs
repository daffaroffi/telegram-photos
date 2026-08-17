//! Bridge to native Android capabilities (PRD sections 4.3, 4.4).
//!
//! On Android these functions call into the Kotlin plugin (`MediaPlugin`) via
//! JNI to read the real MediaStore with a ContentObserver, and to check the
//! real Wi-Fi / charging state for backup constraints. On desktop they fall
//! back to directory scanning and "always satisfied" constraints.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
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
    match jni_call_boolean("checkConstraints", &[wifi_only, charging_only]) {
        Ok(v) => v,
        Err(_) => true,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// MediaStore scan (PRD 4.3)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(not(target_os = "android"))]
pub fn scan_gallery(_folder: Option<&str>) -> Result<Vec<NativeMediaEntry>, String> {
    // Desktop: scan a user-chosen directory (handled by the frontend file
    // picker); nothing to enumerate here.
    Ok(Vec::new())
}

#[cfg(target_os = "android")]
pub fn scan_gallery(folder: Option<&str>) -> Result<Vec<NativeMediaEntry>, String> {
    let json = jni_call_string("scanMediaStore", &[folder.unwrap_or("").to_string()])?;
    serde_json::from_str(&json).map_err(|e| format!("Scan galeri gagal: {}", e))
}

/// Registers a ContentObserver so the Rust side is notified of new media.
#[cfg(target_os = "android")]
pub fn register_content_observer() -> Result<(), String> {
    jni_call_string("registerContentObserver", &["".to_string()]).map(|_| ())
}

#[cfg(not(target_os = "android"))]
pub fn register_content_observer() -> Result<(), String> {
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// JNI helpers (Android only)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(target_os = "android")]
fn jni_call_boolean(method: &str, args: &[bool]) -> Result<bool, String> {
    let ctx = ndk_context::android_context();
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) }
        .map_err(|e| format!("JVM gagal: {}", e))?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|e| format!("JNI attach gagal: {}", e))?;

    let class = env
        .find_class("com/telegram/photos/MediaPlugin")
        .map_err(|e| e.to_string())?;
    let sig = format!("(ZZ)Z");
    let method_id = env
        .get_static_method_id(class, method, &sig)
        .map_err(|e| e.to_string())?;
    let result = env
        .call_static_method(class, method_id, &jni::objects::JValue::from(false), &[])
        .map_err(|e| e.to_string())?;
    // Re-invoke with the real args (jni API varies by version; use the simple path)
    let _ = result;
    env.call_static_method(
        class,
        method_id,
        &jni::objects::JValue::from(false),
        &[
            jni::objects::JValue::Bool(args.first().copied().unwrap_or(false) as u8 != 0),
            jni::objects::JValue::Bool(args.get(1).copied().unwrap_or(false) as u8 != 0),
        ],
    )
    .map(|v| v.z().unwrap_or(false))
    .map_err(|e| e.to_string())
}

#[cfg(target_os = "android")]
fn jni_call_string(method: &str, args: &[String]) -> Result<String, String> {
    let ctx = ndk_context::android_context();
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) }
        .map_err(|e| format!("JVM gagal: {}", e))?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|e| format!("JNI attach gagal: {}", e))?;
    let class = env
        .find_class("com/telegram/photos/MediaPlugin")
        .map_err(|e| e.to_string())?;

    if method == "scanMediaStore" {
        let sig = "(Ljava/lang/String;)Ljava/lang/String;";
        let method_id = env
            .get_static_method_id(class, method, sig)
            .map_err(|e| e.to_string())?;
        let arg = env
            .new_string(args.first().map(String::as_str).unwrap_or(""))
            .map_err(|e| e.to_string())?;
        let result = env
            .call_static_method(
                class,
                method_id,
                &jni::objects::JValue::from(false),
                &[jni::objects::JValue::Object(arg.into())],
            )
            .map_err(|e| e.to_string())?;
        let obj = result.l().map_err(|e| e.to_string())?;
        let s = env.get_string(&jni::objects::JString::from(obj)).map_err(|e| e.to_string())?;
        return Ok(s.into());
    }

    // registerContentObserver etc.: fire-and-forget
    let sig = "(Ljava/lang/String;)Ljava/lang/String;";
    let method_id = env
        .get_static_method_id(class, method, sig)
        .map_err(|e| e.to_string())?;
    let arg = env
        .new_string(args.first().map(String::as_str).unwrap_or(""))
        .map_err(|e| e.to_string())?;
    let _ = env.call_static_method(
        class,
        method_id,
        &jni::objects::JValue::from(false),
        &[jni::objects::JValue::Object(arg.into())],
    );
    Ok(String::new())
}
