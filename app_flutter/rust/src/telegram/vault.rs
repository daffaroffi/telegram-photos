//! Private storage channel ("vault") management (PRD section 4.2).

use crate::telegram::TelegramState;
use grammers_client::types::{Channel, Peer};
use grammers_client::Client;
use grammers_tl_types as tl;
use telegram_photos_core::db::Db;

pub const VAULT_TITLE: &str = "TelegramPhotos_Vault";

pub async fn get_or_create_vault(
    client: &Client,
    db: &Db,
    state: &TelegramState,
) -> Result<(i64, i64, String), String> {
    if let Some(meta) = db.get_vault_meta().map_err(|e| e.to_string())? {
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

    if let Some((peer, channel_id, access_hash)) = find_existing_vault(client).await? {
        *state.vault_peer.lock().unwrap() = Some(peer);
        persist_vault(db, channel_id, access_hash)?;
        return Ok((channel_id, access_hash, VAULT_TITLE.to_string()));
    }

    let result = client
        .invoke(&tl::functions::channels::CreateChannel {
            broadcast: true,
            megagroup: false,
            title: VAULT_TITLE.to_string(),
            about: "Telegram Photos storage vault".to_string(),
            geo_point: None,
            address: None,
            for_import: false,
            forum: false,
            ttl_period: None,
        })
        .await
        .map_err(|e| format!("Gagal membuat channel vault: {}", e))?;

    let (channel_id, access_hash, peer) = match result {
        tl::enums::Updates::Updates(u) => {
            let chat = u
                .chats
                .first()
                .ok_or("No channel in response.")?;
            match chat {
                tl::enums::Chat::Channel(c) => (
                    c.id,
                    c.access_hash.unwrap_or(0),
                    Some(Peer::Channel(Channel { raw: c.clone() })),
                ),
                _ => return Err("Bukan channel.".into()),
            }
        }
        _ => return Err("Unrecognized response.".into()),
    };

    if let Some(peer) = peer {
        *state.vault_peer.lock().unwrap() = Some(peer);
    }
    persist_vault(db, channel_id, access_hash)?;
    Ok((channel_id, access_hash, VAULT_TITLE.to_string()))
}

pub async fn peer_from_vault(
    client: &Client,
    state: &TelegramState,
) -> Result<Peer, String> {
    if let Some(peer) = state.vault_peer.lock().unwrap().clone() {
        return Ok(peer);
    }
    if let Some((peer, _, _)) = find_existing_vault(client).await? {
        *state.vault_peer.lock().unwrap() = Some(peer.clone());
        return Ok(peer);
    }
    Err("Vault not found. Run backup once.".into())
}

async fn find_existing_vault(
    client: &Client,
) -> Result<Option<(Peer, i64, i64)>, String> {
    let mut dialogs = client.iter_dialogs();
    while let Some(dialog) = dialogs.next().await.map_err(|e| e.to_string())? {
        if let Peer::Channel(c) = &dialog.peer {
            if c.raw.title.contains("TelegramPhotos_Vault") {
                return Ok(Some((
                    dialog.peer.clone(),
                    c.raw.id,
                    c.raw.access_hash.unwrap_or(0),
                )));
            }
        }
    }
    Ok(None)
}

fn persist_vault(db: &Db, channel_id: i64, access_hash: i64) -> Result<(), String> {
    let meta = serde_json::json!({
        "channel_id": channel_id,
        "access_hash": access_hash,
        "channel_title": VAULT_TITLE,
    });
    db.set_vault_meta(&meta.to_string()).map_err(|e| e.to_string())
}
