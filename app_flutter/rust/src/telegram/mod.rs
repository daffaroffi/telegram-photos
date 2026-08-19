//! Telegram MTProto integration built on Grammers (the Rust Telegram client).
//!
//! This module lives in the FRB bridge crate (not core) because it depends on
//! grammers which has yanked transitive deps that break core's resolution.

pub mod upload;
pub mod vault;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use grammers_client::types::Peer;
use grammers_client::Client;
use grammers_mtsender::SenderPool;
use grammers_session::storages::SqliteSession;
use grammers_tl_types as tl;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use telegram_photos_core::db::Db;
use telegram_photos_core::models::{AuthCodeResult, TelegramUser, VaultInfo};
use tokio::sync::{oneshot, Mutex};
use tokio::time::{timeout, Duration};

pub struct TelegramState {
    pub client: Mutex<Option<Client>>,
    pub session: Mutex<Option<Arc<SqliteSession>>>,
    pub api_id: Mutex<Option<i32>>,
    pub login_token: Mutex<Option<grammers_client::types::LoginToken>>,
    pub password_token: Mutex<Option<grammers_client::types::PasswordToken>>,
    pub runner_shutdown: std::sync::Mutex<Option<oneshot::Sender<()>>>,
    pub runner_count: AtomicU32,
    pub vault_peer: std::sync::Mutex<Option<Peer>>,
    pub cancelled_transfers: Arc<tokio::sync::RwLock<Vec<String>>>,
}
impl Default for TelegramState {
    fn default() -> Self {
        Self {
            client: Mutex::new(None),
            session: Mutex::new(None),
            api_id: Mutex::new(None),
            login_token: Mutex::new(None),
            password_token: Mutex::new(None),
            runner_shutdown: std::sync::Mutex::new(None),
            runner_count: AtomicU32::new(0),
            vault_peer: std::sync::Mutex::new(None),
            cancelled_transfers: Arc::new(tokio::sync::RwLock::new(Vec::new())),
        }
    }
}

