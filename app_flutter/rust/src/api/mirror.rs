//! FRB mirrors for types defined in `telegram_photos_core`.
//!
//! flutter_rust_bridge can only inspect the local crate (cargo-expand expands
//! one crate), so structs from a path dependency fall back to opaque. The
//! documented "mirroring" feature fixes that: the placeholder definitions
//! below are compile-checked against the originals and only used at
//! codegen time. The external types must also be re-exported publicly.

use flutter_rust_bridge::frb;

pub use telegram_photos_core::models::{
    Album, AppSettings, AuthCodeResult, Collection, MediaItem, TelegramUser, Upload, UploadError, UploadsSummary,
    VaultInfo,
};

#[frb(mirror(AuthCodeResult))]
pub struct _AuthCodeResult {
    pub status: String,
    pub code_length: Option<i32>,
    pub resend_after_seconds: Option<i32>,
    pub delivery: Option<String>,
}

#[frb(mirror(MediaItem))]
pub struct _MediaItem {
    pub id: String,
    pub local_identifier: Option<String>,
    pub file_name: String,
    pub file_path: Option<String>,
    pub mime_type: String,
    pub media_type: String,
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
    pub sync_status: String,
    pub upload_progress: Option<i64>,
    pub error_message: Option<String>,
    pub tg_channel_id: Option<i64>,
    pub tg_message_id: Option<i64>,
    pub tg_file_id: Option<String>,
    pub tg_access_hash: Option<i64>,
    pub imported_from_google_photos: bool,
    pub google_photos_media_id: Option<String>,
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

#[frb(mirror(Album))]
pub struct _Album {
    pub id: String,
    pub name: String,
    pub created_at: i64,
    pub cover_media_id: Option<String>,
    pub is_pinned: bool,
    pub source_type: String,
    pub item_count: i64,
}

#[frb(mirror(AppSettings))]
pub struct _AppSettings {
    pub auto_backup_enabled: bool,
    pub backup_over_wifi_only: bool,
    pub backup_while_charging_only: bool,
    pub upload_original_quality: bool,
    pub folder_backup_settings: std::collections::HashMap<String, bool>,
    pub client_encryption_enabled: bool,
    pub vault_passphrase_set: bool,
    pub grid_column_count: i64,
    pub theme: String,
    pub telegram_api_id: Option<String>,
    pub telegram_api_hash: Option<String>,
    pub google_client_id: Option<String>,
    pub google_client_secret: Option<String>,
}

#[frb(mirror(VaultInfo))]
pub struct _VaultInfo {
    pub channel_id: Option<i64>,
    pub channel_title: String,
    pub is_private: bool,
    pub total_storage_used_bytes: i64,
    pub total_backed_up_files: i64,
    pub last_sync_timestamp: i64,
}

#[frb(mirror(TelegramUser))]
pub struct _TelegramUser {
    pub id: i64,
    pub first_name: String,
    pub last_name: Option<String>,
    pub username: Option<String>,
    pub phone: String,
    pub is_premium: bool,
}

#[frb(mirror(Upload))]
pub struct _Upload {
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

#[frb(mirror(UploadError))]
pub struct _UploadError {
    pub id: i64,
    pub upload_id: String,
    pub error_code: Option<String>,
    pub message: String,
    pub at: i64,
}

#[frb(mirror(UploadsSummary))]
pub struct _UploadsSummary {
    pub queued_count: i64,
    pub queued_bytes: i64,
    pub uploading_count: i64,
    pub uploading_bytes: i64,
    pub failed_count: i64,
    pub failed_bytes: i64,
    pub backed_up_count: i64,
    pub backed_up_bytes: i64,
}

#[frb(mirror(Collection))]
pub struct _Collection {
    pub id: String,
    pub name: String,
    pub cover_media_id: Option<String>,
    pub is_cloud: bool,
    pub sort_order: i64,
    pub created_at: i64,
    pub item_count: i64,
}
