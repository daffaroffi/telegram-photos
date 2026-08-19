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
pub async fn check_connection(handle: TelegramHandle, _app_data_dir: String) -> bool {
    let state = handle.inner.clone();
    let guard = state.client.lock().await;
    guard.is_some()
}

/// Request OTP code for phone login.
pub async fn auth_request_code(
    handle: TelegramHandle,
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
    handle: TelegramHandle,
    code: String,
) -> Result<AuthCodeResult, String> {
    telegram::auth_sign_in(&handle.inner, &code).await
}

/// Submit 2FA password.
pub async fn auth_check_password(
    handle: TelegramHandle,
    password: String,
) -> Result<AuthCodeResult, String> {
    telegram::auth_check_password(&handle.inner, &password).await
}

/// Start QR code login flow. Returns `tg://login?token=...` URL.
pub async fn auth_qr_login(
    handle: TelegramHandle,
    api_id: i32,
    api_hash: String,
    app_data_dir: String,
) -> Result<String, String> {
    let dir = std::path::PathBuf::from(&app_data_dir);
    telegram::auth_qr_login(&handle.inner, api_id, &api_hash, &dir).await
}

/// Poll QR login status. Returns status: "authorized" or "waiting".
pub async fn auth_qr_poll(handle: TelegramHandle) -> Result<AuthCodeResult, String> {
    telegram::auth_qr_poll(&handle.inner).await
}

/// Get current logged-in user info.
pub async fn get_me(
    handle: TelegramHandle,
) -> Result<Option<telegram_photos_core::models::TelegramUser>, String> {
    telegram::get_me(&handle.inner).await
}

/// Logout and delete session.
pub async fn logout(handle: TelegramHandle, app_data_dir: String) -> Result<bool, String> {
    let dir = std::path::PathBuf::from(&app_data_dir);
    telegram::logout(&handle.inner, &dir).await
}
