//! Auto-backup engine (PRD sections 4.4 and 4.5).
//!
//! Implements the backup state machine:
//! `NOT_BACKED_UP -> QUEUED -> UPLOADING -> BACKED_UP`, with `FAILED` retries,
//! `CLOUD_ONLY` after Free Up Space, Wi-Fi/charging constraints, folder
//! whitelists, a 300–500 ms pause between files (PRD 4.4), FLOOD_WAIT
//! resilience (handled in the upload layer) and optional client-side
//! encryption before upload.

use crate::crypto::{self, VaultState};
use crate::db::Db;
use crate::models::{BackupProgressEvent, FreeUpSpaceResult, ReclaimableSpace};
use crate::telegram::upload;
use crate::telegram::vault;
use crate::telegram::{current_client, TelegramState};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};

pub struct BackupState {
    pub running: std::sync::Mutex<bool>,
    pub cancelled: Arc<AtomicBool>,
}

impl Default for BackupState {
    fn default() -> Self {
        Self {
            running: std::sync::Mutex::new(false),
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[tauri::command]
pub async fn cmd_cancel_backup(state: State<'_, BackupState>) -> Result<bool, String> {
    state.cancelled.store(true, Ordering::Relaxed);
    Ok(true)
}

#[tauri::command]
pub async fn cmd_backup_status(state: State<'_, BackupState>) -> Result<bool, String> {
    Ok(*state.running.lock().unwrap())
}

/// Everything the backup core needs to run. The Tauri command supplies the
/// `AppHandle`-backed emitter; the Android background worker supplies a JNI
/// callback that updates a system notification.
pub struct BackupContext<'a> {
    pub db: &'a Db,
    pub tg_state: &'a TelegramState,
    pub vault_state: &'a VaultState,
    pub cache_dir: PathBuf,
    pub cancel: &'a Arc<AtomicBool>,
    pub on_event: &'a (dyn Fn(&BackupProgressEvent) + Send + Sync),
}

/// One full backup cycle: uploads every pending item to the vault.
#[tauri::command]
pub async fn cmd_run_backup(
    app: AppHandle,
    tg_state: State<'_, TelegramState>,
    db: State<'_, Db>,
    backup_state: State<'_, BackupState>,
    vault_state: State<'_, VaultState>,
) -> Result<i64, String> {
    {
        let mut running = backup_state.running.lock().unwrap();
        if *running {
            return Ok(0);
        }
        *running = true;
    }
    backup_state.cancelled.store(false, Ordering::Relaxed);
    let cancel = backup_state.cancelled.clone();

    let cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?;

    let on_event = |event: &BackupProgressEvent| {
        let _ = app.emit("backup-progress", event.clone());
    };
    let ctx = BackupContext {
        db: db.inner(),
        tg_state: tg_state.inner(),
        vault_state: vault_state.inner(),
        cache_dir,
        cancel: &cancel,
        on_event: &on_event,
    };
    let result = run_backup_core(&ctx).await;

    *backup_state.running.lock().unwrap() = false;
    result
}

/// The backup state machine itself, independent of Tauri so the Android
/// background worker can reuse it.
pub async fn run_backup_core(ctx: &BackupContext<'_>) -> Result<i64, String> {
    let settings = ctx.db.get_settings()?;
    if !settings.auto_backup_enabled {
        return Ok(0);
    }

    // Network / charging constraints (PRD 4.4). On desktop these always pass;
    // on Android they are enforced by the native plugin via JNI.
    if !constraints_satisfied(settings.backup_over_wifi_only, settings.backup_while_charging_only) {
        return Ok(0);
    }

    let client = current_client(ctx.tg_state).await?;
    let (channel_id, access_hash, _title) =
        vault::get_or_create_vault(&client, ctx.db, ctx.tg_state).await?;
    let peer = vault::peer_from_vault(&client, ctx.tg_state).await?;

    let pending = ctx.db.list_media_by_statuses(&["NOT_BACKED_UP", "QUEUED", "FAILED"])?;
    let mut success_count: i64 = 0;

    let cache_dir = &ctx.cache_dir;

    for (idx, item) in pending.iter().enumerate() {
        if ctx.cancel.load(Ordering::Relaxed) {
            break;
        }

        // Folder whitelist (PRD 4.4)
        if let Some(folder) = &item.device_folder {
            if settings.folder_backup_settings.get(folder) == Some(&false) {
                continue;
            }
        }

        // Client-side encryption requires an unlocked vault (PRD 4.8)
        let encrypt = settings.client_encryption_enabled;
        if encrypt && ctx.vault_state.key.lock().unwrap().is_none() {
            (ctx.on_event)(&BackupProgressEvent {
                item_id: item.id.clone(),
                file_name: item.file_name.clone(),
                percent: 0,
                status: "VAULT_LOCKED".into(),
            });
            ctx.db.set_media_status(&item.id, "QUEUED")?;
            continue;
        }

        // Resolve a real local file. Items scanned from the Android MediaStore
        // carry a content:// URI in `local_identifier` and no file path; they
        // are materialized into the cache just-in-time (PRD 4.3).
        let mut materialized: Option<PathBuf> = None;
        let path = match &item.file_path {
            Some(p) if PathBuf::from(p).is_file() => PathBuf::from(p),
            _ => {
                let local_id = item.local_identifier.clone().unwrap_or_default();
                if local_id.starts_with("content://") {
                    let dir = cache_dir.join("materialized");
                    let _ = std::fs::create_dir_all(&dir);
                    match crate::android_media::materialize_media(
                        &local_id,
                        &dir.to_string_lossy(),
                        &item.file_name,
                    ) {
                        Ok(p) if !p.is_empty() => {
                            let pb = PathBuf::from(p);
                            materialized = Some(pb.clone());
                            pb
                        }
                        _ => {
                            ctx.db.set_media_status(&item.id, "FAILED")?;
                            continue;
                        }
                    }
                } else {
                    // No local file (cloud-only) — nothing to upload.
                    ctx.db.set_media_status(&item.id, "CLOUD_ONLY")?;
                    continue;
                }
            }
        };

        // Optional pre-upload encryption to a temp file.
        let upload_path: PathBuf;
        let size;
        let reader: Box<dyn tokio::io::AsyncRead + Unpin + Send>;
        if encrypt {
            let temp = cache_dir.join(format!("{}.tdenc", item.id));
            let key_bytes = ctx.vault_state.key.lock().unwrap().clone().ok_or("Vault terkunci")?;
            let key = crypto::VaultKey(key_bytes.try_into().map_err(|_| "Kunci vault tidak valid")?);
            crypto::encrypt_file(&path, &temp, &key)?;
            upload_path = temp;
            size = std::fs::metadata(&upload_path).map_err(|e| e.to_string())?.len() as usize;
            let f = tokio::fs::File::open(&upload_path).await.map_err(|e| e.to_string())?;
            reader = Box::new(f);
        } else {
            upload_path = path;
            size = std::fs::metadata(&upload_path).map_err(|e| e.to_string())?.len() as usize;
            let f = tokio::fs::File::open(&upload_path).await.map_err(|e| e.to_string())?;
            reader = Box::new(f);
        }

        ctx.db.set_media_status(&item.id, "UPLOADING")?;
        (ctx.on_event)(&BackupProgressEvent {
            item_id: item.id.clone(),
            file_name: item.file_name.clone(),
            percent: 0,
            status: "UPLOADING".into(),
        });

        let item_id = item.id.clone();
        let file_name = item.file_name.clone();
        let name = if encrypt {
            format!("{}.tdenc", file_name)
        } else {
            file_name.clone()
        };

        let result = upload::upload_stream_to_peer(
            &client,
            &peer,
            reader,
            size,
            name,
            &item.mime_type,
            item.media_type == "video",
            Some(ctx.cancel.clone()),
            |uploaded, total| {
                let percent = if total > 0 {
                    ((uploaded as f64 / total as f64) * 100.0) as i64
                } else {
                    0
                };
                let _ = ctx.db.set_media_status(&item_id, "UPLOADING");
                (ctx.on_event)(&BackupProgressEvent {
                    item_id: item_id.clone(),
                    file_name: file_name.clone(),
                    percent: percent.min(100),
                    status: "UPLOADING".into(),
                });
            },
        )
        .await;

        // Clean up temp encryption file and any just-in-time materialized copy.
        if encrypt {
            let _ = std::fs::remove_file(&upload_path);
        }
        if let Some(m) = materialized {
            let _ = std::fs::remove_file(&m);
        }

        match result {
            Ok(message_id) => {
                let mut updated = item.clone();
                updated.sync_status = "BACKED_UP".to_string();
                updated.tg_channel_id = Some(channel_id);
                updated.tg_message_id = Some(message_id as i64);
                updated.tg_access_hash = Some(access_hash);
                updated.tg_file_id = Some(format!("{}:{}", channel_id, message_id));
                updated.upload_progress = Some(100);
                updated.error_message = None;
                ctx.db.upsert_media(&updated)?;
                (ctx.on_event)(&BackupProgressEvent {
                    item_id: item.id.clone(),
                    file_name: item.file_name.clone(),
                    percent: 100,
                    status: "BACKED_UP".into(),
                });
                success_count += 1;
            }
            Err(err) => {
                let mut updated = item.clone();
                updated.sync_status = "FAILED".to_string();
                updated.error_message = Some(err.clone());
                ctx.db.upsert_media(&updated)?;
                (ctx.on_event)(&BackupProgressEvent {
                    item_id: item.id.clone(),
                    file_name: item.file_name.clone(),
                    percent: 0,
                    status: "FAILED".into(),
                });
                log::warn!("Backup gagal untuk {}: {}", item.file_name, err);
            }
        }

        // PRD 4.4: 300–500 ms pause between files.
        if idx + 1 < pending.len() {
            tokio::time::sleep(std::time::Duration::from_millis(300 + rand::random::<u64>() % 201))
                .await;
        }
    }

    Ok(success_count)
}

/// Constraint check. On desktop the user is assumed to be on a fixed network;
/// on Android the native plugin reports real network/charging state via JNI.
fn constraints_satisfied(_wifi_only: bool, _charging_only: bool) -> bool {
    #[cfg(target_os = "android")]
    {
        let _ = (_wifi_only, _charging_only);
        crate::android_media::constraints_ok(_wifi_only, _charging_only)
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (_wifi_only, _charging_only);
        true
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Free Up Device Space (PRD section 4.5)
// ─────────────────────────────────────────────────────────────────────────────

/// Items whose local files can be safely removed (verified BACKED_UP).
pub fn calculate_free_up_space(db: &Db) -> Result<ReclaimableSpace, String> {
    let items = db.list_backed_up_media()?;
    let reclaimable: Vec<_> = items
        .iter()
        .filter(|i| {
            i.file_path.is_some()
                && PathBuf::from(i.file_path.as_ref().unwrap()).exists()
                && !i.imported_from_google_photos
        })
        .collect();
    Ok(ReclaimableSpace {
        count: reclaimable.len() as i64,
        total_size_bytes: reclaimable
            .iter()
            .map(|i| i.file_size_bytes)
            .sum::<i64>(),
    })
}

#[tauri::command]
pub async fn cmd_calculate_free_up_space(db: State<'_, Db>) -> Result<ReclaimableSpace, String> {
    calculate_free_up_space(db.inner())
}

/// Deletes verified local copies, keeps thumbnails, marks items CLOUD_ONLY.
/// Integrity is verified by re-hashing the local file against the recorded
/// SHA-256 before deletion (PRD 4.5, 8.3).
#[tauri::command]
pub async fn cmd_execute_free_up_space(db: State<'_, Db>) -> Result<FreeUpSpaceResult, String> {
    let items = db.list_backed_up_media()?;
    let mut freed_count: i64 = 0;
    let mut freed_bytes: i64 = 0;

    for item in items {
        let Some(path_str) = item.file_path.clone() else { continue };
        if item.imported_from_google_photos {
            continue; // never delete "local" copies of cloud-imported items
        }
        let path = PathBuf::from(&path_str);
        if !path.exists() {
            continue;
        }
        // Verify integrity before deleting (PRD 8.3).
        match crate::media::sha256_file(&path) {
            Ok(hash) if hash == item.sha256_hash => {
                let size = item.file_size_bytes;
                if std::fs::remove_file(&path).is_ok() {
                    db.mark_media_cloud_only(&item.id)?;
                    freed_count += 1;
                    freed_bytes += size;
                }
            }
            _ => {
                // Hash mismatch: keep the file, flag for re-upload.
                let mut updated = item.clone();
                updated.sync_status = "NOT_BACKED_UP".to_string();
                updated.error_message = Some(
                    "Hash tidak cocok dengan yang tersimpan di Telegram; antre ulang.".into(),
                );
                db.upsert_media(&updated)?;
            }
        }
    }

    Ok(FreeUpSpaceResult {
        freed_count,
        freed_bytes,
    })
}

/// Restores a cloud-only item back to a local file (downloads from the vault).
#[tauri::command]
pub async fn cmd_restore_media(
    app: AppHandle,
    tg_state: State<'_, TelegramState>,
    db: State<'_, Db>,
    media_id: String,
    dest_dir: String,
) -> Result<String, String> {
    let item = db
        .get_media(&media_id)?
        .ok_or("Item tidak ditemukan.")?;
    let message_id = item
        .tg_message_id
        .ok_or("Item belum di-backup ke Telegram.")? as i32;

    let client = current_client(tg_state.inner()).await?;
    let peer = vault::peer_from_vault(&client, tg_state.inner()).await?;

    let dest = PathBuf::from(&dest_dir).join(&item.file_name);
    upload::download_message_to_path(&client, &peer, message_id, &dest).await?;

    // Optionally decrypt if the item was uploaded encrypted.
    let settings = db.get_settings()?;
    if settings.client_encryption_enabled && item.file_name.ends_with(".tdenc") {
        let key_bytes = app
            .state::<VaultState>()
            .key
            .lock()
            .unwrap()
            .clone()
            .ok_or("Vault terkunci. Buka vault terlebih dahulu.")?;
        let key = crypto::VaultKey(key_bytes.try_into().map_err(|_| "Kunci vault tidak valid")?);
        let plain = dest.with_extension("");
        crypto::decrypt_file(&dest, &plain, &key)?;
        let _ = std::fs::remove_file(&dest);
        return Ok(plain.to_string_lossy().to_string());
    }

    Ok(dest.to_string_lossy().to_string())
}


