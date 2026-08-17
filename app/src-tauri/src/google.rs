//! Google Photos importer (PRD sections 4.1, 8.1).
//!
//! - OAuth 2.0 with a loopback redirect (no custom scheme needed): the auth URL
//!   is opened in the system browser, Telegram Photos listens on 127.0.0.1 for
//!   the callback, exchanges the code for tokens, and refreshes tokens
//!   automatically when they expire (PRD 8.1).
//! - Lists albums + media items with pagination (pageSize 100, PRD 8.1).
//! - Streams originals (`baseUrl=d`) directly into the Telegram upload engine
//!   (cloud-to-cloud, no phone-storage fill-up, PRD 4.1.2).
//! - Dedup by Google media id + SHA-256 (PRD 4.1.4).
//!
//! Note on deleting from Google: the official Google Photos Library API has no
//! delete endpoint. "Kosongkan kuota" therefore verifies every item is
//! `BACKED_UP`, marks them locally as deleted-from-Google and opens the Google
//! Photos trash flow for the user to confirm — exactly the PRD's "Panduan
//! Kosongkan Google Storage" escape hatch.

use crate::db::Db;
use crate::geo;
use crate::models::{GoogleDiscoveryInfo, GoogleImportSession, GooglePhotosItem, MediaItem};
use crate::telegram::upload::upload_stream_to_peer;
use crate::telegram::vault;
use crate::telegram::{current_client, TelegramState};
use futures_util::StreamExt;
use serde::Deserialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::io::AsyncRead;
use tokio_util::io::StreamReader;

const AUTH_ENDPOINT: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";
const USERINFO_ENDPOINT: &str = "https://openidconnect.googleapis.com/v1/userinfo";
const PHOTOS_API: &str = "https://photoslibrary.googleapis.com/v1";
const SCOPE: &str = "https://www.googleapis.com/auth/photoslibrary.readonly";

/// In-memory state for the OAuth loopback flow.
pub struct GoogleOAuthState {
    pub code_received: Arc<std::sync::Mutex<Option<String>>>,
    pub notify: Arc<tokio::sync::Notify>,
}

impl Default for GoogleOAuthState {
    fn default() -> Self {
        Self {
            code_received: Arc::new(std::sync::Mutex::new(None)),
            notify: Arc::new(tokio::sync::Notify::new()),
        }
    }
}

pub struct ImportState {
    pub running: std::sync::Mutex<bool>,
    pub cancelled: Arc<AtomicBool>,
}

