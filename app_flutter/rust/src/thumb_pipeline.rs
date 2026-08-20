//! Multi-tier thumbnail pipeline (PRD Part 2 S7.1, G1).
//!
//! Tier 0: BlurHash (16-32 char, in DB) — instant placeholder, 0ms
//! Tier 1: 96px JPEG (~3-6 KB) — grid 5-8 columns
//! Tier 2: 256px JPEG (~15-25 KB) — grid 1-3 columns (default)
//! Tier 3: 1200px JPEG (~100-200 KB) — lightbox/preview (on demand)
//!
//! State machine: UNCACHED -> CACHED | FAILED
//! FAILED is not retried continuously — render BlurHash, retry when idle.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

/// Thumbnail tier (PRD S7.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThumbTier {
    /// Placeholder (BlurHash in DB, not a file)
    BlurHash = 0,
    /// 96px JPEG — heatmap/grid overview
    Small = 1,
    /// 256px JPEG — standard grid
    Medium = 2,
    /// 1200px JPEG — preview/lightbox (on demand)
    Large = 3,
}

impl ThumbTier {
    pub fn target_px(&self) -> u32 {
        match self {
            ThumbTier::BlurHash => 0,
            ThumbTier::Small => 96,
            ThumbTier::Medium => 256,
            ThumbTier::Large => 1200,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            ThumbTier::BlurHash => "blurhash",
            ThumbTier::Small => "96px",
            ThumbTier::Medium => "256px",
            ThumbTier::Large => "1200px",
        }
    }
}

/// Thumbnail state (PRD G1: THUMBNAIL_CACHED / UNCACHED / FAILED).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThumbStatus {
    Uncached,
    Cached,
    Failed,
}

impl ThumbStatus {
    pub fn as_str(&self) -> &str {
        match self {
            ThumbStatus::Uncached => "UNCACHED",
            ThumbStatus::Cached => "CACHED",
            ThumbStatus::Failed => "FAILED",
        }
    }
}

/// A thumbnail job.
#[derive(Debug, Clone)]
pub struct ThumbJob {
    pub media_id: String,
    pub source_path: String,
    pub tier: ThumbTier,
}

/// Thumbnail pipeline result.
#[derive(Debug, Clone)]
pub struct ThumbResult {
    pub media_id: String,
    pub tier: ThumbTier,
    pub output_path: Option<String>,
    pub status: ThumbStatus,
}

/// Multi-tier thumbnail pipeline.
pub struct ThumbPipeline {
    queue: Arc<Mutex<Vec<ThumbJob>>>,
    result_tx: mpsc::UnboundedSender<ThumbResult>,
    thumb_dir: String,
}

impl ThumbPipeline {
    pub fn new(thumb_dir: String) -> (Arc<Self>, mpsc::UnboundedReceiver<ThumbResult>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let pipeline = Arc::new(Self {
            queue: Arc::new(Mutex::new(Vec::new())),
            result_tx: tx,
            thumb_dir,
        });
        (pipeline, rx)
    }

    /// Enqueue a thumbnail generation job.
    pub async fn enqueue(&self, job: ThumbJob) {
        let mut queue = self.queue.lock().await;
        // Deduplicate: don't queue same media_id + tier twice
        if !queue.iter().any(|j| j.media_id == job.media_id && j.tier == job.tier) {
            queue.push(job);
        }
    }

    /// Process next job (called by worker loop).
    pub async fn process_next(&self) -> Option<ThumbResult> {
        // Pop the job out of the queue under the lock so two concurrent
        // workers cannot both claim it. A previous peek-then-remove
        // pattern let both see the same job, doubling ffmpeg runs.
        let job = {
            let mut queue = self.queue.lock().await;
            if queue.is_empty() {
                None
            } else {
                Some(queue.remove(0))
            }
        };

        let job = job?;

        let result = if Path::new(&job.source_path).exists()
            && job.tier != ThumbTier::BlurHash
        {
            let output_path = format!(
                "{}/{}_{}.jpg",
                self.thumb_dir,
                &job.media_id[..16.min(job.media_id.len())],
                job.tier.label()
            );

            match generate_thumbnail(&job.source_path, &output_path, job.tier.target_px()) {
                Ok(_) => ThumbResult {
                    media_id: job.media_id,
                    tier: job.tier,
                    output_path: Some(output_path),
                    status: ThumbStatus::Cached,
                },
                Err(_) => ThumbResult {
                    media_id: job.media_id,
                    tier: job.tier,
                    output_path: None,
                    status: ThumbStatus::Failed,
                },
            }
        } else {
            ThumbResult {
                media_id: job.media_id,
                tier: job.tier,
                output_path: None,
                status: if job.tier == ThumbTier::BlurHash {
                    ThumbStatus::Cached // BlurHash is always "cached" in DB
                } else {
                    ThumbStatus::Failed
                },
            }
        };

        let _ = self.result_tx.send(result.clone());
        Some(result)
    }

    /// Check if queue is empty.
    pub async fn is_empty(&self) -> bool {
        self.queue.lock().await.is_empty()
    }

    /// Queue size.
    pub async fn pending_count(&self) -> usize {
        self.queue.lock().await.len()
    }
}

/// Generate a JPEG thumbnail at the target pixel size.
fn generate_thumbnail(source: &str, output: &str, target_px: u32) -> Result<(), String> {
    // Use ffmpeg for thumbnail generation (already available on the system)
    use std::process::Command;

    let status = Command::new("ffmpeg")
        .args([
            "-y",
            "-i", source,
            "-vf", &format!(
                "scale='min({tx}px,iw)':'min({tx}px,ih)':force_original_aspect_ratio=decrease",
                tx = target_px
            ),
            "-frames:v", "1",
            "-q:v", "3",
            output,
        ])
        .output()
        .map_err(|e| format!("ffmpeg not found: {e}"))?;

    if status.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&status.stderr).to_string())
    }
}
