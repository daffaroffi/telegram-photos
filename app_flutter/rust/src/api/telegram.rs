//! FRB bridge for Telegram MTProto functions.

use telegram_photos_core::models::AuthCodeResult;

use crate::telegram;

// ─────────────────────────────────────────────────────────────────────────────
// Global state
// ─────────────────────────────────────────────────────────────────────────────

/// Opaque handle to the Telegram state. Dart holds this via Arc.
pub struct TelegramHandle {
    pub(crate) inner: std::sync::Arc<telegram::TelegramState>,
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
pub async fn check_connection(handle: &TelegramHandle, app_data_dir: String) -> bool {
    let state = &handle.inner;
    {
        let guard = state.client.lock().await;
        if let Some(client) = guard.as_ref() {
            return client.is_authorized().await.unwrap_or(false);
        }
    }
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

/// Get vault info (file count, size).
pub async fn get_vault_info() -> telegram_photos_core::models::VaultInfo {
    let db = crate::api::db::db();
    match db {
        Ok(db) => telegram::get_vault_info(db).await.unwrap_or_default(),
        Err(_) => telegram_photos_core::models::VaultInfo::default(),
    }
}

/// Upload a single photo file to the Telegram vault channel.
/// If encryption is enabled in settings and vault is unlocked, encrypts
/// the file before upload. Returns the Telegram message ID on success.
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

    // Check if encryption is enabled and vault is unlocked.
    let settings = db.get_settings().unwrap_or_default();
    let vault_status = crate::api::crypto::vault_status()
        .unwrap_or(crate::api::crypto::VaultStatus {
            enabled: false,
            passphrase_set: false,
            unlocked: false,
        });

    let upload_path = if settings.client_encryption_enabled
        && vault_status.enabled
        && vault_status.unlocked
    {
        // Encrypt file before upload.
        let enc_path = format!("{}.tdenc", file_path);
        crate::api::crypto::encrypt_file(file_path.clone(), enc_path.clone())?;
        enc_path
    } else {
        file_path.clone()
    };

    let file_path_obj = std::path::PathBuf::from(&upload_path);
    let size = tokio::fs::metadata(&file_path_obj)
        .await
        .map_err(|e| format!("Failed to read file metadata: {}", e))?
        .len() as usize;
    let file = tokio::fs::File::open(&file_path_obj)
        .await
        .map_err(|e| format!("Failed to open file: {}", e))?;
    let boxed_reader: Box<dyn tokio::io::AsyncRead + Unpin + Send> = Box::new(file);

    let upload_name = if upload_path != file_path {
        format!("{}.tdenc", file_name)
    } else {
        file_name
    };

    let msg_id = telegram::upload::upload_stream_to_peer(
        &client,
        &vault_peer,
        boxed_reader,
        size,
        upload_name,
        &mime_type,
        is_video,
        None,
        |_, _| {},
    )
    .await?;

    // Cleanup encrypted temp file if we created one.
    if upload_path != file_path {
        let _ = std::fs::remove_file(&upload_path);
    }

    Ok(msg_id as i64)
}
