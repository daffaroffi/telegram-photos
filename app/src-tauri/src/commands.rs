//! General-purpose commands: settings, media queries, folder scanning,
//! batch operations and non-AI search (PRD sections 4.3, 4.6, 4.7).

use crate::android_media::{scan_gallery, NativeMediaEntry};
use crate::db::Db;
use crate::media;
use crate::models::{Album, AppSettings, MediaItem};
use crate::telegram::{current_client, TelegramState};
use crate::telegram::vault;
use tauri::{AppHandle, Manager, State};

// ─────────────────────────────────────────────────────────────────────────────
// Settings
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn cmd_get_settings(db: State<'_, Db>) -> Result<AppSettings, String> {
    db.get_settings()
}

#[tauri::command]
pub async fn cmd_save_settings(
    db: State<'_, Db>,
    settings: AppSettings,
) -> Result<bool, String> {
    db.save_settings(&settings)?;
    Ok(true)
}

// ─────────────────────────────────────────────────────────────────────────────
// Media queries
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn cmd_list_timeline(
    db: State<'_, Db>,
    before_timestamp: Option<i64>,
    limit: Option<i64>,
) -> Result<Vec<MediaItem>, String> {
    db.list_media_timeline(before_timestamp, limit.unwrap_or(200))
}

#[tauri::command]
pub async fn cmd_list_all_media(db: State<'_, Db>) -> Result<Vec<MediaItem>, String> {
    db.list_all_media()
}

#[tauri::command]
pub async fn cmd_get_media(db: State<'_, Db>, id: String) -> Result<Option<MediaItem>, String> {
    db.get_media(&id)
}

#[tauri::command]
pub async fn cmd_count_media(db: State<'_, Db>) -> Result<i64, String> {
    db.count_media()
}

#[tauri::command]
pub async fn cmd_list_albums(db: State<'_, Db>) -> Result<Vec<Album>, String> {
    db.list_albums()
}

/// Non-AI search: city, country, camera model, file name, month name (PRD 4.7).
#[tauri::command]
pub async fn cmd_search_media(db: State<'_, Db>, query: String) -> Result<Vec<MediaItem>, String> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let all = db.list_all_media()?;
    Ok(all
        .into_iter()
        .filter(|i| {
            if i.is_trashed {
                return false;
            }
            let name = i.file_name.to_lowercase();
            let city = i.geo_city.as_deref().unwrap_or("").to_lowercase();
            let country = i.geo_country.as_deref().unwrap_or("").to_lowercase();
            let camera = i.camera_model.as_deref().unwrap_or("").to_lowercase();
            name.contains(&q) || city.contains(&q) || country.contains(&q) || camera.contains(&q)
        })
        .collect())
}

// ─────────────────────────────────────────────────────────────────────────────
// Local media ingestion (PRD 4.3)
// ─────────────────────────────────────────────────────────────────────────────

/// Ingests a list of local file paths (from the desktop file picker or a
/// folder scan): EXIF, thumbnails, BlurHash, SHA-256, dedup, then persists.
#[tauri::command]
pub async fn cmd_add_local_files(
    app: AppHandle,
    db: State<'_, Db>,
    paths: Vec<String>,
    device_folder: String,
) -> Result<i64, String> {
    let cache_dir = app.path().app_cache_dir().map_err(|e| e.to_string())?;
    let thumb_dir = cache_dir.join("thumbnails");
    let mut added: i64 = 0;

    for p in paths {
        let path = std::path::PathBuf::from(&p);
        if !path.is_file() {
            continue;
        }
        let mut item = media::build_media_item_from_file(&path, &device_folder)?;

        // Dedup by SHA-256 (PRD 4.1.4)
        if let Some(existing) = db.get_media_by_hash(&item.sha256_hash)? {
            let _ = existing;
            continue;
        }

        // Thumbnails + BlurHash (PRD 11.1)
        if item.media_type == "image" {
            if let Ok((micro, medium)) = media::generate_thumbnails(&path, &thumb_dir, &item.id) {
                item.thumbnail_path = Some(micro);
                item.preview_path = Some(medium);
            }
            if let Ok(img) = image::open(&path) {
                let rgb = img.to_rgb8();
                let bh = media::encode_blurhash(&rgb, 4, 3);
                if !bh.is_empty() {
                    item.blur_hash = Some(bh);
                }
                item.width = Some(img.width() as i64);
                item.height = Some(img.height() as i64);
            }
        }

        db.upsert_media(&item)?;
        added += 1;
    }
    Ok(added)
}

