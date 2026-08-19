//! flutter_rust_bridge bridge over `telegram_photos_core`.
//!
//! The Rust core is a separate crate (no Tauri). `init_core` opens the SQLite
//! database once; all other functions are thin read/write calls on it.

use flutter_rust_bridge::frb;
use std::path::Path;
use std::sync::OnceLock;
use telegram_photos_core::db::Db;
use telegram_photos_core::models::{
    Album, AppSettings, Collection, MediaItem, Upload, UploadsSummary, VaultInfo,
};

static DB: OnceLock<Db> = OnceLock::new();

/// Opens (and migrates) the local database. Call once at app startup with the
/// path from `path_provider` (e.g. `getApplicationSupportDirectory`).
#[frb(sync)]
pub fn init_core(db_path: String) -> Result<(), String> {
    if let Some(parent) = Path::new(&db_path).parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let db = Db::open(Path::new(&db_path))?;
    DB.set(db).map_err(|_| "core already initialized".to_string())
}

pub(crate) fn db() -> Result<&'static Db, String> {
    DB.get().ok_or_else(|| "core not initialized — call init_core first".to_string())
}

// ── Overview / timeline ────────────────────────────────────────────────────

/// Total non-trashed items (grid badge & boot summary).
#[frb(sync)]
pub fn count_media() -> Result<i64, String> {
    db()?.count_media()
}

/// Ingests the JSON array produced by the native MediaStore scanner
/// (MethodChannel `scanMediaStore`) and upserts every entry.
/// Returns the number of items written.
#[frb(sync)]
pub fn import_scan_results(json: String) -> Result<i64, String> {
    let trimmed = json.trim();
    if trimmed.is_empty() || trimmed == "[]" {
        return Ok(0);
    }
    let items: Vec<telegram_photos_core::models::MediaItem> = serde_json::from_str(trimmed)
        .map_err(|e| {
            let sample: String = trimmed.chars().take(300).collect();
            format!("invalid scan JSON: {e} — sample: {sample}")
        })?;
    let db = db()?;
    let mut count = 0i64;
    for item in items {
        db.upsert_media(&item)?;
        count += 1;
    }
    Ok(count)
}

/// Keyset-paginated timeline (PRD Part 1 §11.3): pass `before_timestamp` from
/// the last item of the previous page to page forward without OFFSET.
#[frb(sync)]
pub fn list_timeline(before_timestamp: Option<i64>, limit: i64) -> Result<Vec<MediaItem>, String> {
    db()?.list_media_timeline(before_timestamp, limit)
}

pub fn get_media(id: String) -> Result<Option<MediaItem>, String> {
    db()?.get_media(&id)
}

// ── Settings ───────────────────────────────────────────────────────────────

#[frb(sync)]
pub fn get_settings() -> Result<AppSettings, String> {
    db()?.get_settings()
}

#[frb(sync)]
pub fn save_settings(settings: AppSettings) -> Result<(), String> {
    db()?.save_settings(&settings)
}

// ── Vault status ───────────────────────────────────────────────────────────

#[frb(sync)]
pub fn get_vault_info() -> Result<VaultInfo, String> {
    db()?.get_vault_info()
}

// ── Uploads (PRD Part 2 §6.2) ──────────────────────────────────────────────

#[frb(sync)]
pub fn list_uploads_by_status(status: String) -> Result<Vec<Upload>, String> {
    db()?.list_uploads_by_status(&status)
}

/// Retry a failed/paused upload (PRD Part 2 §6.2: resets state machine).
#[frb(sync)]
pub fn retry_upload(upload_id: String) -> Result<(), String> {
    db()?.retry_upload(&upload_id)
}

/// Backup banner aggregate (G4): one indexed GROUP BY.
#[frb(sync)]
pub fn uploads_summary() -> Result<UploadsSummary, String> {
    db()?.uploads_summary()
}

// ── Thumbnails (G1) ────────────────────────────────────────────────────────

/// Media ids that still need a thumbnail (thumb_status != CACHED).
#[frb(sync)]
pub fn list_media_without_thumb(limit: i64) -> Result<Vec<String>, String> {
    db()?.list_media_without_thumb(limit)
}

/// Records a generated thumbnail path (JSON map mediaId -> absolute path).
#[frb(sync)]
pub fn save_thumbnail_paths(json: String) -> Result<i64, String> {
    let map: std::collections::HashMap<String, String> = serde_json::from_str(&json)
        .map_err(|e| format!("invalid thumbnail map: {e}"))?;
    let db = db()?;
    let mut count = 0i64;
    for (media_id, path) in map {
        db.set_thumbnail_path(&media_id, &path)?;
        count += 1;
    }
    Ok(count)
}

// ── Captions & hashtags (PRD Part 2 §6.3) ──────────────────────────────────

#[frb(sync)]
pub fn get_caption(media_id: String) -> Result<Option<String>, String> {
    db()?.get_caption(&media_id)
}

#[frb(sync)]
pub fn save_caption(media_id: String, text: String) -> Result<(), String> {
    db()?.upsert_caption(&media_id, &text)
}

#[frb(sync)]
pub fn add_caption_tag(media_id: String, tag: String) -> Result<(), String> {
    db()?.add_caption_tag(&media_id, &tag)
}

#[frb(sync)]
pub fn search_by_hashtag(tag: String) -> Result<Vec<String>, String> {
    db()?.search_by_hashtag(&tag)
}

// ── Collections (PRD Part 2 §6.4) ──────────────────────────────────────────

#[frb(sync)]
pub fn create_collection(name: String) -> Result<Collection, String> {
    db()?.create_collection(&name)
}

#[frb(sync)]
pub fn list_collections() -> Result<Vec<Collection>, String> {
    db()?.list_collections()
}

#[frb(sync)]
pub fn add_to_collection(collection_id: String, media_id: String) -> Result<(), String> {
    db()?.add_to_collection(&collection_id, &media_id)
}

#[frb(sync)]
pub fn remove_from_collection(collection_id: String, media_id: String) -> Result<(), String> {
    db()?.remove_from_collection(&collection_id, &media_id)
}

#[frb(sync)]
pub fn list_collection_items(collection_id: String) -> Result<Vec<MediaItem>, String> {
    db()?.list_collection_items(&collection_id)
}

// ── Albums (existing) ──────────────────────────────────────────────────────

#[frb(sync)]
pub fn list_albums() -> Result<Vec<Album>, String> {
    db()?.list_albums()
}

/// Update sync status for a media item.
/// Status values: 0=NOT_BACKED_UP, 1=BACKED_UP, 2=PARTIAL, 3=CONFLICT, 4=TRASHED
#[frb(sync)]
pub fn set_media_status(id: String, status: i32) -> Result<(), String> {
    let status_str = match status {
        0 => "NOT_BACKED_UP",
        1 => "BACKED_UP",
        2 => "PARTIAL",
        3 => "CONFLICT",
        4 => "TRASHED",
        _ => return Err(format!("Invalid status: {}", status)),
    };
    db()?.set_media_status(&id, status_str)
}
