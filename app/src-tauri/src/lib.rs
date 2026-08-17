//! Telegram Photos — Tauri application entry point.
//!
//! Owns the SQLite database, the Grammers Telegram client, the crypto vault
//! key, the backup engine and the Google OAuth state, and registers every
//! backend command exposed to the React frontend.

pub mod android_media;
pub mod backup;
pub mod commands;
pub mod crypto;
pub mod db;
pub mod geo;
pub mod google;
pub mod media;
pub mod models;
pub mod telegram;

use backup::BackupState;
use crypto::VaultState;
use db::Db;
use google::{GoogleOAuthState, ImportState};
use tauri::Manager;
use telegram::TelegramState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .manage(TelegramState::default())
        .manage(VaultState::default())
        .manage(BackupState::default())
        .manage(GoogleOAuthState::default())
        .manage(ImportState::default())
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app data dir");
            let db = Db::open(&app_data_dir.join("telegram_photos.db"))
                .expect("failed to open telegram_photos.db");
            app.manage(db);

            // On Android, register the ContentObserver for real-time media
            // change notifications (PRD 4.3).
            #[cfg(target_os = "android")]
            let _ = android_media::register_content_observer();

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Telegram auth & connection
            telegram::cmd_connect,
            telegram::cmd_check_connection,
            telegram::cmd_get_me,
            telegram::cmd_auth_request_code,
            telegram::cmd_auth_sign_in,
            telegram::cmd_auth_check_password,
            telegram::cmd_auth_qr_login,
            telegram::cmd_auth_qr_poll,
            telegram::cmd_logout,
            telegram::cmd_get_vault,
            telegram::vault::cmd_get_or_create_vault,
            // Backup engine
            backup::cmd_run_backup,
            backup::cmd_cancel_backup,
            backup::cmd_backup_status,
            backup::cmd_calculate_free_up_space,
            backup::cmd_execute_free_up_space,
            backup::cmd_restore_media,
            // Google Photos importer
            google::cmd_google_start_oauth,
            google::cmd_google_wait_oauth,
            google::cmd_google_disconnect,
            google::cmd_google_status,
            google::cmd_google_discover,
            google::cmd_google_start_import,
            google::cmd_google_cancel_import,
            google::cmd_google_post_import,
            // Settings & media
            commands::cmd_get_settings,
            commands::cmd_save_settings,
            commands::cmd_list_timeline,
            commands::cmd_list_all_media,
            commands::cmd_get_media,
            commands::cmd_count_media,
            commands::cmd_list_albums,
            commands::cmd_search_media,
            commands::cmd_add_local_files,
            commands::cmd_scan_folder,
            commands::cmd_scan_gallery_android,
            commands::cmd_batch_toggle_favorite,
            commands::cmd_batch_trash,
            commands::cmd_batch_queue_backup,
            commands::cmd_purge_trash,
            commands::cmd_delete_media_from_telegram,
            // Crypto vault
            crypto::cmd_vault_setup,
            crypto::cmd_vault_unlock,
            crypto::cmd_vault_lock,
            crypto::cmd_vault_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
