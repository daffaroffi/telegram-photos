//! Upload/download engine for Telegram media (PRD section 4.4, 8.2, 11.4).
//!
//! Implements the chunked part-upload loop directly on `upload.saveFilePart` /
//! `upload.saveBigFilePart` (512 KB chunks, PRD 11.4) so that progress is
//! reported and FLOOD_WAIT responses are handled with an `X + 2s` sleep before
//! resuming the same part, exactly as specified in PRD section 8.2.

use grammers_client::{Client, InputMessage};
use grammers_mtsender::InvocationError;
use grammers_tl_types as tl;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncReadExt;

pub const CHUNK_SIZE: usize = 512 * 1024; // PRD 11.4: 512 KB chunks
const BIG_FILE_THRESHOLD: usize = 10 * 1024 * 1024; // Telegram big-file threshold

/// Uploads a stream to a peer (the vault channel) and returns the message id.
///
/// `on_progress` receives `(bytes_uploaded, total_bytes)`.
pub async fn upload_stream_to_peer(
    client: &Client,
    peer: &grammers_client::types::Peer,
    mut reader: Box<dyn tokio::io::AsyncRead + Unpin + Send>,
    size: usize,
    name: String,
    mime_type: &str,
    is_video: bool,
    cancel: Option<Arc<AtomicBool>>,
    mut on_progress: impl FnMut(u64, usize),
) -> Result<i32, String> {
    let big_file = size > BIG_FILE_THRESHOLD;
    let file_id: i64 = rand::random::<i64>().abs();
    let total_parts = size.div_ceil(CHUNK_SIZE) as i32;
    let mut md5 = md5::Context::new();

    let mut part: i32 = 0;
    let mut uploaded_bytes: u64 = 0;
    let mut buf = vec![0u8; CHUNK_SIZE];

    loop {
        if let Some(c) = &cancel {
            if c.load(Ordering::Relaxed) {
                return Err("Transfer dibatalkan".into());
            }
        }

        // Read one full 512 KB chunk, handling partial reads.
        let mut filled = 0usize;
        while filled < CHUNK_SIZE {
            match reader.read(&mut buf[filled..]).await {
                Ok(0) => break,
                Ok(n) => filled += n,
                Err(e) => return Err(format!("Gagal membaca file: {}", e)),
            }
        }
        if filled == 0 {
            break; // EOF (empty file)
        }

        let bytes = buf[..filled].to_vec();
        md5.consume(&bytes);

        // Send part with FloodWait resilience (PRD 8.2: sleep X + 2s, resume).
        loop {
            let res = if big_file {
                client
                    .invoke(&tl::functions::upload::SaveBigFilePart {
                        file_id,
                        file_part: part,
                        file_total_parts: total_parts,
                        bytes: bytes.clone(),
                    })
                    .await
            } else {
                client
                    .invoke(&tl::functions::upload::SaveFilePart {
                        file_id,
                        file_part: part,
                        bytes: bytes.clone(),
                    })
                    .await
            };

            match res {
                Ok(true) => break,
                Ok(false) => {
                    return Err("Server Telegram gagal menyimpan data part.".into())
                }
                Err(InvocationError::Rpc(err)) if err.is("FLOOD_WAIT_*") => {
                    let secs = flood_wait_seconds(&err.name);
                    log::warn!(
                        "FLOOD_WAIT_{} saat upload, menunggu {} detik lalu melanjutkan",
                        secs,
                        secs + 2
                    );
                    tokio::time::sleep(Duration::from_secs(secs + 2)).await;
                }
                Err(e) => {
                    return Err(format!("Upload gagal pada part {}: {}", part, e))
                }
            }
        }

        part += 1;
        uploaded_bytes += filled as u64;
        on_progress(uploaded_bytes, size);

        if filled < CHUNK_SIZE {
            break; // last (partial) chunk
        }
    }

    // Build the input file reference.
    let input_file: tl::enums::InputFile = if big_file {
        tl::types::InputFileBig {
            id: file_id,
            parts: total_parts,
            name: name.clone(),
        }
        .into()
    } else {
        tl::types::InputFile {
            id: file_id,
            parts: total_parts,
            name: name.clone(),
            md5_checksum: format!("{:x}", md5.compute()),
        }
        .into()
    };

    // Attributes: file name (+ video attribute when applicable).
    let mut attributes: Vec<tl::enums::DocumentAttribute> = vec![
        tl::enums::DocumentAttribute::Filename(tl::types::DocumentAttributeFilename {
            file_name: name.clone(),
        }),
    ];
    if is_video {
        attributes.push(tl::enums::DocumentAttribute::Video(tl::types::DocumentAttributeVideo {
            round_message: false,
            supports_streaming: true,
            nosound: false,
            duration: 0.0,
            w: 0,
            h: 0,
            preload_prefix_size: None,
            video_start_ts: None,
            video_codec: None,
        }));
    }

    let media = tl::enums::InputMedia::UploadedDocument(tl::types::InputMediaUploadedDocument {
        nosound_video: false,
        force_file: false,
        spoiler: false,
        file: input_file,
        thumb: None,
        mime_type: mime_type.to_string(),
        attributes,
        stickers: None,
        video_cover: None,
        video_timestamp: None,
        ttl_seconds: None,
    });

    let message = InputMessage::new().text("").media(media);

    // Send with FloodWait + retry.
    let mut last_err: Option<String> = None;
    for _attempt in 0..4 {
        match client.send_message(peer, message.clone()).await {
            Ok(msg) => return Ok(msg.id()),
            Err(e) => {
                let err = e.to_string();
                if let InvocationError::Rpc(rpc) = &e {
                    if rpc.is("FLOOD_WAIT_*") {
                        let secs = flood_wait_seconds(&rpc.name);
                        log::warn!(
                            "FLOOD_WAIT_{} saat kirim pesan, menunggu {}s",
                            secs,
                            secs + 2
                        );
                        tokio::time::sleep(Duration::from_secs(secs + 2)).await;
                        last_err = Some(err);
                        continue;
                    }
                }
                last_err = Some(err);
                break;
            }
        }
    }

    Err(format!(
        "Gagal mengirim media ke Telegram: {}",
        last_err.unwrap_or_default()
    ))
}

/// Downloads a media message to a local file (used by restore/Free-Up-Space flows).
pub async fn download_message_to_path(
    client: &Client,
    peer: &grammers_client::types::Peer,
    message_id: i32,
    dest_path: &std::path::Path,
) -> Result<(), String> {
    let messages = client
        .get_messages_by_id(peer, &[message_id])
        .await
        .map_err(|e| format!("Gagal mengambil pesan media: {}", e))?;
    let message = match messages.first() {
        Some(Some(m)) => m,
        _ => return Err("Pesan media tidak ditemukan di Telegram.".into()),
    };
    let media = message
        .media()
        .ok_or("Pesan tidak memiliki media.")?;
    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    client
        .download_media(&media, dest_path)
        .await
        .map_err(|e| format!("Gagal mengunduh media: {}", e))?;
    Ok(())
}

/// Parses the seconds out of a `FLOOD_WAIT_<seconds>` RPC error name.
fn flood_wait_seconds(name: &str) -> u64 {
    name.strip_prefix("FLOOD_WAIT_")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(30)
}