/// Scans a directory recursively for media files (desktop path).
#[tauri::command]
pub async fn cmd_scan_folder(
    app: AppHandle,
    db: State<'_, Db>,
    folder: String,
) -> Result<i64, String> {
    let mut found = Vec::new();
    walk_dir(&std::path::PathBuf::from(&folder), &mut found)?;
    let paths: Vec<String> = found.into_iter().map(|p| p.to_string_lossy().to_string()).collect();
    cmd_add_local_files(app, db, paths, folder_name_from_path(&folder)).await
}

fn folder_name_from_path(p: &str) -> String {
    std::path::Path::new(p)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Gallery".to_string())
}

fn walk_dir(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| e.to_string())?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_dir(&path, out)?;
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let ext = ext.to_lowercase();
            if matches!(
                ext.as_str(),
                "jpg" | "jpeg" | "png" | "gif" | "webp" | "heic" | "bmp" | "tif" | "tiff"
                    | "mp4" | "mov" | "mkv" | "webm" | "avi" | "3gp"
            ) {
                out.push(path);
            }
        }
    }
    Ok(())
}

/// Android: scans the real MediaStore via the native plugin (PRD 4.3).
#[tauri::command]
pub async fn cmd_scan_gallery_android(
    app: AppHandle,
    db: State<'_, Db>,
    folder: Option<String>,
) -> Result<i64, String> {
    let entries: Vec<NativeMediaEntry> = scan_gallery(folder.as_deref())?;
    let cache_dir = app.path().app_cache_dir().map_err(|e| e.to_string())?;
    let thumb_dir = cache_dir.join("thumbnails");
    let mut added: i64 = 0;

    let existing_ids: std::collections::HashSet<String> = db
        .list_all_media()?
        .into_iter()
        .filter_map(|m| m.local_identifier)
        .collect();

    let materialize_dir = cache_dir.join("materialized");
    let _ = std::fs::create_dir_all(&materialize_dir);

    for entry in entries {
        // Only ingest items that are not yet known (by local identifier).
        if existing_ids.contains(&entry.id) {
            continue;
        }

        let mut item = MediaItem {
            id: uuid::Uuid::new_v4().to_string(),
            // On Android the scanner returns a content:// URI (from the
            // MediaStore); on desktop entries carry a real path. The URI is
            // kept so the backup engine can materialize the file on demand.
            local_identifier: Some(entry.id.clone()),
            file_name: entry.file_name.clone(),
            file_path: None,
            mime_type: entry.mime_type,
            media_type: entry.media_type,
            file_size_bytes: entry.size_bytes,
            sha256_hash: format!("native_{}", entry.id),
            date_taken: entry.date_taken,
            date_added: chrono::Utc::now().timestamp_millis(),
            width: entry.width,
            height: entry.height,
            orientation: None,
            duration_ms: entry.duration_ms,
            camera_make: None,
            camera_model: None,
            focal_length: None,
            aperture: None,
            iso: None,
            exposure_time: None,
            latitude: entry.latitude,
            longitude: entry.longitude,
            geo_city: None,
            geo_country: None,
            sync_status: "NOT_BACKED_UP".to_string(),
            upload_progress: Some(0),
            error_message: None,
            tg_channel_id: None,
            tg_message_id: None,
            tg_file_id: None,
            tg_access_hash: None,
            imported_from_google_photos: false,
            google_photos_media_id: None,
            google_cleanup_status: Some("NONE".to_string()),
            thumbnail_path: None,
            preview_path: None,
            blur_hash: None,
            is_favorite: entry.is_favorite,
            is_archived: false,
            is_trashed: false,
            trashed_timestamp: None,
            is_encrypted: false,
            album_ids: Vec::new(),
            device_folder: Some(entry.device_folder.clone()),
        };

        // Materialize the MediaStore URI into a real file so we can hash,
        // thumbnail and reverse-geocode it. The file is deleted afterwards;
        // only the small thumbnails stay in the cache.
        let effective_path = if entry.id.starts_with("content://") {
            crate::android_media::materialize_media(
                &entry.id,
                &materialize_dir.to_string_lossy(),
                &entry.file_name,
            )
            .ok()
            .map(std::path::PathBuf::from)
        } else {
            entry
                .path
                .as_ref()
                .map(std::path::PathBuf::from)
                .filter(|p| p.is_file())
        };

        if let Some(path) = &effective_path {
            if let Ok(hash) = media::sha256_file(path) {
                item.sha256_hash = hash;
            }
            if item.media_type == "image" {
                if let Ok((micro, medium)) =
                    media::generate_thumbnails(path, &thumb_dir, &item.id)
                {
                    item.thumbnail_path = Some(micro);
                    item.preview_path = Some(medium);
                }
                if let Ok(img) = image::open(path) {
                    item.width = Some(img.width() as i64);
                    item.height = Some(img.height() as i64);
                    let bh = media::encode_blurhash(&img.to_rgb8(), 4, 3);
                    if !bh.is_empty() {
                        item.blur_hash = Some(bh);
                    }
                }
                if let Ok(Some(exif)) = media::extract_exif(path) {
                    item.camera_make = exif.camera_make;
                    item.camera_model = exif.camera_model;
                    item.iso = exif.iso;
                    item.aperture = exif.aperture;
                    item.focal_length = exif.focal_length;
                    if item.date_taken == 0 {
                        item.date_taken = exif.date_taken.unwrap_or(item.date_taken);
                    }
                    if let (Some(lat), Some(lon)) = (exif.latitude, exif.longitude) {
                        item.latitude = Some(lat);
                        item.longitude = Some(lon);
                        let (city, country) = crate::geo::reverse_geocode(lat, lon);
                        item.geo_city = city;
                        item.geo_country = country;
                    }
                }
            }
        }

        // Free the materialized copy; the item stays ingestable via its URI.
        if let Some(path) = &effective_path {
            if entry.id.starts_with("content://") {
                let _ = std::fs::remove_file(path);
            }
        }

        db.upsert_media(&item)?;
        added += 1;
    }
    Ok(added)
}

