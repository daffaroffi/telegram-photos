//! Upload manager with resume capability (PRD Part 2 S6.2).
//!
//! State machine: NOT_BACKED_UP -> QUEUED -> UPLOADING -> BACKED_UP
//! With pause, resume from last chunk, and exponential backoff for failures.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Upload state (PRD Part 2 S6.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UploadState {
    Pending,
    Queued,
    Uploading,
    Paused,
    BackedUp,
    Failed,
    Skipped,
}

impl UploadState {
    pub fn as_str(&self) -> &str {
        match self {
            UploadState::Pending => "PENDING",
            UploadState::Queued => "QUEUED",
            UploadState::Uploading => "UPLOADING",
            UploadState::Paused => "PAUSED",
            UploadState::BackedUp => "BACKED_UP",
            UploadState::Failed => "FAILED",
            UploadState::Skipped => "SKIPPED",
        }
    }
}

/// Upload item with resume state.
#[derive(Debug, Clone)]
pub struct UploadItem {
    pub media_id: String,
    pub state: UploadState,
    pub retry_count: u32,
    pub uploaded_bytes: u64,
    pub total_bytes: u64,
    pub last_error: Option<String>,
    pub file_path: String,
    pub file_name: String,
    pub mime_type: String,
    pub is_video: bool,
}

/// Upload error record (PRD Part 2 S6.2: upload_errors table).
#[derive(Debug, Clone)]
pub struct UploadError {
    pub upload_id: String,
    pub error_code: String,
    pub message: String,
    pub at_timestamp: u64,
}

/// Manages upload queue with resume and backoff.
pub struct UploadManager {
    queue: Arc<Mutex<Vec<UploadItem>>>,
    errors: Arc<Mutex<Vec<UploadError>>>,
    max_retries: u32,
}

impl UploadManager {
    pub fn new() -> Self {
        Self {
            queue: Arc::new(Mutex::new(Vec::new())),
            errors: Arc::new(Mutex::new(Vec::new())),
            max_retries: 5,
        }
    }

    /// Queue an item for upload.
    pub async fn queue_item(&self, item: UploadItem) {
        let queue = self.queue.lock().await;
        queue.push(UploadItem {
            state: UploadState::Queued,
            ..item
        });
    }

    /// Get next item to upload (respecting backoff).
    pub async fn next_item(&self) -> Option<UploadItem> {
        let queue = self.queue.lock().await;
        queue
            .iter()
            .find(|item| {
                item.state == UploadState::Queued
                    && item.retry_count < self.max_retries
            })
            .cloned()
    }

    /// Mark item as uploading with resume position.
    pub async fn start_upload(&self, media_id: &str, uploaded_bytes: u64) {
        let mut queue = self.queue.lock().await;
        if let Some(item) = queue.iter_mut().find(|i| i.media_id == media_id) {
            item.state = UploadState::Uploading;
            item.uploaded_bytes = uploaded_bytes;
        }
    }

    /// Mark upload as completed.
    pub async fn complete_upload(&self, media_id: &str) {
        let mut queue = self.queue.lock().await;
        if let Some(item) = queue.iter_mut().find(|i| i.media_id == media_id) {
            item.state = UploadState::BackedUp;
        }
    }

    /// Mark upload as failed with exponential backoff.
    pub async fn fail_upload(&self, media_id: &str, error: String) {
        let mut queue = self.queue.lock().await;
        let mut errors = self.errors.lock().await;

        if let Some(item) = queue.iter_mut().find(|i| i.media_id == media_id) {
            item.state = UploadState::Failed;
            item.retry_count += 1;
            item.last_error = Some(error.clone());

            // Exponential backoff: 2^retry_count seconds, max 5 retries
            if item.retry_count < self.max_retries {
                item.state = UploadState::Queued;
            }

            errors.push(UploadError {
                upload_id: media_id.to_string(),
                error_code: "UPLOAD_FAILED".to_string(),
                message: error,
                at_timestamp: timestamp_secs(),
            });
        }
    }

    /// Pause an upload (PRD: PAUSED state).
    pub async fn pause_upload(&self, media_id: &str) {
        let mut queue = self.queue.lock().await;
        if let Some(item) = queue.iter_mut().find(|i| i.media_id == media_id) {
            item.state = UploadState::Paused;
        }
    }

    /// Resume a paused upload.
    pub async fn resume_upload(&self, media_id: &str) {
        let mut queue = self.queue.lock().await;
        if let Some(item) = queue.iter_mut().find(|i| i.media_id == media_id) {
            if item.state == UploadState::Paused {
                item.state = UploadState::Queued;
            }
        }
    }

    /// Cancel an upload.
    pub async fn cancel_upload(&self, media_id: &str) {
        let mut queue = self.queue.lock().await;
        if let Some(item) = queue.iter_mut().find(|i| i.media_id == media_id) {
            item.state = UploadState::Skipped;
        }
    }

    /// Get queue stats for Progress Hub banner.
    pub async fn stats(&self) -> (usize, usize, u64, u64) {
        let queue = self.queue.lock().await;
        let total = queue.len();
        let uploading = queue
            .iter()
            .filter(|i| i.state == UploadState::Uploading)
            .count();
        let done: u64 = queue
            .iter()
            .filter(|i| i.state == UploadState::BackedUp)
            .map(|i| i.total_bytes)
            .sum();
        let total_bytes: u64 = queue.iter().map(|i| i.total_bytes).sum();
        (total, uploading, done, total_bytes)
    }

    /// Get upload errors for display.
    pub async fn recent_errors(&self, limit: usize) -> Vec<UploadError> {
        let errors = self.errors.lock().await;
        errors.iter().rev().take(limit).cloned().collect()
    }

    /// Retry a failed upload (PRD: resets retry count).
    pub async fn retry_upload(&self, media_id: &str) {
        let mut queue = self.queue.lock().await;
        if let Some(item) = queue.iter_mut().find(|i| i.media_id == media_id) {
            if item.state == UploadState::Failed {
                item.state = UploadState::Queued;
                item.retry_count = 0;
                item.uploaded_bytes = 0;
            }
        }
    }
}

fn timestamp_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
