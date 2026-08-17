//! Telegram Photos — Tauri application entry point.
//!
//! Owns the SQLite database and the Grammers Telegram client state, and
//! registers all backend commands exposed to the React frontend.

pub mod db;
pub mod models;
pub mod telegram;

use db::Db;
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
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app data dir");
            let db = Db::open(&app_data_dir.join("telegram_photos.db"))
                .expect("failed to open telegram_photos.db");
            app.manage(db);
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}