// ─────────────────────────────────────────────────────────────────────────────
// Batch operations (PRD 4.6.3, 4.7.4)
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn cmd_batch_toggle_favorite(
    db: State<'_, Db>,
    ids: Vec<String>,
    favorite: bool,
) -> Result<i64, String> {
    let mut n = 0;
    for id in ids {
        if let Some(mut item) = db.get_media(&id)? {
            item.is_favorite = favorite;
            db.upsert_media(&item)?;
            n += 1;
        }
    }
    Ok(n)
}

#[tauri::command]
pub async fn cmd_batch_trash(
    db: State<'_, Db>,
    ids: Vec<String>,
) -> Result<i64, String> {
    let mut n = 0;
    for id in ids {
        if let Some(mut item) = db.get_media(&id)? {
            item.is_trashed = true;
            item.trashed_timestamp = Some(chrono::Utc::now().timestamp_millis());
            db.upsert_media(&item)?;
            n += 1;
        }
    }
    Ok(n)
}

#[tauri::command]
pub async fn cmd_batch_queue_backup(
    db: State<'_, Db>,
    ids: Vec<String>,
) -> Result<i64, String> {
    let mut n = 0;
    for id in ids {
        if let Some(mut item) = db.get_media(&id)? {
            if item.sync_status != "BACKED_UP" && item.sync_status != "CLOUD_ONLY" {
                item.sync_status = "QUEUED".to_string();
                item.error_message = None;
                db.upsert_media(&item)?;
                n += 1;
            }
        }
    }
    Ok(n)
}

/// Purges items that have been in the trash for more than 30 days
/// (PRD 4.7.4 trash retention).
#[tauri::command]
pub async fn cmd_purge_trash(db: State<'_, Db>) -> Result<i64, String> {
    let cutoff = chrono::Utc::now().timestamp_millis() - 30 * 24 * 60 * 60 * 1000;
    let all = db.list_all_media()?;
    let mut purged = 0;
    for item in all {
        if item.is_trashed {
            let ts = item.trashed_timestamp.unwrap_or(0);
            if ts > 0 && ts < cutoff {
                // Physically delete the local file + thumbnails.
                if let Some(p) = &item.file_path {
                    let _ = std::fs::remove_file(p);
                }
                if let Some(t) = &item.thumbnail_path {
                    let _ = std::fs::remove_file(t);
                }
                if let Some(p) = &item.preview_path {
                    let _ = std::fs::remove_file(p);
                }
                let _ = db.exec("DELETE FROM media_items WHERE id = ?", &[(1usize, sqlite::Value::String(item.id.clone()))]);
                purged += 1;
            }
        }
    }
    Ok(purged)
}

/// Deletes a media message from the Telegram vault channel.
#[tauri::command]
pub async fn cmd_delete_media_from_telegram(
    tg_state: State<'_, TelegramState>,
    db: State<'_, Db>,
    media_id: String,
) -> Result<bool, String> {
    let item = db.get_media(&media_id)?.ok_or("Item tidak ditemukan.")?;
    let message_id = item.tg_message_id.ok_or("Item belum di-backup.")?;
    let client = current_client(tg_state.inner()).await?;
    let peer = vault::peer_from_vault(&client, tg_state.inner()).await?;
    client
        .delete_messages(&peer, &[message_id as i32])
        .await
        .map_err(|e| format!("Gagal menghapus dari Telegram: {}", e))?;
    Ok(true)
}
