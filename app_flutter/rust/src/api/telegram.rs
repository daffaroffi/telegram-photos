//! FRB bridge for Telegram MTProto functions.

use telegram_photos_core::models::AuthCodeResult;

use crate::telegram;

// ─────────────────────────────────────────────────────────────────────────────
// Global state
// ─────────────────────────────────────────────────────────────────────────────

/// Opaque handle to the Telegram state. Dart holds this via Arc.
pub struct TelegramHandle {
    inner: std::sync::Arc<telegram::TelegramState>,
}

impl TelegramHandle {
    /// Creates a new handle. Called once from Dart at app startup.
    pub fn new() -> Self {
        Self {
            inner: std::sync::Arc::new(telegram::TelegramState::default()),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Auth flow
// ─────────────────────────────────────────────────────────────────────────────

/// Check if there is an existing authorized session (cold start).
/// Tries to restore from DB if no in-memory client exists.
pub async fn check_connection(handle: &TelegramHandle, app_data_dir: String) -> bool {
    let state = &handle.inner;
    // Quick check: is the client already initialized and authorized?
    {
        let guard = state.client.lock().await;
        if let Some(client) = guard.as_ref() {
            return client.is_authorized().await.unwrap_or(false);
        }
    }
    // Cold start: try to restore session from DB + session file.
    let dir = std::path::PathBuf::from(&app_data_dir);
    if dir.exists() {
        let db = crate::api::db::db();
        if let Ok(db) = db {
            match telegram::check_connection(state, db, &dir).await {
                Ok(authorized) => authorized,
                Err(_) => false,
            }
        } else {
            false
        }
    } else {
        false
    }
}

/// Request OTP code for phone login.
pub async fn auth_request_code(
    handle: &TelegramHandle,
    phone: String,
    api_id: i32,
    api_hash: String,
    app_data_dir: String,
) -> Result<AuthCodeResult, String> {
    let dir = std::path::PathBuf::from(&app_data_dir);
    telegram::auth_request_code(&handle.inner, &phone, api_id, &api_hash, &dir).await
}

/// Submit OTP code to sign in.
pub async fn auth_sign_in(
    handle: &TelegramHandle,
    code: String,
) -> Result<AuthCodeResult, String> {
    telegram::auth_sign_in(&handle.inner, &code).await
}

/// Submit 2FA password.
pub async fn auth_check_password(
    handle: &TelegramHandle,
    password: String,
) -> Result<AuthCodeResult, String> {
    telegram::auth_check_password(&handle.inner, &password).await
}

/// Start QR code login flow. Returns `tg://login?token=...` URL.
pub async fn auth_qr_login(
    handle: &TelegramHandle,
    api_id: i32,
    api_hash: String,
    app_data_dir: String,
) -> Result<String, String> {
    let dir = std::path::PathBuf::from(&app_data_dir);
    telegram::auth_qr_login(&handle.inner, api_id, &api_hash, &dir).await
}

/// Poll QR login status. Returns status: "authorized" or "waiting".
pub async fn auth_qr_poll(handle: &TelegramHandle) -> Result<AuthCodeResult, String> {
    telegram::auth_qr_poll(&handle.inner).await
}

/// Get current logged-in user info.
pub async fn get_me(
    handle: &TelegramHandle,
) -> Result<Option<telegram_photos_core::models::TelegramUser>, String> {
    telegram::get_me(&handle.inner).await
}

/// Logout and delete session.
pub async fn logout(handle: &TelegramHandle, app_data_dir: String) -> Result<bool, String> {
    let dir = std::path::PathBuf::from(&app_data_dir);
    telegram::logout(&handle.inner, &dir).await
}

/// Upload a single photo file to the Telegram vault channel.
/// Returns the Telegram message ID on success.
pub async fn upload_photo(
    handle: &TelegramHandle,
    file_path: String,
    file_name: String,
    mime_type: String,
    is_video: bool,
) -> Result<i64, String> {
    let client = telegram::current_client(&handle.inner).await?;
    let db = crate::api::db::db()?;
    // Ensure vault channel exists (creates if needed)
    let _ = telegram::vault::get_or_create_vault(&client, db, &handle.inner).await?;
    let vault_peer = telegram::vault::peer_from_vault(&client, &handle.inner).await?;
    let file_path = std::path::PathBuf::from(&file_path);
    let file_bytes = tokio::fs::read(&file_path)
        .await
        .map_err(|e| format!("Failed to read file: {}", e))?;
    let size = file_bytes.len();
    let reader = std::io::Cursor::new(file_bytes);
    let boxed_reader: Box<dyn tokio::io::AsyncRead + Unpin + Send> = Box::new(reader);
    let msg_id = telegram::upload::upload_stream_to_peer(
        &client,
        &vault_peer,
        boxed_reader,
        size,
        file_name,
        &mime_type,
        is_video,
        None,
        |_, _| {},
    )
    .await?;
    Ok(msg_id as i64)
}
