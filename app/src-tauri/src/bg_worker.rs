//! Background auto-backup worker (PRD section 4.4).
//!
//! Android's WorkManager cannot run WebView code, so the periodic worker
//! (`com.telegramphotos.app.BackgroundWorker`) calls into this Rust export via
//! JNI. The worker opens the same SQLite database and Telegram session used by
//! the UI, reuses the backup state machine from `backup.rs`, and reports
//! progress back to Kotlin so a foreground notification can be updated.
//!
//! Security: if client-side encryption is enabled, the vault is locked in the
//! background (no passphrase available), so encrypted items stay queued and
//! are only uploaded once the user unlocks the vault from the UI.

#[cfg(target_os = "android")]
use crate::backup::{run_backup_core, BackupContext};
#[cfg(target_os = "android")]
use crate::crypto::VaultState;
#[cfg(target_os = "android")]
use crate::db::Db;
#[cfg(target_os = "android")]
use crate::models::BackupProgressEvent;
#[cfg(target_os = "android")]
use crate::telegram::{self, TelegramState};
#[cfg(target_os = "android")]
use std::path::PathBuf;
#[cfg(target_os = "android")]
use std::sync::atomic::AtomicBool;
#[cfg(target_os = "android")]
use std::sync::Arc;

/// Initializes the JNI context so `with_env` (android_media.rs) and the
/// background worker can attach to the JVM.
///
/// Tauri's tao/wry runtimes do NOT seed the `ndk-context` crate, so without
/// this hook any call to `ndk_context::android_context()` panics with
/// "android context was not initialized" (crashing the app on startup via
/// `tauri::mobile_entry_point`'s `stop_unwind`). `JNI_OnLoad` runs as soon as
/// the native library is loaded by `TauriActivity`, i.e. before `setup()`.
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn JNI_OnLoad(
    vm: *mut jni::sys::JavaVM,
    _reserved: *mut std::ffi::c_void,
) -> jni::sys::jint {
    use jni::sys::JNI_VERSION_1_6;
    unsafe {
        ndk_context::initialize_android_context(
            vm as *mut std::ffi::c_void,
            std::ptr::null_mut(),
        );
    }
    JNI_VERSION_1_6
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_com_telegramphotos_app_BackgroundWorker_runBackup(
    env: *mut jni::sys::JNIEnv,
    _class: jni::sys::jclass,
    data_dir: jni::sys::jstring,
) -> jni::sys::jint {
    use jni::objects::JString;

    let result = std::panic::catch_unwind(|| {
        let mut env = match unsafe { jni::JNIEnv::from_raw(env) } {
            Ok(e) => e,
            Err(_) => return -1,
        };
        let dir: String = match env.get_string(&unsafe { JString::from_raw(data_dir) }) {
            Ok(s) => s.into(),
            Err(_) => return -1,
        };

        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(_) => return -1,
        };
        rt.block_on(run_worker_backup(PathBuf::from(dir)))
    });

    match result {
        Ok(code) => code,
        Err(_) => -1,
    }
}

#[cfg(target_os = "android")]
async fn run_worker_backup(data_dir: PathBuf) -> i32 {
    let Ok(db) = Db::open(&data_dir.join("telegram_photos.db")) else {
        return 0;
    };
    let Ok(settings) = db.get_settings() else {
        return 0;
    };
    // Only run when auto-backup is enabled in the app settings.
    if !settings.auto_backup_enabled {
        return 0;
    }
    // Respect Wi-Fi / charging constraints via the native plugin.
    if !crate::android_media::constraints_ok(
        settings.backup_over_wifi_only,
        settings.backup_while_charging_only,
    ) {
        return 0;
    }
    // A Telegram session must already exist (user has logged in).
    if !data_dir.join("telegram.session").exists() {
        return 0;
    }
    let Some(api_id_str) = settings.telegram_api_id.clone() else {
        return 0;
    };
    let Ok(api_id) = api_id_str.trim().parse::<i32>() else {
        return 0;
    };

    let tg_state = TelegramState::default();
    let Ok(client) = telegram::ensure_client_initialized_with_dir(&tg_state, api_id, &data_dir).await
    else {
        return 0;
    };
    // Not logged in — nothing to back up.
    if !client.is_authorized().await.unwrap_or(false) {
        return 0;
    }

    let vault_state = VaultState::default();
    let cancel = Arc::new(AtomicBool::new(false));
    let cache_dir = data_dir.join("cache");
    let _ = std::fs::create_dir_all(&cache_dir);

    let ctx = BackupContext {
        db: &db,
        tg_state: &tg_state,
        vault_state: &vault_state,
        cache_dir,
        cancel: &cancel,
        on_event: &notify_progress,
    };

    match run_backup_core(&ctx).await {
        Ok(count) => {
            notify_done(count);
            count.min(i32::MAX as i64) as i32
        }
        Err(_) => -1,
    }
}

/// Updates the progress notification. Attaches to the JVM on the calling
/// (worker) thread, so no `JNIEnv` needs to be captured by the callback.
#[cfg(target_os = "android")]
fn notify_progress(event: &BackupProgressEvent) {
    let _ = (|| -> Result<(), String> {
        let ctx = ndk_context::android_context();
        let vm = unsafe { jni::JavaVM::from_raw(ctx.vm() as *mut jni::sys::JavaVM) }
            .map_err(|e| e.to_string())?;
        let mut env = vm
            .attach_current_thread()
            .map_err(|e| e.to_string())?;
        let class = crate::android_media::media_plugin_class(&mut env)
            .map_err(|e| e.to_string())?;
        let file = env
            .new_string(&event.file_name)
            .map_err(|e| e.to_string())?;
        let status = env.new_string(&event.status).map_err(|e| e.to_string())?;
        env.call_static_method(
            class,
            "reportProgress",
            "(Ljava/lang/String;ILjava/lang/String;)V",
            &[
                jni::objects::JValue::Object(&file),
                jni::objects::JValue::Int(event.percent.min(i32::MAX as i64) as i32),
                jni::objects::JValue::Object(&status),
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })();
}

/// Posts the completion summary notification.
#[cfg(target_os = "android")]
fn notify_done(count: i64) {
    let _ = (|| -> Result<(), String> {
        let ctx = ndk_context::android_context();
        let vm = unsafe { jni::JavaVM::from_raw(ctx.vm() as *mut jni::sys::JavaVM) }
            .map_err(|e| e.to_string())?;
        let mut env = vm
            .attach_current_thread()
            .map_err(|e| e.to_string())?;
        let class = crate::android_media::media_plugin_class(&mut env)
            .map_err(|e| e.to_string())?;
        env.call_static_method(
            class,
            "notifyBackupDone",
            "(I)V",
            &[jni::objects::JValue::Int(count.min(i32::MAX as i64) as i32)],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })();
}

// Desktop: no background worker.
#[cfg(not(target_os = "android"))]
pub fn _desktop_noop() {}
