//! Telegram MTProto integration built on Grammers (the Rust Telegram client).
//!
//! Pattern follows the Telegram-Drive reference: a `SenderPool` runner owned by
//! Tauri's async runtime, a SQLite-backed session, phone/QR auth with 2FA, a
//! private storage channel as the vault, and chunked uploads with progress and
//! FLOOD_WAIT resilience (PRD section 4.2, 4.4, 8.2).

pub mod upload;
pub mod vault;

use crate::db::Db;
use crate::models::{AuthCodeResult, TelegramUser, VaultInfo};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use grammers_client::types::Peer;
use grammers_client::Client;
use grammers_mtsender::SenderPool;
use grammers_session::storages::SqliteSession;
use grammers_tl_types as tl;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};
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
    /// Cached vault channel peer (avoids dialog scans on every upload).
    pub vault_peer: std::sync::Mutex<Option<Peer>>,
    /// Set of transfer ids marked for cancellation.
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

/// Ensures the Grammers client is initialized with the given API ID, properly
/// shutting down any previous runner to avoid task accumulation (reference
/// pattern for preventing stack overflow). Resolves the app data dir through
/// Tauri's path resolver.
pub async fn ensure_client_initialized(
    app: &AppHandle,
    state: &TelegramState,
    api_id: i32,
) -> Result<Client, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;
    std::fs::create_dir_all(&app_data_dir).map_err(|e| e.to_string())?;
    ensure_client_initialized_with_dir(state, api_id, &app_data_dir).await
}

/// Same as [`ensure_client_initialized`] but with an explicit data directory.
/// Used by the background worker (which has no Tauri `AppHandle`).
pub async fn ensure_client_initialized_with_dir(
    state: &TelegramState,
    api_id: i32,
    app_data_dir: &std::path::Path,
) -> Result<Client, String> {
    #[cfg(target_os = "android")]
    {
        let mut count = 0;
        while ndk_context::android_context().vm().is_null()
            || ndk_context::android_context().context().is_null()
        {
            if count >= 200 {
                return Err("Timeout waiting for Android JNI context initialization.".into());
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
            count += 1;
        }
    }

    let mut client_guard = state.client.lock().await;
    if let Some(client) = client_guard.as_ref() {
        return Ok(client.clone());
    }

    // Signal the old runner to shut down before creating a new one.
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
    // Retry a few times in case the old runner still holds the DB lock.
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
    tauri::async_runtime::spawn(async move {
        tokio::select! {
            _ = runner.run() => log::info!("MTProto runner #{} exited", runner_num),
            _ = shutdown_rx => log::info!("MTProto runner #{} shutdown", runner_num),
        }
    });

    *client_guard = Some(client.clone());
    Ok(client)
}

pub(crate) async fn current_client(state: &TelegramState) -> Result<Client, String> {
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
            return Err("Masukkan nomor dalam format internasional, contoh +6281234567890.".into());
        }
    }
    let digits = normalized
        .strip_prefix('+')
        .ok_or_else(|| "Gunakan format internasional dengan kode negara (+62...).".to_string())?;
    if !(7..=15).contains(&digits.len()) || digits.starts_with('0') {
        return Err("Nomor telepon tidak valid.".to_string());
    }
    Ok(normalized)
}

fn map_auth_error(e: impl std::fmt::Display) -> String {
    let raw = e.to_string();
    let mappings = [
        ("API_ID_INVALID", "API ID / API Hash tidak valid. Periksa kembali di my.telegram.org."),
        ("API_ID_PUBLISHED_FLOOD", "API ID ini telah dipublikasikan dan dinonaktifkan Telegram. Buat kredensial baru."),
        ("PHONE_NUMBER_INVALID", "Nomor telepon ditolak Telegram."),
        ("PHONE_NUMBER_BANNED", "Nomor telepon ini telah diblokir Telegram."),
        ("PHONE_NUMBER_FLOOD", "Terlalu banyak permintaan kode untuk nomor ini. Tunggu sebentar."),
        ("PHONE_PASSWORD_FLOOD", "Terlalu banyak percobaan login. Tunggu sebentar."),
        ("PHONE_CODE_INVALID", "Kode verifikasi salah."),
        ("PHONE_CODE_EXPIRED", "Kode verifikasi kedaluwarsa. Minta kode baru."),
        ("SESSION_PASSWORD_NEEDED", "Akun ini memerlukan kata sandi 2FA."),
        ("FLOOD_WAIT", "Telegram meminta jeda (FloodWait). Coba lagi nanti."),
    ];
    for (needle, message) in mappings {
        if raw.contains(needle) {
            return message.to_string();
        }
    }
    raw
}

// ─────────────────────────────────────────────────────────────────────────────
// Commands
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn cmd_connect(
    app: AppHandle,
    state: State<'_, TelegramState>,
    api_id: i32,
    _api_hash: String,
) -> Result<bool, String> {
    *state.api_id.lock().await = Some(api_id);
    ensure_client_initialized(&app, &state, api_id).await?;
    Ok(true)
}

#[tauri::command]
pub async fn cmd_check_connection(state: State<'_, TelegramState>) -> Result<bool, String> {
    if let Some(client) = state.client.lock().await.as_ref().cloned() {
        if client.get_me().await.is_ok() {
            return Ok(true);
        }
    }
    Ok(false)
}

