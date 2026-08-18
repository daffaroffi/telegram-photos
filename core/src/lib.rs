//! Telegram Photos — Rust core (no Tauri / no Flutter dependencies).
//!
//! Pure business logic shared between the Tauri desktop wrapper
//! (`app/src-tauri`) and the Flutter UI (`app_flutter/rust` via
//! flutter_rust_bridge): SQLite schema + migration, media pipeline
//! (hash / EXIF / thumbnails), offline reverse geocoding and data models.
//!
//! Telegram MTProto, crypto, Google import and the backup engine will move
//! here progressively as they are decoupled from Tauri state.

pub mod crypto;
pub mod db;
pub mod geo;
pub mod media;
pub mod models;
