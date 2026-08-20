//! Task Progress Hub (PRD Part 2 S6.1).
//!
//! Central registry for all background tasks (upload, scan, hash, restore).
//! Events are batched and throttled before sending to Dart.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

/// Task kind.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TaskKind {
    Upload,
    Scan,
    Hash,
    Restore,
    FreeUpSpace,
}

/// Task status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskStatus {
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

/// A single task in the progress hub.
#[derive(Debug, Clone)]
pub struct Task {
    pub id: u64,
    pub kind: TaskKind,
    pub total: u64,
    pub done: u64,
    pub status: TaskStatus,
    pub message: String,
    pub created_at: u64,
    pub updated_at: u64,
}

/// Event sent to Dart UI (throttled at 50ms per PRD S7.9).
#[derive(Debug, Clone)]
pub struct TaskEvent {
    pub task_id: u64,
    pub kind: String,
    pub total: u64,
    pub done: u64,
    pub status: String,
    pub message: String,
}

/// The Task Progress Hub.
pub struct TaskHub {
    tasks: RwLock<HashMap<u64, Task>>,
    next_id: AtomicU64,
    event_tx: mpsc::UnboundedSender<TaskEvent>,
}

impl TaskHub {
    pub fn new() -> (Arc<Self>, mpsc::UnboundedReceiver<TaskEvent>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let hub = Arc::new(Self {
            tasks: RwLock::new(HashMap::new()),
            next_id: AtomicU64::new(1),
            event_tx: tx,
        });
        (hub, rx)
    }

    /// Start a new task. Returns the task ID.
    pub async fn start_task(&self, kind: TaskKind, total: u64, message: &str) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let now = timestamp_secs();
        let task = Task {
            id,
            kind: kind.clone(),
            total,
            done: 0,
            status: TaskStatus::Running,
            message: message.to_string(),
            created_at: now,
            updated_at: now,
        };
        self.tasks.write().await.insert(id, task);
        self.emit_event(id).await;
        id
    }

    /// Update progress for a task.
    pub async fn update_task(&self, id: u64, done: u64, message: Option<&str>) {
        if let Some(task) = self.tasks.write().await.get_mut(&id) {
            task.done = done;
            task.updated_at = timestamp_secs();
            if let Some(msg) = message {
                task.message = msg.to_string();
            }
            self.emit_event(id).await;
        }
    }

    /// Complete a task.
    pub async fn complete_task(&self, id: u64) {
        if let Some(task) = self.tasks.write().await.get_mut(&id) {
            task.status = TaskStatus::Completed;
            task.done = task.total;
            task.updated_at = timestamp_secs();
            self.emit_event(id).await;
        }
    }

    /// Fail a task.
    pub async fn fail_task(&self, id: u64, message: &str) {
        if let Some(task) = self.tasks.write().await.get_mut(&id) {
            task.status = TaskStatus::Failed;
            task.message = message.to_string();
            task.updated_at = timestamp_secs();
            self.emit_event(id).await;
        }
    }

    /// Pause a task.
    pub async fn pause_task(&self, id: u64) {
        if let Some(task) = self.tasks.write().await.get_mut(&id) {
            task.status = TaskStatus::Paused;
            task.updated_at = timestamp_secs();
            self.emit_event(id).await;
        }
    }

    /// Resume a paused task.
    pub async fn resume_task(&self, id: u64) {
        if let Some(task) = self.tasks.write().await.get_mut(&id) {
            task.status = TaskStatus::Running;
            task.updated_at = timestamp_secs();
            self.emit_event(id).await;
        }
    }

    /// Cancel a task.
    pub async fn cancel_task(&self, id: u64) {
        if let Some(task) = self.tasks.write().await.get_mut(&id) {
            task.status = TaskStatus::Cancelled;
            task.updated_at = timestamp_secs();
            self.emit_event(id).await;
        }
    }

    /// Get all tasks (for Progress Hub UI).
    pub async fn list_tasks(&self) -> Vec<Task> {
        let tasks = self.tasks.read().await;
        let mut list: Vec<Task> = tasks.values().cloned().collect();
        list.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        list
    }

    /// Get active task count (for badge in topbar).
    pub async fn active_count(&self) -> usize {
        self.tasks
            .read()
            .await
            .values()
            .filter(|t| t.status == TaskStatus::Running)
            .count()
    }

    async fn emit_event(&self, task_id: u64) {
        let tasks = self.tasks.read().await;
        if let Some(task) = tasks.get(&task_id) {
            let event = TaskEvent {
                task_id: task.id,
                kind: format!("{:?}", task.kind),
                total: task.total,
                done: task.done,
                status: format!("{:?}", task.status),
                message: task.message.clone(),
            };
            let _ = self.event_tx.send(event);
        }
    }
}

fn timestamp_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