#[tauri::command]
pub async fn cmd_get_me(state: State<'_, TelegramState>) -> Result<Option<TelegramUser>, String> {
    let client = match current_client(&state).await {
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

#[tauri::command]
pub async fn cmd_auth_request_code(
    app: AppHandle,
    state: State<'_, TelegramState>,
    phone: String,
    api_id: i32,
    api_hash: String,
) -> Result<AuthCodeResult, String> {
    if api_hash.trim().is_empty() {
        return Err("API Hash tidak boleh kosong.".into());
    }
    let phone = normalize_phone_number(&phone)?;
    *state.login_token.lock().await = None;
    *state.password_token.lock().await = None;
    *state.api_id.lock().await = Some(api_id);

    let client = ensure_client_initialized(&app, &state, api_id).await?;

    let token = timeout(
        Duration::from_secs(30),
        client.request_login_code(&phone, &api_hash),
    )
    .await
    .map_err(|_| "Telegram tidak merespons saat meminta kode. Periksa koneksi.".to_string())?
    .map_err(|e| map_auth_error(e))?;

    *state.login_token.lock().await = Some(token);

    Ok(AuthCodeResult {
        status: "code_required".into(),
        code_length: Some(5),
        resend_after_seconds: Some(60),
        delivery: Some("telegram_app".into()),
    })
}

#[tauri::command]
pub async fn cmd_auth_sign_in(
    state: State<'_, TelegramState>,
    code: String,
) -> Result<AuthCodeResult, String> {
    let code = code.trim().to_string();
    if code.is_empty() {
        return Err("Masukkan kode verifikasi.".into());
    }
    let client = current_client(&state).await?;
    let token = state
        .login_token
        .lock()
        .await
        .take()
        .ok_or("Tidak ada sesi login aktif. Mulai lagi dengan nomor telepon.")?;

    match timeout(Duration::from_secs(30), client.sign_in(&token, &code)).await {
        Err(_) => Err("Telegram tidak merespons saat memverifikasi kode.".into()),
        Ok(Ok(_user)) => {
            Ok(AuthCodeResult {
                status: "authorized".into(),
                code_length: None,
                resend_after_seconds: None,
                delivery: None,
            })
        }
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
            Err("Nomor ini harus terdaftar di aplikasi Telegram resmi terlebih dahulu.".into())
        }
        Ok(Err(grammers_client::SignInError::InvalidCode)) => {
            Err("Kode verifikasi salah.".into())
        }
        Ok(Err(grammers_client::SignInError::InvalidPassword)) => {
            Err("Kata sandi 2FA salah.".into())
        }
        Ok(Err(grammers_client::SignInError::Other(e))) => Err(map_auth_error(e)),
    }
}

#[tauri::command]
pub async fn cmd_auth_check_password(
    state: State<'_, TelegramState>,
    password: String,
) -> Result<AuthCodeResult, String> {
    let client = current_client(&state).await?;
    let pw = state
        .password_token
        .lock()
        .await
        .take()
        .ok_or("Tidak ada sesi 2FA aktif.")?;
    match timeout(Duration::from_secs(30), client.check_password(pw, &password)).await {
        Err(_) => Err("Telegram tidak merespons saat memverifikasi kata sandi.".into()),
        Ok(Ok(_user)) => {
            Ok(AuthCodeResult {
                status: "authorized".into(),
                code_length: None,
                resend_after_seconds: None,
                delivery: None,
            })
        }
        Ok(Err(e)) => Err(format!("Verifikasi 2FA gagal: {}", map_auth_error(e))),
    }
}

/// QR login step 1: export a login token and return the `tg://login?token=...` URL.
#[tauri::command]
pub async fn cmd_auth_qr_login(
    app: AppHandle,
    state: State<'_, TelegramState>,
    api_id: i32,
    api_hash: String,
) -> Result<String, String> {
    if api_hash.trim().is_empty() {
        return Err("API Hash tidak boleh kosong.".into());
    }
    *state.api_id.lock().await = Some(api_id);
    *state.login_token.lock().await = None;
    *state.password_token.lock().await = None;

    let client = ensure_client_initialized(&app, &state, api_id).await?;

    let result = timeout(
        Duration::from_secs(30),
        client.invoke(&tl::functions::auth::ExportLoginToken {
            api_id,
            api_hash,
            except_ids: vec![],
        }),
    )
    .await
    .map_err(|_| "Telegram tidak merespons saat membuat token QR.".to_string())?
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

/// QR login step 2: poll until the session becomes authorized.
#[tauri::command]
pub async fn cmd_auth_qr_poll(state: State<'_, TelegramState>) -> Result<AuthCodeResult, String> {
    let client = match current_client(&state).await {
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

#[tauri::command]
pub async fn cmd_logout(app: AppHandle, state: State<'_, TelegramState>) -> Result<bool, String> {
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

    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(app_data_dir.join(format!("telegram.session{}", suffix)));
    }
    Ok(true)
}

#[tauri::command]
pub async fn cmd_get_vault(db: State<'_, Db>) -> Result<VaultInfo, String> {
    db.get_vault_info()
}
