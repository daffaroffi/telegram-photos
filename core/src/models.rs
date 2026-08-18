//! Data models shared between the Rust backend and the React frontend.
//! All structs serialize to camelCase JSON consumed by the Tauri frontend.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────────────────
// Telegram
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TelegramUser {
    pub id: i64,
    pub first_name: String,
    pub last_name: Option<String>,
    pub username: Option<String>,
    pub phone: String,
    pub is_premium: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VaultInfo {
    pub channel_id: Option<i64>,
    pub channel_title: String,
    pub is_private: bool,
    pub total_storage_used_bytes: i64,
    pub total_backed_up_files: i64,
    pub last_sync_timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthCodeResult {
    /// code_required | password_required | authorized | qr
    pub status: String,
    pub code_length: Option<u32>,
    pub resend_after_seconds: Option<u64>,
    /// sms | call | telegram_app | missed_call | fragment
    pub delivery: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Media
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaItem {
    pub id: String,
    pub local_identifier: Option<String>,
    pub file_name: String,
    pub file_path: Option<String>,
    pub mime_type: String,
    pub media_type: String, // image | video
    pub file_size_bytes: i64,
    pub sha256_hash: String,

    pub date_taken: i64,
    pub date_added: i64,

    pub width: Option<i64>,
    pub height: Option<i64>,
    pub orientation: Option<i64>,
    pub duration_ms: Option<i64>,

    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub focal_length: Option<f64>,
    pub aperture: Option<f64>,
    pub iso: Option<i64>,
    pub exposure_time: Option<String>,

    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub geo_city: Option<String>,
    pub geo_country: Option<String>,

    // NOT_BACKED_UP | QUEUED | UPLOADING | BACKED_UP | CLOUD_ONLY | FAILED
    pub sync_status: String,
    pub upload_progress: Option<i64>,
    pub error_message: Option<String>,

    pub tg_channel_id: Option<i64>,
    pub tg_message_id: Option<i64>,
    pub tg_file_id: Option<String>,
    pub tg_access_hash: Option<i64>,

    pub imported_from_google_photos: bool,
    pub google_photos_media_id: Option<String>,
    // NONE | QUEUED_FOR_DELETE | DELETED_FROM_GOOGLE
    pub google_cleanup_status: Option<String>,

    pub thumbnail_path: Option<String>,
    pub preview_path: Option<String>,
    pub blur_hash: Option<String>,

    pub is_favorite: bool,
    pub is_archived: bool,
    pub is_trashed: bool,
    pub trashed_timestamp: Option<i64>,
    pub is_encrypted: bool,

    pub album_ids: Vec<String>,
    pub device_folder: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Album {
    pub id: String,
    pub name: String,
    pub created_at: i64,
    pub cover_media_id: Option<String>,
    pub is_pinned: bool,
    pub source_type: String, // LOCAL | GOOGLE_PHOTOS
    pub item_count: i64,
}

// ─────────────────────────────────────────────────────────────────────────────
// Google Photos migration
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleImportSession {
    pub session_id: String,
    pub google_account_email: String,
    pub started_at: i64,
    pub completed_at: Option<i64>,
    pub total_items_found: i64,
    pub items_imported_success: i64,
    pub items_imported_failed: i64,
    pub total_bytes_migrated: i64,
    pub post_cleanup_choice: Option<String>, // DELETE_FROM_GOOGLE | KEEP_IN_GOOGLE
    pub cleanup_completed_at: Option<i64>,
    // RUNNING | COMPLETED | PAUSED | FAILED
    pub status: String,
    pub current_speed_mbps: Option<f64>,
    pub eta_seconds: Option<i64>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Settings
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    // Backup constraints
    pub auto_backup_enabled: bool,
    pub backup_over_wifi_only: bool,
    pub backup_while_charging_only: bool,
    pub upload_original_quality: bool,

    // Folder whitelist: folder name -> enabled
    pub folder_backup_settings: HashMap<String, bool>,

    // Client-side encryption (Zero-Knowledge Vault)
    pub client_encryption_enabled: bool,
    pub vault_passphrase_set: bool,

    // Grid & layout
    pub grid_column_count: i64,
    pub theme: String, // system | light | dark

    // Telegram API credentials (user-provided from my.telegram.org)
    pub telegram_api_id: Option<String>,
    pub telegram_api_hash: Option<String>,

    // Google OAuth credentials (user-provided from Google Cloud Console)
    pub google_client_id: Option<String>,
    pub google_client_secret: Option<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        let mut folders = HashMap::new();
        folders.insert("Camera".to_string(), true);
        folders.insert("WhatsApp".to_string(), true);
        folders.insert("Instagram".to_string(), true);
        folders.insert("Screenshots".to_string(), false);
        folders.insert("Download".to_string(), false);

        Self {
            auto_backup_enabled: true,
            backup_over_wifi_only: true,
            backup_while_charging_only: false,
            upload_original_quality: true,
            folder_backup_settings: folders,
            client_encryption_enabled: false,
            vault_passphrase_set: false,
            grid_column_count: 3,
            theme: "system".to_string(),
            telegram_api_id: None,
            telegram_api_hash: None,
            google_client_id: None,
            google_client_secret: None,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PRD Part 2: uploads / captions / collections
// ─────────────────────────────────────────────────────────────────────────────

/// Upload state machine row (PRD Part 2 §6.2).
/// Status: PENDING | QUEUED | UPLOADING | BACKED_UP | FAILED | SKIPPED | PAUSED
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Upload {
    pub id: String,
    pub media_id: String,
    pub message_id: Option<i64>,
    pub file_id: Option<String>,
    pub hash_sha256: Option<String>,
    pub status: String,
    pub retry_count: i64,
    pub last_error: Option<String>,
    pub uploaded_bytes: i64,
    pub total_bytes: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadError {
    pub id: i64,
    pub upload_id: String,
    pub error_code: Option<String>,
    pub message: String,
    pub at: i64,
}

/// Backup-banner aggregate (G4): count + bytes per status group.
/// Flat fields (no tuples) so flutter_rust_bridge generates a plain Dart class.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UploadsSummary {
    pub queued_count: i64,
    pub queued_bytes: i64,
    pub uploading_count: i64,
    pub uploading_bytes: i64,
    pub failed_count: i64,
    pub failed_bytes: i64,
    pub backed_up_count: i64,
    pub backed_up_bytes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub cover_media_id: Option<String>,
    pub is_cloud: bool,
    pub sort_order: i64,
    pub created_at: i64,
    pub item_count: i64,
}

// ─────────────────────────────────────────────────────────────────────────────
// Events & results
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupProgressEvent {
    pub item_id: String,
    pub file_name: String,
    pub percent: i64,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FreeUpSpaceResult {
    pub freed_count: i64,
    pub freed_bytes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReclaimableSpace {
    pub count: i64,
    pub total_size_bytes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GooglePhotosItem {
    pub id: String,
    pub filename: String,
    pub mime_type: Option<String>,
    pub base_url: String,
    pub creation_time_ms: Option<i64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub file_size_bytes: Option<i64>,
    pub camera_model: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub album_name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleDiscoveryInfo {
    pub total_count: i64,
    pub total_size_bytes: i64,
    pub albums: Vec<String>,
}