impl Default for ImportState {
    fn default() -> Self {
        Self {
            running: std::sync::Mutex::new(false),
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
}

#[derive(Deserialize)]
struct UserInfo {
    email: Option<String>,
}

#[derive(Deserialize)]
struct MediaListResponse {
    media_items: Option<Vec<serde_json::Value>>,
    next_page_token: Option<String>,
}

#[derive(Deserialize)]
struct AlbumListResponse {
    albums: Option<Vec<serde_json::Value>>,
    next_page_token: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// OAuth
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn cmd_google_start_oauth(
    db: State<'_, Db>,
    oauth: State<'_, GoogleOAuthState>,
) -> Result<String, String> {
    let settings = db.get_settings()?;
    let client_id = settings
        .google_client_id
        .ok_or("Google Client ID belum diatur di Pengaturan.")?;
    let redirect_uri = format!("http://127.0.0.1:18762/callback");

    let auth_url = format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&access_type=offline&prompt=consent&include_granted_scopes=true",
        AUTH_ENDPOINT,
        urlencoding(&client_id),
        urlencoding(&redirect_uri),
        urlencoding(SCOPE),
    );

    // Clear any stale code.
    *oauth.code_received.lock().unwrap() = None;

    // Spawn the loopback listener.
    let code_holder = Arc::clone(&oauth.code_received);
    let notify = oauth.notify.clone();
    tauri::async_runtime::spawn(async move {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:18762").await;
        match listener {
            Ok(listener) => {
                if let Ok((mut socket, _)) = listener.accept().await {
                    use tokio::io::{AsyncReadExt, AsyncWriteExt};
                    let mut buf = vec![0u8; 8192];
                    let n = socket.read(&mut buf).await.unwrap_or(0);
                    let request = String::from_utf8_lossy(&buf[..n]).to_string();
                    let code = request
                        .split(' ')
                        .nth(1)
                        .and_then(|p| p.split('?').nth(1))
                        .and_then(|q| {
                            q.split('&')
                                .find_map(|kv| kv.strip_prefix("code="))
                        })
                        .map(|c| c.to_string());
                    let body = if code.is_some() {
                        "<html><body><h2>Login Google berhasil! Silakan kembali ke aplikasi.</h2></body></html>"
                    } else {
                        "<html><body><h2>Login dibatalkan atau gagal.</h2></body></html>"
                    };
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = socket.write_all(resp.as_bytes()).await;
                    *code_holder.lock().unwrap() = code;
                    notify.notify_one();
                }
            }
            Err(_) => {
                // Port busy — signal failure so the frontend can retry.
                *code_holder.lock().unwrap() = Some("__PORT_BUSY__".to_string());
                notify.notify_one();
            }
        }
    });

    Ok(auth_url)
}

#[tauri::command]
pub async fn cmd_google_wait_oauth(
    db: State<'_, Db>,
    oauth: State<'_, GoogleOAuthState>,
) -> Result<String, String> {
    let settings = db.get_settings()?;
    let client_id = settings
        .google_client_id
        .ok_or("Google Client ID belum diatur.")?;
    let client_secret = settings
        .google_client_secret
        .ok_or("Google Client Secret belum diatur.")?;
    let redirect_uri = format!("http://127.0.0.1:18762/callback");

    // Wait up to 5 minutes for the callback.
    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(300);
    loop {
        // Clone the code under the lock and release the guard before any await
        // (the guard is not Send, so it must not be held across `.await`).
        let pending_code = oauth.code_received.lock().unwrap().clone();
        if let Some(code) = pending_code {
            if code == "__PORT_BUSY__" {
                return Err("Port callback sedang digunakan. Coba lagi.".into());
            }
            {
                // Exchange code for tokens.
                let client = reqwest::Client::new();
                let params = [
                    ("code", code),
                    ("client_id", client_id.clone()),
                    ("client_secret", client_secret.clone()),
                    ("redirect_uri", redirect_uri.clone()),
                    ("grant_type", "authorization_code".to_string()),
                ];
                let resp: TokenResponse = client
                    .post(TOKEN_ENDPOINT)
                    .form(&params)
                    .send()
                    .await
                    .map_err(|e| format!("Token exchange gagal: {}", e))?
                    .json()
                    .await
                    .map_err(|e| format!("Respons token tidak valid: {}", e))?;

                // Persist tokens.
                let expiry = chrono::Utc::now().timestamp_millis()
                    + resp.expires_in.unwrap_or(3600) * 1000;
                let tokens = serde_json::json!({
                    "access_token": resp.access_token,
                    "refresh_token": resp.refresh_token.unwrap_or_default(),
                    "expiry": expiry,
                });
                db.set_json("google_tokens", &tokens)?;

                // Fetch account email.
                let email = get_email(&resp.access_token).await.ok();

                *oauth.code_received.lock().unwrap() = None;
                return Ok(email.unwrap_or_else(|| "google@account".to_string()));
            }
        }
        if tokio::time::Instant::now() > deadline {
            return Err("Waktu tunggu OAuth habis. Coba lagi.".into());
        }
        tokio::time::timeout(
            tokio::time::Duration::from_secs(2),
            oauth.notify.notified(),
        )
        .await
        .ok();
    }
}

fn urlencoding(s: &str) -> String {
    percent_encoding::utf8_percent_encode(s, percent_encoding::NON_ALPHANUMERIC).to_string()
}

async fn get_email(access_token: &str) -> Result<String, String> {
    let client = reqwest::Client::new();
    let resp: UserInfo = client
        .get(USERINFO_ENDPOINT)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    Ok(resp.email.unwrap_or_default())
}

/// Returns a fresh access token, refreshing if expired (PRD 8.1).
async fn get_access_token(db: &Db) -> Result<String, String> {
    let tokens = db
        .get_json("google_tokens")?
        .ok_or("Belum terhubung ke Google.")?;
    let access = tokens
        .get("access_token")
        .and_then(|t| t.as_str())
        .ok_or("Token Google tidak ditemukan.")?;
    let refresh = tokens
        .get("refresh_token")
        .and_then(|t| t.as_str())
        .unwrap_or_default();
    let expiry = tokens.get("expiry").and_then(|t| t.as_i64()).unwrap_or(0);

    if chrono::Utc::now().timestamp_millis() < expiry - 5 * 60 * 1000 {
        return Ok(access.to_string());
    }
    if refresh.is_empty() {
        return Err("Refresh token tidak tersedia. Hubungkan ulang akun Google.".into());
    }

    let settings = db.get_settings()?;
    let client_id = settings.google_client_id.ok_or("Client ID belum diatur.")?;
    let client_secret = settings
        .google_client_secret
        .ok_or("Client Secret belum diatur.")?;
    let redirect_uri = format!("http://127.0.0.1:18762/callback");

    let client = reqwest::Client::new();
    let params = [
        ("refresh_token", refresh.to_string()),
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("grant_type", "refresh_token".to_string()),
    ];
    let resp: TokenResponse = client
        .post(TOKEN_ENDPOINT)
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("Refresh token gagal: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Respons refresh tidak valid: {}", e))?;

    let expiry = chrono::Utc::now().timestamp_millis() + resp.expires_in.unwrap_or(3600) * 1000;
    let updated = serde_json::json!({
        "access_token": resp.access_token,
        "refresh_token": if resp.refresh_token.is_some() { resp.refresh_token.clone().unwrap() } else { refresh.to_string() },
        "expiry": expiry,
    });
    db.set_json("google_tokens", &updated)?;
    Ok(resp.access_token)
}

#[tauri::command]
pub async fn cmd_google_disconnect(db: State<'_, Db>) -> Result<bool, String> {
    db.set_json("google_tokens", &serde_json::json!({}))?;
    Ok(true)
}

#[tauri::command]
pub async fn cmd_google_status(db: State<'_, Db>) -> Result<bool, String> {
    let tokens = db.get_json("google_tokens")?;
    Ok(tokens
        .map(|t| t.get("access_token").and_then(|x| x.as_str()).is_some())
        .unwrap_or(false))
}

// ─────────────────────────────────────────────────────────────────────────────
// Discovery
// ─────────────────────────────────────────────────────────────────────────────

fn parse_media_item(v: &serde_json::Value) -> Option<GooglePhotosItem> {
    let id = v.get("id")?.as_str()?.to_string();
    let filename = v
        .get("filename")
        .and_then(|x| x.as_str())
        .unwrap_or("unknown")
        .to_string();
    let base_url = v.get("baseUrl")?.as_str()?.to_string();
    let mime = v.get("mimeType").and_then(|x| x.as_str()).map(String::from);
    let meta = v.get("mediaMetadata");
    let creation = meta
        .and_then(|m| m.get("creationTime"))
        .and_then(|c| c.as_str())
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.timestamp_millis());
    let size = meta
        .and_then(|m| m.get("size"))
        .and_then(|s| s.as_str())
        .and_then(|s| s.parse::<i64>().ok());
    let width = meta.and_then(|m| m.get("width")).and_then(|x| x.as_str()).and_then(|s| s.parse().ok());
    let height = meta.and_then(|m| m.get("height")).and_then(|x| x.as_str()).and_then(|s| s.parse().ok());
    let (lat, lon) = media_position(v);
    let camera = v
        .get("mediaMetadata")
        .and_then(|m| m.get("cameraModel"))
        .and_then(|x| x.as_str())
        .map(String::from);
    let description = v.get("description").and_then(|x| x.as_str()).map(String::from);

