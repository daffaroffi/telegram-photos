//! Background hash worker (PRD Part 2 S7.2).
//!
//! SHA-256 streaming in a dedicated thread, not blocking UI or scan.
//! Priority: new files (before upload) > unhashed files.

use std::collections::VecDeque;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

/// A file to hash.
#[derive(Debug, Clone)]
pub struct HashJob {
    pub media_id: String,
    pub file_path: String,
    pub priority: u8, // 0 = highest (before upload), 1 = normal
}

/// Hash result.
#[derive(Debug, Clone)]
pub struct HashResult {
    pub media_id: String,
    pub sha256: String,
    pub success: bool,
    pub error: Option<String>,
}

/// Background hash worker.
pub struct HashWorker {
    queue: Arc<Mutex<VecDeque<HashJob>>>,
    result_tx: mpsc::UnboundedSender<HashResult>,
}

impl HashWorker {
    pub fn new() -> (Arc<Self>, mpsc::UnboundedReceiver<HashResult>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let worker = Arc::new(Self {
            queue: Arc::new(Mutex::new(VecDeque::new())),
            result_tx: tx,
        });
        (worker, rx)
    }

    /// Enqueue a file for hashing.
    pub async fn enqueue(&self, job: HashJob) {
        let mut queue = self.queue.lock().await;
        // Insert by priority (0 = highest)
        let pos = queue
            .iter()
            .position(|j| j.priority > job.priority)
            .unwrap_or(queue.len());
        queue.insert(pos, job);
    }

    /// Process next job from queue (called by worker loop).
    pub async fn process_next(&self) -> Option<HashResult> {
        let job = {
            let mut queue = self.queue.lock().await;
            queue.pop_front()
        };

        let job = job?;

        let result = if Path::new(&job.file_path).exists() {
            match compute_sha256_streaming(&job.file_path) {
                Ok(hash) => HashResult {
                    media_id: job.media_id,
                    sha256: hash,
                    success: true,
                    error: None,
                },
                Err(e) => HashResult {
                    media_id: job.media_id,
                    sha256: String::new(),
                    success: false,
                    error: Some(e),
                },
            }
        } else {
            HashResult {
                media_id: job.media_id,
                sha256: String::new(),
                success: false,
                error: Some("File not found".to_string()),
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

/// Compute SHA-256 of a file using streaming (1 MB buffer).
/// Budget: hash 1 GB < 10s on mid-range device (PRD S7.2).
fn compute_sha256_streaming(path: &str) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;

    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1024 * 1024]; // 1 MB buffer

    loop {
        let bytes_read = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}
