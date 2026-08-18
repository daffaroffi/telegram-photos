//! Auto-backup engine (PRD sections 4.4 and 4.5).

use crate::telegram::upload;
use crate::telegram::vault;
use crate::telegram::{current_client, TelegramState};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use telegram_photos_core::crypto::{self, VaultState};
use telegram_photos_core::db::Db;
use telegram_photos_core::models::BackupProgressEvent;

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

pub struct BackupContext<'a> {
    pub db: &'a Db,
    pub tg_state: &'a TelegramState,
    pub vault_state: &'a VaultState,
    pub cache_dir: PathBuf,
    pub cancel: &'a Arc<AtomicBool>,
    pub on_event: &'a (dyn Fn(&BackupProgressEvent) + Send + Sync),
    pub materialize: &'a dyn Fn(&str, &str, &str) -> Result<String, String>,
    pub constraints_ok: &'a dyn Fn(bool, bool) -> bool,
}

pub async fn run_backup(
    state: &BackupState,
    ctx: &BackupContext<'_>,
) -> Result<i64, String> {
    {
        let mut running = state.running.lock().unwrap();
        if *running {
            return Ok(0);
        }
        *running = true;
    }
    state.cancelled.store(false, Ordering::Relaxed);
    let result = run_backup_core(ctx).await;
    *state.running.lock().unwrap() = false;
    result
}

async fn run_backup_core(ctx: &BackupContext<'_>) -> Result<i64, String> {
    let settings = ctx.db.get_settings().map_err(|e| e.to_string())?;
    if !settings.auto_backup_enabled {
        return Ok(0);
    }
    if !(ctx.constraints_ok)(settings.backup_over_wifi_only, settings.backup_while_charging_only) {
        return Ok(0);
    }

    let client = current_client(ctx.tg_state).await?;
    let (channel_id, access_hash, _) =
        vault::get_or_create_vault(&client, ctx.db, ctx.tg_state).await?;
    let peer = vault::peer_from_vault(&client, ctx.tg_state).await?;

    let pending = ctx
        .db
        .list_media_by_statuses(&["NOT_BACKED_UP", "QUEUED", "FAILED"])
        .map_err(|e| e.to_string())?;
    let mut success_count: i64 = 0;

    for (idx, item) in pending.iter().enumerate() {
        if ctx.cancel.load(Ordering::Relaxed) {
            break;
        }
        if let Some(folder) = &item.device_folder {
            if settings.folder_backup_settings.get(folder) == Some(&false) {
                continue;
            }
        }

        let encrypt = settings.client_encryption_enabled;
        if encrypt && ctx.vault_state.key.lock().unwrap().is_none() {
            (ctx.on_event)(&BackupProgressEvent {
                item_id: item.id.clone(),
                file_name: item.file_name.clone(),
                percent: 0,
                status: "VAULT_LOCKED".into(),
            });
            ctx.db.set_media_status(&item.id, "QUEUED").map_err(|e| e.to_string())?;
            continue;
        }

        let mut materialized: Option<PathBuf> = None;
        let path = match &item.file_path {
            Some(p) if PathBuf::from(p).is_file() => PathBuf::from(p),
            _ => {
                let local_id = item.local_identifier.clone().unwrap_or_default();
                if local_id.starts_with("content://") {
                    let dir = ctx.cache_dir.join("materialized");
                    let _ = std::fs::create_dir_all(&dir);
                    match (ctx.materialize)(&local_id, &dir.to_string_lossy(), &item.file_name) {
                        Ok(p) if !p.is_empty() => {
                            let pb = PathBuf::from(p);
                            materialized = Some(pb.clone());
                            pb
                        }
                        _ => {
                            ctx.db.set_media_status(&item.id, "FAILED").map_err(|e| e.to_string())?;
                            continue;
                        }
                    }
                } else {
                    ctx.db.set_media_status(&item.id, "CLOUD_ONLY").map_err(|e| e.to_string())?;
                    continue;
                }
            }
        };

        let upload_path: PathBuf;
        let size;
        let reader: Box<dyn tokio::io::AsyncRead + Unpin + Send>;
        if encrypt {
            let temp = ctx.cache_dir.join(format!("{}.tdenc", item.id));
            let key_bytes = ctx.vault_state.key.lock().unwrap().clone().ok_or("Vault terkunci")?;
            let key = crypto::VaultKey(key_bytes.try_into().map_err(|_| "Kunci tidak valid")?);
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

        ctx.db.set_media_status(&item.id, "UPLOADING").map_err(|e| e.to_string())?;
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
                ctx.db.upsert_media(&updated).map_err(|e| e.to_string())?;
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
                ctx.db.upsert_media(&updated).map_err(|e| e.to_string())?;
                (ctx.on_event)(&BackupProgressEvent {
                    item_id: item.id.clone(),
                    file_name: item.file_name.clone(),
                    percent: 0,
                    status: "FAILED".into(),
                });
                log::warn!("Backup gagal: {}: {}", item.file_name, err);
            }
        }

        if idx + 1 < pending.len() {
            tokio::time::sleep(std::time::Duration::from_millis(300 + rand::random::<u64>() % 201))
                .await;
        }
    }

    Ok(success_count)
}