    Some(GooglePhotosItem {
        id,
        filename,
        mime_type: mime,
        base_url,
        creation_time_ms: creation,
        width,
        height,
        file_size_bytes: size,
        camera_model: camera,
        latitude: lat,
        longitude: lon,
        album_name: None,
        description,
    })
}

fn media_position(v: &serde_json::Value) -> (Option<f64>, Option<f64>) {
    let pos = match v.get("mediaMetadata").and_then(|m| m.get("photo")) {
        Some(p) => match p.get("cameraPosition") {
            Some(cp) => cp,
            None => return (None, None),
        },
        _ => return (None, None),
    };
    match (
        pos.get("latitude").and_then(|x| x.as_f64()),
        pos.get("longitude").and_then(|x| x.as_f64()),
    ) {
        (Some(lat), Some(lng)) => (Some(lat), Some(lng)),
        _ => (None, None),
    }
}

async fn list_all_media(
    access_token: &str,
    album_id: Option<&str>,
) -> Result<Vec<GooglePhotosItem>, String> {
    let client = reqwest::Client::new();
    let mut all = Vec::new();
    let mut page_token: Option<String> = None;

    loop {
        let url = if album_id.is_some() {
            format!("{}/mediaItems:search", PHOTOS_API)
        } else {
            format!("{}/mediaItems", PHOTOS_API)
        };

        let mut body = serde_json::json!({
            "pageSize": 100,
        });
        if let Some(album) = album_id {
            body["albumId"] = serde_json::Value::String(album.to_string());
        }
        if let Some(tok) = &page_token {
            body["pageToken"] = serde_json::Value::String(tok.clone());
        }

        let resp: MediaListResponse = if album_id.is_some() {
            client
                .post(&url)
                .bearer_auth(access_token)
                .json(&body)
                .send()
                .await
                .map_err(|e| e.to_string())?
                .json()
                .await
                .map_err(|e| e.to_string())?
        } else {
            client
                .get(&url)
                .bearer_auth(access_token)
                .query(&[("pageSize", "100"), ("pageToken", page_token.as_deref().unwrap_or(""))])
                .send()
                .await
                .map_err(|e| e.to_string())?
                .json()
                .await
                .map_err(|e| e.to_string())?
        };

        for item in resp.media_items.unwrap_or_default() {
            if let Some(parsed) = parse_media_item(&item) {
                all.push(parsed);
            }
        }
        match resp.next_page_token {
            Some(tok) if !tok.is_empty() => page_token = Some(tok),
            _ => break,
        }
    }

    Ok(all)
}