pub async fn ensure_client_initialized(
    state: &TelegramState,
    api_id: i32,
    app_data_dir: &std::path::Path,
) -> Result<Client, String> {
    std::fs::create_dir_all(app_data_dir).map_err(|e| e.to_string())?;

    let mut client_guard = state.client.lock().await;
    if let Some(client) = client_guard.as_ref() {
        return Ok(client.clone());
    }

    let did_shutdown = {
        let mut guard = state.runner_shutdown.lock().unwrap();
        if let Some(tx) = guard.take() {
            let _ = tx.send(());
            true
        } else {
            false
        }
    };
    if did_shutdown {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let runner_num = state.runner_count.fetch_add(1, Ordering::SeqCst) + 1;
    let session_path = app_data_dir.join("telegram.session");
    let session_path_str = session_path.to_string_lossy().to_string();

    let mut session = SqliteSession::open(&session_path_str).map_err(|e| e.to_string())?;
    for _ in 0..5 {
        if let Ok(s) = SqliteSession::open(&session_path_str) {
            session = s;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let session = Arc::new(session);
    *state.session.lock().await = Some(session.clone());

    let pool = SenderPool::with_configuration(
        session,
        api_id,
        grammers_mtsender::ConnectionParams::default(),
    );
    let client = Client::new(&pool);

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    *state.runner_shutdown.lock().unwrap() = Some(shutdown_tx);

    let SenderPool { runner, .. } = pool;
    tokio::spawn(async move {
        tokio::select! {
            _ = runner.run() => log::info!("MTProto runner #{} exited", runner_num),
            _ = shutdown_rx => log::info!("MTProto runner #{} shutdown", runner_num),
        }
    });

    *client_guard = Some(client.clone());
    Ok(client)
}

/// Reset all client state and delete session files.
/// Used by AUTH_RESTART and AUTH_CLEAR handlers.
async fn reset_client_state(state: &TelegramState, app_data_dir: &std::path::Path) {
    {
        let mut guard = state.client.lock().await;
        *guard = None;
    }
    {
        let mut guard = state.session.lock().await;
        *guard = None;
    }
    {
        let mut guard = state.runner_shutdown.lock().unwrap();
        if let Some(tx) = guard.take() {
            let _ = tx.send(());
        }
    }
    // Delete all session files (main + WAL + SHM).
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(app_data_dir.join(format!("telegram.session{}", suffix)));
    }
    // Small delay to ensure I/O completes and runner exits.
    tokio::time::sleep(Duration::from_millis(300)).await;
}

pub async fn current_client(state: &TelegramState) -> Result<Client, String> {
    state
        .client
        .lock()
        .await
        .as_ref()
        .cloned()
        .ok_or_else(|| "Telegram client is not initialized.".to_string())
}

fn normalize_phone_number(input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    let mut normalized = String::with_capacity(trimmed.len());
    for (index, ch) in trimmed.chars().enumerate() {
        if ch.is_ascii_digit() || (ch == '+' && index == 0) {
            normalized.push(ch);
        } else if ch.is_whitespace() || matches!(ch, '-' | '(' | ')' | '.') {
            continue;
        } else {
            return Err("Use international format, e.g. +6281234567890".into());
        }
    }
    let digits = normalized
        .strip_prefix('+')
        .ok_or_else(|| "Use international format with country code (+62...)".to_string())?;
    if !(7..=15).contains(&digits.len()) || digits.starts_with('0') {
        return Err("Invalid phone number".to_string());
    }
    Ok(normalized)
}

fn map_auth_error(e: impl std::fmt::Display) -> String {
    let raw = e.to_string();
    let mappings = [
        ("AUTH_RESTART", "Session expired. Retrying..."),
        ("API_ID_INVALID", "Invalid API ID or API Hash"),
        ("API_ID_PUBLISHED_FLOOD", "API ID has been disabled by Telegram"),
        ("PHONE_NUMBER_INVALID", "Phone number rejected by Telegram"),
        ("PHONE_NUMBER_BANNED", "Phone number is banned"),
        ("PHONE_NUMBER_FLOOD", "Too many code requests. Please wait"),
        ("PHONE_PASSWORD_FLOOD", "Too many login attempts"),
        ("PHONE_CODE_INVALID", "Invalid verification code"),
        ("PHONE_CODE_EXPIRED", "Verification code expired"),
        ("SESSION_PASSWORD_NEEDED", "Account requires 2FA password"),
        ("FLOOD_WAIT", "Telegram requests a delay (FloodWait)"),
        ("PHONE_MIGRATE", "Phone number registered on different server. Reconnecting..."),
    ];
    for (needle, message) in mappings {
        if raw.contains(needle) {
            return message.to_string();
        }
    }
    raw
}

/// Check if an error is a transient auth error that can be retried.
fn is_retryable_auth_error(e: &str) -> bool {
    e.contains("AUTH_RESTART") || e.contains("AUTH_KEY_UNREGISTERED")
}

/// Core logic: try to request login code once with the given client.
async fn try_request_login_code(
    client: &Client,
    phone: &str,
    api_hash: &str,
) -> Result<grammers_client::types::LoginToken, String> {
    timeout(
        Duration::from_secs(30),
        client.request_login_code(phone, api_hash),
    )
    .await
    .map_err(|_| "Telegram did not respond. Check your connection.".to_string())?
    .map_err(|e| map_auth_error(e))
}

pub async fn auth_request_code(
    state: &TelegramState,
    phone: &str,
    api_id: i32,
    api_hash: &str,
    app_data_dir: &std::path::Path,
) -> Result<AuthCodeResult, String> {
    if api_hash.trim().is_empty() {
        return Err("API Hash must not be empty".into());
    }
    let phone = normalize_phone_number(phone)?;
    *state.login_token.lock().await = None;
    *state.password_token.lock().await = None;
    *state.api_id.lock().await = Some(api_id);

    // Save api_id and api_hash to file for session restore on cold start.
    let creds_path = app_data_dir.join(".telegram_creds");
    let _ = std::fs::write(&creds_path, format!("{}\n{}", api_id, api_hash));

    // Retry loop: up to 3 attempts for transient errors (AUTH_RESTART, AUTH_KEY_UNREGISTERED).
    const MAX_ATTEMPTS: u32 = 3;
    let mut last_error = String::new();

    for attempt in 1..=MAX_ATTEMPTS {
        let client = ensure_client_initialized(state, api_id, app_data_dir).await?;

        match try_request_login_code(&client, &phone, api_hash).await {
            Ok(token) => {
                *state.login_token.lock().await = Some(token);
                return Ok(AuthCodeResult {
                    status: "code_required".into(),
                    code_length: Some(5),
                    resend_after_seconds: Some(60),
                    delivery: Some("telegram_app".into()),
                });
            }
            Err(e) if is_retryable_auth_error(&e) && attempt < MAX_ATTEMPTS => {
                log::warn!(
                    "AUTH attempt {}/{} failed ({}): cleaning up and retrying...",
                    attempt,
                    MAX_ATTEMPTS,
                    &e
                );
                reset_client_state(state, app_data_dir).await;
                // Exponential backoff: 500ms, 1000ms.
                let delay_ms = 500 * attempt;
                tokio::time::sleep(Duration::from_millis(delay_ms as u64)).await;
                last_error = e;
            }
            Err(e) => {
                last_error = e;
                break;
            }
        }
    }

    Err(last_error)
}

pub async fn auth_sign_in(
    state: &TelegramState,
    code: &str,
) -> Result<AuthCodeResult, String> {
    let code = code.trim().to_string();
    if code.is_empty() {
        return Err("Enter verification code".into());
    }
    let client = current_client(state).await?;
    let token = state
        .login_token
        .lock()
        .await
        .take()
        .ok_or("No active login session")?;

    match timeout(Duration::from_secs(30), client.sign_in(&token, &code)).await {
        Err(_) => Err("Telegram did not respond".into()),
        Ok(Ok(_)) => Ok(AuthCodeResult {
            status: "authorized".into(),
            code_length: None,
            resend_after_seconds: None,
            delivery: None,
        }),
        Ok(Err(grammers_client::SignInError::PasswordRequired(pw))) => {
            *state.password_token.lock().await = Some(pw);
            Ok(AuthCodeResult {
                status: "password_required".into(),
                code_length: None,
                resend_after_seconds: None,
                delivery: None,
            })
        }
        Ok(Err(grammers_client::SignInError::SignUpRequired { .. })) => {
            Err("Phone number must be registered on official Telegram".into())
        }
        Ok(Err(grammers_client::SignInError::InvalidCode)) => Err("Invalid code".into()),
        Ok(Err(grammers_client::SignInError::InvalidPassword)) => Err("Invalid 2FA password.".into()),
        Ok(Err(grammers_client::SignInError::Other(e))) => Err(map_auth_error(e)),
    }
}

pub async fn auth_check_password(
    state: &TelegramState,
    password: &str,
) -> Result<AuthCodeResult, String> {
    let client = current_client(state).await?;
    let pw = state
        .password_token
        .lock()
        .await
        .take()
        .ok_or("No active 2FA session")?;
    match timeout(Duration::from_secs(30), client.check_password(pw, password)).await {
        Err(_) => Err("Telegram did not respond".into()),
        Ok(Ok(_)) => Ok(AuthCodeResult {
            status: "authorized".into(),
            code_length: None,
            resend_after_seconds: None,
            delivery: None,
        }),
        Ok(Err(e)) => Err(format!("2FA failed: {}", map_auth_error(e))),
    }
}

pub async fn auth_qr_login(
    state: &TelegramState,
    api_id: i32,
    api_hash: &str,
    app_data_dir: &std::path::Path,
) -> Result<String, String> {
    if api_hash.trim().is_empty() {
        return Err("API Hash must not be empty".into());
    }
    *state.api_id.lock().await = Some(api_id);
    *state.login_token.lock().await = None;
    *state.password_token.lock().await = None;

    let client = ensure_client_initialized(state, api_id, app_data_dir).await?;
    let result = timeout(
        Duration::from_secs(30),
        client.invoke(&tl::functions::auth::ExportLoginToken {
            api_id,
            api_hash: api_hash.to_string(),
            except_ids: vec![],
        }),
    )
    .await
    .map_err(|_| "Telegram did not respond while creating QR token")?
    .map_err(|e| map_auth_error(e))?;

    match result {
        tl::enums::auth::LoginToken::Token(t) => {
            let encoded = URL_SAFE_NO_PAD.encode(&t.token);
            Ok(format!("tg://login?token={}", encoded))
        }
        tl::enums::auth::LoginToken::Success(_) => Ok("__authorized__".to_string()),
        tl::enums::auth::LoginToken::MigrateTo(m) => {
            let encoded = URL_SAFE_NO_PAD.encode(&m.token);
            Ok(format!("tg://login?token={}", encoded))
        }
    }
}

pub async fn auth_qr_poll(state: &TelegramState) -> Result<AuthCodeResult, String> {
    let client = match current_client(state).await {
        Ok(c) => c,
        Err(_) => {
            return Ok(AuthCodeResult {
                status: "waiting".into(),
                code_length: None,
                resend_after_seconds: None,
                delivery: None,
            })
        }
    };
    match client.is_authorized().await {
        Ok(true) => {
            *state.login_token.lock().await = None;
            Ok(AuthCodeResult {
                status: "authorized".into(),
                code_length: None,
                resend_after_seconds: None,
                delivery: None,
            })
        }
        _ => Ok(AuthCodeResult {
            status: "waiting".into(),
            code_length: None,
            resend_after_seconds: None,
            delivery: None,
        }),
    }
}

pub async fn logout(
    state: &TelegramState,
    app_data_dir: &std::path::Path,
) -> Result<bool, String> {
    {
        let mut guard = state.runner_shutdown.lock().unwrap();
        if let Some(tx) = guard.take() {
            let _ = tx.send(());
        }
    }
    if let Some(client) = state.client.lock().await.as_ref().cloned() {
        let _ = client.sign_out().await;
    }
    *state.client.lock().await = None;
    *state.login_token.lock().await = None;
    *state.password_token.lock().await = None;
    *state.api_id.lock().await = None;
    *state.session.lock().await = None;

    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(app_data_dir.join(format!("telegram.session{}", suffix)));
    }
    Ok(true)
}

pub async fn check_connection(
    state: &TelegramState,
    db: &Db,
    app_data_dir: &std::path::Path,
) -> Result<bool, String> {
    if let Some(client) = state.client.lock().await.as_ref().cloned() {
        return Ok(client.get_me().await.is_ok());
    }
    // Try to restore from saved credentials file.
    let creds_path = app_data_dir.join(".telegram_creds");
    if let Ok(contents) = std::fs::read_to_string(&creds_path) {
        let lines: Vec<&str> = contents.lines().collect();
        if lines.len() >= 2 {
            if let Ok(api_id) = lines[0].trim().parse::<i32>() {
                let _api_hash = lines[1].trim();
                match ensure_client_initialized(state, api_id, app_data_dir).await {
                    Ok(client) => Ok(client.is_authorized().await.unwrap_or(false)),
                    Err(_) => Ok(false),
                }
            } else {
                Ok(false)
            }
        } else {
            Ok(false)
        }
    } else {
        Ok(false)
    }
}

pub async fn get_me(state: &TelegramState) -> Result<Option<TelegramUser>, String> {
    let client = match current_client(state).await {
        Ok(c) => c,
        Err(_) => return Ok(None),
    };
    match client.get_me().await {
        Ok(me) => {
            let is_premium = match &me.raw {
                tl::enums::User::User(u) => u.premium,
                _ => false,
            };
            Ok(Some(TelegramUser {
                id: me.bare_id(),
                first_name: me.first_name().unwrap_or("").to_string(),
                last_name: me.last_name().map(|s| s.to_string()),
                username: me.username().map(|s| s.to_string()),
                phone: me.phone().map(|p| p.to_string()).unwrap_or_default(),
                is_premium,
            }))
        }
        Err(_) => Ok(None),
    }
}

pub async fn get_vault_info(db: &Db) -> Result<VaultInfo, String> {
    db.get_vault_info()
}
