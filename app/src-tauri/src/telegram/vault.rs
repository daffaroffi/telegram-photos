//! Private storage channel ("vault") management (PRD section 4.2).
//!
//! The app auto-creates a private channel named `TelegramPhotos_Vault` on the
//! user's own Telegram account and persists its `(channel_id, access_hash)` in
//! the local database so uploads can target it without scanning dialogs.

use crate::db::Db;
use crate::telegram::{current_client, TelegramState};
use grammers_client::types::Peer;
use grammers_client::Client;
use grammers_tl_types as tl;
use serde_json::json;
use tauri::State;

pub const VAULT_TITLE: &str = "TelegramPhotos_Vault";

/// Returns `(channel_id, access_hash, title)` for the vault, creating it if needed.
pub async fn get_or_create_vault(client: &Client, db: &Db) -> Result<(i64, i64, String), String> {
    // Fast path: stored channel info
    if let Some(meta) = db.get_vault_meta()? {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&meta) {
            if let (Some(channel_id), Some(access_hash)) = (
                v.get("channel_id").and_then(|x| x.as_i64()),
                v.get("access_hash").and_then(|x| x.as_i64()),
            ) {
            let title = v
                .get("channel_title")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| VAULT_TITLE.to_string());
            return Ok((channel_id, access_hash, title));
            }
        }
    }

    // Slow path: scan dialogs for an existing vault created by a previous session.
    if let Some((channel_id, access_hash)) = find_existing_vault(client).await? {
        persist_vault(db, channel_id, access_hash)?;
        return Ok((channel_id, access_hash, VAULT_TITLE.to_string()));
    }

    // Create a new private channel.
    let result = client
        .invoke(&tl::functions::channels::CreateChannel {
            broadcast: true,
            megagroup: false,
            title: VAULT_TITLE.to_string(),
            about: "Telegram Photos storage vault - private backup channel".to_string(),
            geo_point: None,
            address: None,
            for_import: false,
            forum: false,
            ttl_period: None,
        })
        .await
        .map_err(|e| format!("Gagal membuat channel vault: {}", e))?;

    let (channel_id, access_hash) = match result {
        tl::enums::Updates::Updates(u) => {
            let chat = u
                .chats
                .first()
                .ok_or("Tidak ada channel dalam respons Telegram.")?;
            match chat {
                tl::enums::Chat::Channel(c) => (c.id, c.access_hash.unwrap_or(0)),
                _ => return Err("Chat yang dibuat bukan channel.".into()),
            }
        }
        _ => return Err("Respons pembuatan channel tidak dikenali.".into()),
    };

    persist_vault(db, channel_id, access_hash)?;
    Ok((channel_id, access_hash, VAULT_TITLE.to_string()))
}

async fn find_existing_vault(
    client: &Client,
) -> Result<Option<(i64, i64)>, String> {
    let mut dialogs = client.iter_dialogs();
    while let Some(dialog) = dialogs.next().await.map_err(|e| e.to_string())? {
        if let Peer::Channel(c) = &dialog.peer {
            if c.raw.title.contains("TelegramPhotos_Vault") {
                return Ok(Some((c.raw.id, c.raw.access_hash.unwrap_or(0))));
            }
        }
    }
    Ok(None)
}

fn persist_vault(db: &Db, channel_id: i64, access_hash: i64) -> Result<(), String> {
    let meta = json!({
        "channel_id": channel_id,
        "access_hash": access_hash,
        "channel_title": VAULT_TITLE,
    });
    db.set_vault_meta(&meta.to_string())
}

#[tauri::command]
pub async fn cmd_get_or_create_vault(
    state: State<'_, TelegramState>,
    db: State<'_, Db>,
) -> Result<(i64, i64, String), String> {
    let client = current_client(&state).await?;
    get_or_create_vault(&client, db.inner()).await
}