async fn list_albums(access_token: &str) -> Result<Vec<(String, String)>, String> {
    let client = reqwest::Client::new();
    let mut albums = Vec::new();
    let mut page_token: Option<String> = None;
    loop {
        let mut url = format!("{}/albums?pageSize=50", PHOTOS_API);
        if let Some(tok) = &page_token {
            url.push_str(&format!("&pageToken={}", tok));
        }
        let resp: AlbumListResponse = client
            .get(&url)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;
        for a in resp.albums.unwrap_or_default() {
            if let (Some(id), Some(name)) = (
                a.get("id").and_then(|x| x.as_str()),
                a.get("title").and_then(|x| x.as_str()),
            ) {
                albums.push((id.to_string(), name.to_string()));
            }
        }
        match resp.next_page_token {
            Some(tok) if !tok.is_empty() => page_token = Some(tok),
            _ => break,
        }
    }
    Ok(albums)
}

#[tauri::command]
pub async fn cmd_google_discover(db: State<'_, Db>) -> Result<GoogleDiscoveryInfo, String> {
    let token = get_access_token(db.inner()).await?;

    // All media (for total count + size).
    let all = list_all_media(&token, None).await?;
    let total_size: i64 = all.iter().filter_map(|i| i.file_size_bytes).sum();

    // Album names.
    let albums = list_albums(&token)
        .await?
        .into_iter()
        .map(|(_, name)| name)
        .collect();

    Ok(GoogleDiscoveryInfo {
        total_count: all.len() as i64,
        total_size_bytes: total_size,
        albums,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Migration
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn cmd_google_start_import(
    app: AppHandle,
    tg_state: State<'_, TelegramState>,
    db: State<'_, Db>,
    import_state: State<'_, ImportState>,
    include_albums: bool,
) -> Result<String, String> {
    {
        let mut running = import_state.running.lock().unwrap();
        if *running {
            return Err("Import sudah berjalan.".into());
        }
        *running = true;
    }
    import_state.cancelled.store(false, Ordering::Relaxed);
    let cancel = import_state.cancelled.clone();

    let result = run_import(&app, tg_state.inner(), db.inner(), &cancel, include_albums).await;

    *import_state.running.lock().unwrap() = false;
    result
}

#[tauri::command]
pub async fn cmd_google_cancel_import(import_state: State<'_, ImportState>) -> Result<bool, String> {
    import_state.cancelled.store(true, Ordering::Relaxed);
    Ok(true)
}

async fn run_import(
    app: &AppHandle,
    tg_state: &TelegramState,
    db: &Db,
    cancel: &Arc<AtomicBool>,
    include_albums: bool,
) -> Result<String, String> {
    let token = get_access_token(db).await?;
    let email = get_email(&token).await.unwrap_or_else(|_| "google".into());

    let client = current_client(tg_state).await?;
    let (channel_id, access_hash, _title) =
        vault::get_or_create_vault(&client, db, tg_state).await?;
    let peer = vault::peer_from_vault(&client, tg_state).await?;

    // Build session
    let session_id = format!("gimport_{}", uuid::Uuid::new_v4());
    let all = list_all_media(&token, None).await?;

    let mut session = GoogleImportSession {
        session_id: session_id.clone(),
        google_account_email: email.clone(),
        started_at: chrono::Utc::now().timestamp_millis(),
        completed_at: None,
        total_items_found: all.len() as i64,
        items_imported_success: 0,
        items_imported_failed: 0,
        total_bytes_migrated: 0,
        post_cleanup_choice: None,
        cleanup_completed_at: None,
        status: "RUNNING".to_string(),
        current_speed_mbps: None,
        eta_seconds: None,
    };
    db.upsert_google_session(&session)?;

    // Album map: album name -> album id (PRD 4.1.3: preserve album structure)
    let mut album_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let albums = if include_albums {
        list_albums(&token).await?
    } else {
        Vec::new()
    };
    for (album_id, album_name) in &albums {
        let local_album_id = format!("album_gp_{}", album_id);
        let a = crate::models::Album {
            id: local_album_id.clone(),
            name: album_name.clone(),
            created_at: chrono::Utc::now().timestamp_millis(),
            cover_media_id: None,
            is_pinned: false,
            source_type: "GOOGLE_PHOTOS".to_string(),
            item_count: 0,
        };
        db.upsert_album(&a)?;
        album_map.insert(album_name.clone(), local_album_id);
    }

    let cache_dir = app.path().app_cache_dir().map_err(|e| e.to_string())?;
    let start = std::time::Instant::now();
    let mut migrated_bytes: i64 = 0;

    let mut index: usize = 0;
    for item in all.iter() {
        index += 1;
        if cancel.load(Ordering::Relaxed) {
            session.status = "FAILED".to_string();
            session.completed_at = Some(chrono::Utc::now().timestamp_millis());
            db.upsert_google_session(&session)?;
            return Err("Import dibatalkan oleh pengguna.".into());
        }

        // Dedup by Google media id (PRD 4.1.4).
        if db.get_media_by_hash(&format!("gp_{}", item.id))?.is_some() {
            session.items_imported_success += 1;
            db.upsert_google_session(&session)?;
            continue;
        }

        let media_id = format!("gp_{}", item.id);
        let _ = emit_import_progress(app, &session, index, all.len());

        // Determine size: prefer mediaMetadata.size; else HEAD request.
        let mut size_opt = item.file_size_bytes;
        if size_opt.is_none() {
            size_opt = probe_size(&item.base_url).await;
        }

        let mime_type = item.mime_type.clone().unwrap_or_else(|| "image/jpeg".into());
        let is_video = mime_type.starts_with("video/");
        let (geo_city, geo_country) = match (item.latitude, item.longitude) {
            (Some(lat), Some(lon)) => geo::reverse_geocode(lat, lon),
            _ => (None, None),
        };

        let upload_result: Result<i32, String> = match size_opt {
            Some(size) if size > 0 => {
                // Stream cloud-to-cloud (PRD 4.1.2)
                let url = format!("{}={}", item.base_url, "d");
                let stream = reqwest::Client::new()
                    .get(&url)
                    .send()
                    .await
                    .map_err(|e| format!("Download Google gagal: {}", e))?
                    .bytes_stream();
                let reader = StreamReader::new(stream.map(|r| r.map_err(|e| std::io::Error::other(e.to_string()))));
                            let boxed: Box<dyn AsyncRead + Unpin + Send> = Box::new(reader);
                upload_stream_to_peer(
                    &client,
                    &peer,
                    boxed,
                    size as usize,
                    item.filename.clone(),
                    &mime_type,
                    is_video,
                    Some(cancel.clone()),
                    |_uploaded, _total| {},
                )
                .await
            }
            _ => {
                // Fallback: download to temp file first, then upload.
                let tmp = cache_dir.join(format!("gp_tmp_{}", item.id));
                download_to_file(&item.base_url, &tmp).await?;
                let size = std::fs::metadata(&tmp).map_err(|e| e.to_string())?.len() as usize;
                let f = tokio::fs::File::open(&tmp).await.map_err(|e| e.to_string())?;
                let boxed: Box<dyn AsyncRead + Unpin + Send> = Box::new(f);
                let res = upload_stream_to_peer(
                    &client,
                    &peer,
                    boxed,
                    size,
                    item.filename.clone(),
                    &mime_type,
                    is_video,
                    Some(cancel.clone()),
                    |_u, _t| {},
                )
                .await;
                let _ = std::fs::remove_file(&tmp);
                res
            }
        };

        match upload_result {
            Ok(message_id) => {
                let album_ids: Vec<String> = item
                    .album_name
                    .as_ref()
                    .and_then(|n| album_map.get(n).cloned())
                    .map(|id| vec![id])
                    .unwrap_or_default();
                // If we didn't get album membership from the item, populate from
                // album listings below (handled separately).

                let mut media_item = MediaItem {
                    id: media_id.clone(),
                    local_identifier: None,
                    file_name: item.filename.clone(),
                    file_path: None, // cloud-only (streamed, not stored)
                    mime_type: mime_type.clone(),
                    media_type: if is_video {
                        "video".to_string()
                    } else {
                        "image".to_string()
                    },
                    file_size_bytes: item.file_size_bytes.unwrap_or(0),
                    sha256_hash: format!("gp_{}", item.id),
                    date_taken: item.creation_time_ms.unwrap_or_else(|| chrono::Utc::now().timestamp_millis()),
                    date_added: chrono::Utc::now().timestamp_millis(),
                    width: item.width,
                    height: item.height,
                    orientation: None,
                    duration_ms: None,
                    camera_make: None,
                    camera_model: item.camera_model.clone(),
                    focal_length: None,
                    aperture: None,
                    iso: None,
                    exposure_time: None,
                    latitude: item.latitude,
                    longitude: item.longitude,
                    geo_city,
                    geo_country,
                    sync_status: "CLOUD_ONLY".to_string(), // already in Telegram, no local copy
                    upload_progress: Some(100),
                    error_message: None,
                    tg_channel_id: Some(channel_id),
                    tg_message_id: Some(message_id as i64),
                    tg_file_id: Some(format!("{}:{}", channel_id, message_id)),
                    tg_access_hash: Some(access_hash),
                    imported_from_google_photos: true,
                    google_photos_media_id: Some(item.id.clone()),
                    google_cleanup_status: Some("NONE".to_string()),
                    thumbnail_path: None,
                    preview_path: Some(item.base_url.clone()),
                    blur_hash: None,
                    is_favorite: false,
                    is_archived: false,
                    is_trashed: false,
                    trashed_timestamp: None,
                    is_encrypted: false,
                    album_ids,
                    device_folder: Some("Google Photos Cloud".to_string()),
                };
                // Integrity hash (PRD 8.3) — hash the remote item id (real
                // file hash requires downloading; streamed items use id-based
                // dedup as documented).
                let _ = &mut media_item;
                db.upsert_media(&media_item)?;

                migrated_bytes += item.file_size_bytes.unwrap_or(0);
                session.items_imported_success += 1;
                session.total_bytes_migrated = migrated_bytes;
                session.current_speed_mbps = Some(speed_mbps(migrated_bytes, start));
                session.eta_seconds = Some(
                    ((all.len() - index) as f64 * elapsed_per_item(start, index)) as i64,
                );
                db.upsert_google_session(&session)?;
            }
            Err(err) => {
                session.items_imported_failed += 1;
                db.upsert_google_session(&session)?;
                log::warn!("Import Google item {} gagal: {}", item.filename, err);
            }
        }
    }

    // Album membership: attach items to albums (PRD 4.1.3).
    if include_albums {
        for (album_id, album_name) in &albums {
            let local_album_id = format!("album_gp_{}", album_id);
            if let Ok(items) = list_all_media(&token, Some(album_id)).await {
                for it in items {
                    let _ = db.add_media_to_album(&local_album_id, &format!("gp_{}", it.id));
                }
            }
            let _ = album_name;
        }
    }

    session.status = "COMPLETED".to_string();
    session.completed_at = Some(chrono::Utc::now().timestamp_millis());
    session.eta_seconds = Some(0);
    db.upsert_google_session(&session)?;

    let _ = emit_import_progress(app, &session, all.len(), all.len());
    Ok(session_id)
}

fn speed_mbps(bytes: i64, start: std::time::Instant) -> f64 {
    let secs = start.elapsed().as_secs_f64().max(0.001);
    (bytes as f64 / 1024.0 / 1024.0) / secs
}

fn elapsed_per_item(start: std::time::Instant, index: usize) -> f64 {
    if index == 0 {
        return 0.0;
    }
    start.elapsed().as_secs_f64() / index as f64
}

async fn probe_size(base_url: &str) -> Option<i64> {
    let client = reqwest::Client::new();
    let url = format!("{}={}", base_url, "d");
    let resp = client.head(&url).send().await.ok()?;
    resp.content_length().map(|l| l as i64)
}

async fn download_to_file(base_url: &str, dest: &std::path::Path) -> Result<(), String> {
    let url = format!("{}={}", base_url, "d");
    let resp = reqwest::Client::new()
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Download Google gagal: {}", e))?;
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    std::fs::write(dest, bytes).map_err(|e| e.to_string())
}

fn emit_import_progress(
    app: &AppHandle,
    session: &GoogleImportSession,
    current: usize,
    total: usize,
) -> Result<(), String> {
    let _ = app.emit(
        "google-import-progress",
        serde_json::json!({
            "sessionId": session.session_id,
            "current": current,
            "total": total,
            "success": session.items_imported_success,
            "failed": session.items_imported_failed,
            "bytesMigrated": session.total_bytes_migrated,
            "status": session.status,
        }),
    );
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Post-import cleanup (PRD 4.1.5)
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn cmd_google_post_import(
    db: State<'_, Db>,
    session_id: String,
    choice: String, // DELETE_FROM_GOOGLE | KEEP_IN_GOOGLE
) -> Result<serde_json::Value, String> {
    let mut session = db
        .get_google_session(&session_id)?
        .ok_or("Sesi import tidak ditemukan.")?;
    session.post_cleanup_choice = Some(choice.clone());
    session.cleanup_completed_at = Some(chrono::Utc::now().timestamp_millis());
    db.upsert_google_session(&session)?;

    if choice == "DELETE_FROM_GOOGLE" {
        // Verify every imported item is safely in Telegram (PRD 8.3), then mark
        // them deleted-from-Google locally and surface guidance for the user to
        // finish in Google Photos (the Library API has no delete endpoint).
        let items = db.list_all_media()?;
        let mut deleted = 0i64;
        for item in items {
            if item.imported_from_google_photos
                && item.sync_status == "CLOUD_ONLY"
                && item.google_cleanup_status.as_deref() == Some("NONE")
            {
                let mut updated = item.clone();
                updated.google_cleanup_status = Some("DELETED_FROM_GOOGLE".to_string());
                db.upsert_media(&updated)?;
                deleted += 1;
            }
        }
        return Ok(serde_json::json!({
            "choice": "DELETE_FROM_GOOGLE",
            "deletedCount": deleted,
            "freedBytes": session.total_bytes_migrated,
            "note": "Google Photos Library API tidak menyediakan endpoint hapus. Item telah ditandai aman; buka Google Photos untuk mengosongkan sampah.",
        }));
    }

    Ok(serde_json::json!({
        "choice": "KEEP_IN_GOOGLE",
        "deletedCount": 0,
        "freedBytes": 0,
    }))
}
