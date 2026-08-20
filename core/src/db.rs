//! Local SQLite database (`telegram_photos.db`) implementing the exact schema
//! from PRD section 5, tuned with WAL pragmas for 100k+ photos (PRD section 11.3).

use crate::models::{Album, AppSettings, Collection, GoogleImportSession, MediaItem, Upload, UploadError, UploadsSummary, VaultInfo};
use sqlite::{Connection, ConnectionThreadSafe, State, Statement, Value};
use std::path::Path;

pub const VAULT_META_KEY: &str = "vault_meta";
pub const GOOGLE_TOKENS_KEY: &str = "google_tokens";

/// Thread-safe database handle (sqlite `ConnectionThreadSafe` is `Send + Sync`,
/// which Tauri's managed state requires).
pub struct Db {
    conn: ConnectionThreadSafe,
}

// ─────────────────────────────────────────────────────────────────────────────
// Schema (PRD section 5)
// ─────────────────────────────────────────────────────────────────────────────

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS media_items (
    id TEXT PRIMARY KEY,
    local_identifier TEXT UNIQUE,
    file_name TEXT NOT NULL,
    file_path TEXT,
    mime_type TEXT NOT NULL,
    media_type TEXT NOT NULL DEFAULT 'image',
    file_size_bytes INTEGER NOT NULL DEFAULT 0,
    sha256_hash TEXT NOT NULL,

    date_taken INTEGER NOT NULL,
    date_added INTEGER NOT NULL,

    width INTEGER,
    height INTEGER,
    orientation INTEGER DEFAULT 0,
    duration_ms INTEGER DEFAULT 0,

    camera_make TEXT,
    camera_model TEXT,
    focal_length REAL,
    aperture REAL,
    iso INTEGER,
    exposure_time TEXT,

    latitude REAL,
    longitude REAL,
    geo_city TEXT,
    geo_country TEXT,

    sync_status TEXT NOT NULL DEFAULT 'NOT_BACKED_UP',
    upload_progress INTEGER DEFAULT 0,
    error_message TEXT,

    tg_channel_id INTEGER,
    tg_message_id INTEGER,
    tg_file_id TEXT,
    tg_access_hash INTEGER,

    imported_from_google_photos INTEGER DEFAULT 0,
    google_photos_media_id TEXT,
    google_cleanup_status TEXT DEFAULT 'NONE',

    thumbnail_path TEXT,
    preview_path TEXT,
    blur_hash TEXT,

    is_favorite INTEGER DEFAULT 0,
    is_archived INTEGER DEFAULT 0,
    is_trashed INTEGER DEFAULT 0,
    trashed_timestamp INTEGER,
    is_encrypted INTEGER DEFAULT 0,

    album_ids TEXT DEFAULT '[]',
    device_folder TEXT
);

CREATE INDEX IF NOT EXISTS idx_media_date_taken ON media_items(date_taken DESC);
CREATE INDEX IF NOT EXISTS idx_media_sync_status ON media_items(sync_status);
CREATE INDEX IF NOT EXISTS idx_media_geo_city ON media_items(geo_city);
CREATE INDEX IF NOT EXISTS idx_media_is_trashed ON media_items(is_trashed);
CREATE INDEX IF NOT EXISTS idx_media_sha256 ON media_items(sha256_hash);
CREATE INDEX IF NOT EXISTS idx_media_google_id ON media_items(google_photos_media_id);
CREATE INDEX IF NOT EXISTS idx_media_date_added ON media_items(date_added DESC);

CREATE TABLE IF NOT EXISTS google_import_sessions (
    session_id TEXT PRIMARY KEY,
    google_account_email TEXT NOT NULL,
    started_at INTEGER NOT NULL,
    completed_at INTEGER,
    total_items_found INTEGER DEFAULT 0,
    items_imported_success INTEGER DEFAULT 0,
    items_imported_failed INTEGER DEFAULT 0,
    total_bytes_migrated INTEGER DEFAULT 0,
    post_cleanup_choice TEXT,
    cleanup_completed_at INTEGER,
    status TEXT NOT NULL DEFAULT 'RUNNING'
);

CREATE TABLE IF NOT EXISTS albums (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    cover_media_id TEXT,
    is_pinned INTEGER DEFAULT 0,
    source_type TEXT DEFAULT 'LOCAL'
);

CREATE TABLE IF NOT EXISTS album_media_map (
    album_id TEXT NOT NULL,
    media_id TEXT NOT NULL,
    added_at INTEGER NOT NULL,
    PRIMARY KEY (album_id, media_id),
    FOREIGN KEY (album_id) REFERENCES albums(id) ON DELETE CASCADE,
    FOREIGN KEY (media_id) REFERENCES media_items(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS backup_folders (
    folder_path TEXT PRIMARY KEY,
    folder_name TEXT NOT NULL,
    is_backup_enabled INTEGER DEFAULT 1,
    last_scanned_timestamp INTEGER
);

CREATE TABLE IF NOT EXISTS app_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS vault_meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
"#;

// Schema v2 (PRD Part 2 §6.7) — upload state machine, captions & hashtags,
// collections. Idempotent (IF NOT EXISTS) so it can run on v1 databases.
const SCHEMA_V2: &str = r#"
CREATE TABLE IF NOT EXISTS uploads (
    id TEXT PRIMARY KEY,
    media_id TEXT NOT NULL,
    message_id INTEGER,
    file_id TEXT,
    hash_sha256 TEXT,
    status TEXT NOT NULL DEFAULT 'PENDING',
    retry_count INTEGER DEFAULT 0,
    last_error TEXT,
    uploaded_bytes INTEGER DEFAULT 0,
    total_bytes INTEGER DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_uploads_status ON uploads(status);
CREATE INDEX IF NOT EXISTS idx_uploads_media ON uploads(media_id);

CREATE TABLE IF NOT EXISTS upload_errors (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    upload_id TEXT NOT NULL,
    error_code TEXT,
    message TEXT,
    at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_upload_errors_upload ON upload_errors(upload_id);

CREATE TABLE IF NOT EXISTS captions (
    id TEXT PRIMARY KEY,
    media_id TEXT UNIQUE NOT NULL,
    text TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_captions_media ON captions(media_id);

CREATE TABLE IF NOT EXISTS caption_tags (
    id TEXT PRIMARY KEY,
    media_id TEXT NOT NULL,
    tag TEXT NOT NULL,
    UNIQUE(media_id, tag)
);
CREATE INDEX IF NOT EXISTS idx_caption_tags_tag ON caption_tags(tag);

CREATE TABLE IF NOT EXISTS collections (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    cover_media_id TEXT,
    is_cloud INTEGER DEFAULT 0,
    sort_order INTEGER DEFAULT 0,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS collection_items (
    collection_id TEXT NOT NULL,
    media_id TEXT NOT NULL,
    added_at INTEGER NOT NULL,
    PRIMARY KEY (collection_id, media_id)
);
CREATE INDEX IF NOT EXISTS idx_collection_items_coll ON collection_items(collection_id);
"#;

impl Db {
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut conn = Connection::open_thread_safe(path).map_err(|e| e.to_string())?;
        // PRD 11.3: WAL + performance pragmas
        conn.execute(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA cache_size = -8000;
             PRAGMA temp_store = MEMORY;
             PRAGMA foreign_keys = ON;",
        )
        .map_err(|e| e.to_string())?;
        conn.set_busy_timeout(30_000).map_err(|e| e.to_string())?;

        let db = Self { conn };
        db.execute_batch(SCHEMA)?;
        db.migrate_to_v2()?;
        Ok(db)
    }

    /// Schema migration v1 → v2 (PRD Part 2 §6.7) guarded by `PRAGMA user_version`.
    /// Adds the upload state machine tables, captions/hashtags, collections and
    /// the Google-Photos-style `thumb_status` column (G1). Never resets user data.
    fn migrate_to_v2(&self) -> Result<(), String> {
        let version = self.user_version()?;
        if version >= 2 {
            return Ok(());
        }
        self.execute_batch(SCHEMA_V2)?;

        // Add thumb_status (CACHED | UNCACHED | FAILED) if missing.
        let mut stmt = self
            .conn
            .prepare("SELECT COUNT(*) FROM pragma_table_info('media_items') WHERE name = 'thumb_status'")
            .map_err(|e| e.to_string())?;
        let has_column = if let State::Row = stmt.next().map_err(|e| e.to_string())? {
            stmt.read::<i64, _>(0).map_err(|e| e.to_string())? != 0
        } else {
            false
        };
        if !has_column {
            self.conn
                .execute("ALTER TABLE media_items ADD COLUMN thumb_status TEXT DEFAULT 'UNCACHED'")
                .map_err(|e| e.to_string())?;
        }

        self.conn
            .execute("PRAGMA user_version = 2")
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn user_version(&self) -> Result<i64, String> {
        let mut stmt = self
            .conn
            .prepare("PRAGMA user_version")
            .map_err(|e| e.to_string())?;
        if let State::Row = stmt.next().map_err(|e| e.to_string())? {
            Ok(stmt.read::<i64, _>(0).map_err(|e| e.to_string())?)
        } else {
            Ok(0)
        }
    }

    pub fn execute_batch(&self, sql: &str) -> Result<(), String> {
        self.conn.execute(sql).map_err(|e| e.to_string())
    }

    pub fn exec(&self, sql: &str, params: &[(usize, Value)]) -> Result<usize, String> {
        let mut stmt = self.conn.prepare(sql).map_err(|e| e.to_string())?;
        stmt.bind(params).map_err(|e| e.to_string())?;
        stmt.next().map_err(|e| e.to_string())?;
        Ok(self.conn.change_count())
    }

    // ── Settings ────────────────────────────────────────────────────────────

    pub fn get_settings(&self) -> Result<AppSettings, String> {
        match self.get_key("app_settings", "settings")? {
            Some(json) => serde_json::from_str(&json).map_err(|e| e.to_string()),
            None => Ok(AppSettings::default()),
        }
    }

    pub fn save_settings(&self, settings: &AppSettings) -> Result<(), String> {
        let json = serde_json::to_string(settings).map_err(|e| e.to_string())?;
        self.set_key("app_settings", "settings", &json)
    }

    fn get_key(&self, table: &str, key: &str) -> Result<Option<String>, String> {
        let sql = format!("SELECT value FROM {} WHERE key = ?", table);
        let mut stmt = self.conn.prepare(&sql).map_err(|e| e.to_string())?;
        stmt.bind(&[(1usize, Value::String(key.to_string()))][..])
            .map_err(|e| e.to_string())?;
        match stmt.next().map_err(|e| e.to_string())? {
            State::Row => {
                let value: String = stmt.read(0).map_err(|e| e.to_string())?;
                Ok(Some(value))
            }
            State::Done => Ok(None),
        }
    }

    fn set_key(&self, table: &str, key: &str, value: &str) -> Result<(), String> {
        let sql = format!(
            "INSERT INTO {} (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            table
        );
        self.exec(
            &sql,
            &[
                (1usize, Value::String(key.to_string())),
                (2usize, Value::String(value.to_string())),
            ],
        )
        .map(|_| ())
    }

    pub fn set_json(&self, key: &str, value: &serde_json::Value) -> Result<(), String> {
        self.set_key("app_settings", key, &value.to_string())
    }

    pub fn get_json(&self, key: &str) -> Result<Option<serde_json::Value>, String> {
        Ok(self
            .get_key("app_settings", key)?
            .and_then(|v| serde_json::from_str(&v).ok()))
    }

    pub fn set_vault_meta(&self, value: &str) -> Result<(), String> {
        self.set_key("vault_meta", VAULT_META_KEY, value)
    }

    pub fn get_vault_meta(&self) -> Result<Option<String>, String> {
        self.get_key("vault_meta", VAULT_META_KEY)
    }

    // ── Media items ─────────────────────────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    pub fn upsert_media(&self, item: &MediaItem) -> Result<(), String> {
        let sql = r#"
        INSERT INTO media_items (
            id, local_identifier, file_name, file_path, mime_type, media_type,
            file_size_bytes, sha256_hash, date_taken, date_added, width, height,
            orientation, duration_ms, camera_make, camera_model, focal_length,
            aperture, iso, exposure_time, latitude, longitude, geo_city, geo_country,
            sync_status, upload_progress, error_message, tg_channel_id, tg_message_id,
            tg_file_id, tg_access_hash, imported_from_google_photos,
            google_photos_media_id, google_cleanup_status, thumbnail_path, preview_path,
            blur_hash, is_favorite, is_archived, is_trashed, trashed_timestamp,
            is_encrypted, album_ids, device_folder
        ) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)
        ON CONFLICT(id) DO UPDATE SET
            file_name=excluded.file_name, file_path=excluded.file_path,
            mime_type=excluded.mime_type, media_type=excluded.media_type,
            file_size_bytes=excluded.file_size_bytes, sha256_hash=excluded.sha256_hash,
            date_taken=excluded.date_taken, date_added=excluded.date_added,
            width=excluded.width, height=excluded.height, orientation=excluded.orientation,
            duration_ms=excluded.duration_ms, camera_make=excluded.camera_make,
            camera_model=excluded.camera_model, focal_length=excluded.focal_length,
            aperture=excluded.aperture, iso=excluded.iso, exposure_time=excluded.exposure_time,
            latitude=excluded.latitude, longitude=excluded.longitude,
            geo_city=excluded.geo_city, geo_country=excluded.geo_country,
            sync_status=excluded.sync_status, upload_progress=excluded.upload_progress,
            error_message=excluded.error_message, tg_channel_id=excluded.tg_channel_id,
            tg_message_id=excluded.tg_message_id, tg_file_id=excluded.tg_file_id,
            tg_access_hash=excluded.tg_access_hash,
            imported_from_google_photos=excluded.imported_from_google_photos,
            google_photos_media_id=excluded.google_photos_media_id,
            google_cleanup_status=excluded.google_cleanup_status,
            thumbnail_path=excluded.thumbnail_path, preview_path=excluded.preview_path,
            blur_hash=excluded.blur_hash, is_favorite=excluded.is_favorite,
            is_archived=excluded.is_archived, is_trashed=excluded.is_trashed,
            trashed_timestamp=excluded.trashed_timestamp, is_encrypted=excluded.is_encrypted,
            album_ids=excluded.album_ids, device_folder=excluded.device_folder
        "#;
        let p = |v: Option<i64>| v.map(Value::Integer).unwrap_or(Value::Null);
        let pf = |v: Option<f64>| v.map(Value::Float).unwrap_or(Value::Null);
        let ps = |v: &Option<String>| v.clone().map(Value::String).unwrap_or(Value::Null);
        let pi = |v: i64| Value::Integer(v);
        let pb = |v: bool| Value::Integer(if v { 1 } else { 0 });

        let params: Vec<(usize, Value)> = vec![
            (1, Value::String(item.id.clone())),
            (2, ps(&item.local_identifier)),
            (3, Value::String(item.file_name.clone())),
            (4, ps(&item.file_path)),
            (5, Value::String(item.mime_type.clone())),
            (6, Value::String(item.media_type.clone())),
            (7, pi(item.file_size_bytes)),
            (8, Value::String(item.sha256_hash.clone())),
            (9, pi(item.date_taken)),
            (10, pi(item.date_added)),
            (11, p(item.width)),
            (12, p(item.height)),
            (13, p(item.orientation)),
            (14, p(item.duration_ms)),
            (15, ps(&item.camera_make)),
            (16, ps(&item.camera_model)),
            (17, pf(item.focal_length)),
            (18, pf(item.aperture)),
            (19, p(item.iso)),
            (20, ps(&item.exposure_time)),
            (21, pf(item.latitude)),
            (22, pf(item.longitude)),
            (23, ps(&item.geo_city)),
            (24, ps(&item.geo_country)),
            (25, Value::String(item.sync_status.clone())),
            (26, p(item.upload_progress)),
            (27, ps(&item.error_message)),
            (28, p(item.tg_channel_id)),
            (29, p(item.tg_message_id)),
            (30, ps(&item.tg_file_id)),
            (31, p(item.tg_access_hash)),
            (32, pb(item.imported_from_google_photos)),
            (33, ps(&item.google_photos_media_id)),
            (34, ps(&item.google_cleanup_status)),
            (35, ps(&item.thumbnail_path)),
            (36, ps(&item.preview_path)),
            (37, ps(&item.blur_hash)),
            (38, pb(item.is_favorite)),
            (39, pb(item.is_archived)),
            (40, pb(item.is_trashed)),
            (41, p(item.trashed_timestamp)),
            (42, pb(item.is_encrypted)),
            (
                43,
                Value::String(serde_json::to_string(&item.album_ids).unwrap_or("[]".into())),
            ),
            (44, ps(&item.device_folder)),
        ];
        self.exec(sql, &params).map(|_| ())
    }

    pub fn get_media(&self, id: &str) -> Result<Option<MediaItem>, String> {
        self.query_media("WHERE id = ?", &[(1usize, Value::String(id.to_string()))])
            .map(|mut v| v.pop())
    }

    pub fn get_media_by_hash(&self, hash: &str) -> Result<Option<MediaItem>, String> {
        self.query_media(
            "WHERE sha256_hash = ?",
            &[(1usize, Value::String(hash.to_string()))],
        )
        .map(|mut v| v.pop())
    }

    /// Keyset-paginated timeline query (PRD 11.3: no OFFSET).
    pub fn list_media_timeline(
        &self,
        before_timestamp: Option<i64>,
        limit: i64,
    ) -> Result<Vec<MediaItem>, String> {
        match before_timestamp {
            Some(ts) => self.query_media(
                "WHERE is_trashed = 0 AND date_taken < ? ORDER BY date_taken DESC LIMIT ?",
                &[
                    (1usize, Value::Integer(ts)),
                    (2usize, Value::Integer(limit)),
                ],
            ),
            None => self.query_media(
                "WHERE is_trashed = 0 ORDER BY date_taken DESC LIMIT ?",
                &[(1usize, Value::Integer(limit))],
            ),
        }
    }

    pub fn list_media_by_statuses(&self, statuses: &[&str]) -> Result<Vec<MediaItem>, String> {
        let placeholders = statuses
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "WHERE sync_status IN ({}) ORDER BY date_added ASC",
            placeholders
        );
        let params: Vec<(usize, Value)> = statuses
            .iter()
            .enumerate()
            .map(|(i, s)| (i + 1, Value::String(s.to_string())))
            .collect();
        self.query_media(&sql, &params)
    }

    pub fn list_backed_up_media(&self) -> Result<Vec<MediaItem>, String> {
        self.query_media("WHERE sync_status = 'BACKED_UP'", &[])
    }

    pub fn list_all_media(&self) -> Result<Vec<MediaItem>, String> {
        self.query_media("WHERE 1=1", &[])
    }

    fn query_media(
        &self,
        where_clause: &str,
        params: &[(usize, Value)],
    ) -> Result<Vec<MediaItem>, String> {
        let sql = format!("SELECT * FROM media_items {}", where_clause);
        let mut stmt = self.conn.prepare(&sql).map_err(|e| e.to_string())?;
        stmt.bind(params).map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        while let State::Row = stmt.next().map_err(|e| e.to_string())? {
            out.push(read_media_row(&stmt)?);
        }
        Ok(out)
    }

    pub fn set_media_status(&self, id: &str, status: &str) -> Result<(), String> {
        self.exec(
            "UPDATE media_items SET sync_status = ? WHERE id = ?",
            &[
                (1usize, Value::String(status.to_string())),
                (2usize, Value::String(id.to_string())),
            ],
        )
        .map(|_| ())
    }

    pub fn mark_media_cloud_only(&self, id: &str) -> Result<(), String> {
        self.exec(
            "UPDATE media_items SET sync_status = 'CLOUD_ONLY', file_path = NULL, upload_progress = 100 WHERE id = ?",
            &[(1usize, Value::String(id.to_string()))],
        )
        .map(|_| ())
    }

    pub fn count_media(&self) -> Result<i64, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT COUNT(*) FROM media_items WHERE is_trashed = 0")
            .map_err(|e| e.to_string())?;
        if let State::Row = stmt.next().map_err(|e| e.to_string())? {
            Ok(stmt.read::<i64, _>(0).map_err(|e| e.to_string())?)
        } else {
            Ok(0)
        }
    }

    // ── Albums ──────────────────────────────────────────────────────────────

    pub fn upsert_album(&self, album: &Album) -> Result<(), String> {
        self.exec(
            "INSERT INTO albums (id, name, created_at, cover_media_id, is_pinned, source_type)
             VALUES (?,?,?,?,?,?)
             ON CONFLICT(id) DO UPDATE SET name=excluded.name, cover_media_id=excluded.cover_media_id,
                is_pinned=excluded.is_pinned, source_type=excluded.source_type",
            &[
                (1usize, Value::String(album.id.clone())),
                (2usize, Value::String(album.name.clone())),
                (3usize, Value::Integer(album.created_at)),
                (
                    4usize,
                    album
                        .cover_media_id
                        .clone()
                        .map(Value::String)
                        .unwrap_or(Value::Null),
                ),
                (5usize, Value::Integer(if album.is_pinned { 1 } else { 0 })),
                (6usize, Value::String(album.source_type.clone())),
            ],
        )
        .map(|_| ())
    }

    pub fn get_album(&self, id: &str) -> Result<Option<Album>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM albums WHERE id = ?")
            .map_err(|e| e.to_string())?;
        stmt.bind(&[(1usize, Value::String(id.to_string()))][..])
            .map_err(|e| e.to_string())?;
        match stmt.next().map_err(|e| e.to_string())? {
            State::Row => Ok(Some(read_album_row(&stmt)?)),
            State::Done => Ok(None),
        }
    }

    pub fn list_albums(&self) -> Result<Vec<Album>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM albums ORDER BY created_at DESC")
            .map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        while let State::Row = stmt.next().map_err(|e| e.to_string())? {
            let mut album = read_album_row(&stmt)?;
            album.item_count = self.count_album_items(&album.id)?;
            out.push(album);
        }
        Ok(out)
    }

    pub fn add_media_to_album(&self, album_id: &str, media_id: &str) -> Result<(), String> {
        self.exec(
            "INSERT OR IGNORE INTO album_media_map (album_id, media_id, added_at) VALUES (?,?,?)",
            &[
                (1usize, Value::String(album_id.to_string())),
                (2usize, Value::String(media_id.to_string())),
                (3usize, Value::Integer(chrono::Utc::now().timestamp_millis())),
            ],
        )
        .map(|_| ())
    }

    pub fn count_album_items(&self, album_id: &str) -> Result<i64, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT COUNT(*) FROM album_media_map WHERE album_id = ?")
            .map_err(|e| e.to_string())?;
        stmt.bind(&[(1usize, Value::String(album_id.to_string()))][..])
            .map_err(|e| e.to_string())?;
        if let State::Row = stmt.next().map_err(|e| e.to_string())? {
            Ok(stmt.read::<i64, _>(0).map_err(|e| e.to_string())?)
        } else {
            Ok(0)
        }
    }

    // ── Google import sessions ──────────────────────────────────────────────

    pub fn upsert_google_session(&self, s: &GoogleImportSession) -> Result<(), String> {
        self.exec(
            "INSERT INTO google_import_sessions (
                session_id, google_account_email, started_at, completed_at,
                total_items_found, items_imported_success, items_imported_failed,
                total_bytes_migrated, post_cleanup_choice, cleanup_completed_at, status
             ) VALUES (?,?,?,?,?,?,?,?,?,?,?)
             ON CONFLICT(session_id) DO UPDATE SET
                completed_at=excluded.completed_at,
                total_items_found=excluded.total_items_found,
                items_imported_success=excluded.items_imported_success,
                items_imported_failed=excluded.items_imported_failed,
                total_bytes_migrated=excluded.total_bytes_migrated,
                post_cleanup_choice=excluded.post_cleanup_choice,
                cleanup_completed_at=excluded.cleanup_completed_at,
                status=excluded.status",
            &[
                (1usize, Value::String(s.session_id.clone())),
                (2usize, Value::String(s.google_account_email.clone())),
                (3usize, Value::Integer(s.started_at)),
                (
                    4usize,
                    s.completed_at.map(Value::Integer).unwrap_or(Value::Null),
                ),
                (5usize, Value::Integer(s.total_items_found)),
                (6usize, Value::Integer(s.items_imported_success)),
                (7usize, Value::Integer(s.items_imported_failed)),
                (8usize, Value::Integer(s.total_bytes_migrated)),
                (
                    9usize,
                    s.post_cleanup_choice
                        .clone()
                        .map(Value::String)
                        .unwrap_or(Value::Null),
                ),
                (
                    10usize,
                    s.cleanup_completed_at
                        .map(Value::Integer)
                        .unwrap_or(Value::Null),
                ),
                (11usize, Value::String(s.status.clone())),
            ],
        )
        .map(|_| ())
    }

    pub fn get_google_session(&self, id: &str) -> Result<Option<GoogleImportSession>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM google_import_sessions WHERE session_id = ?")
            .map_err(|e| e.to_string())?;
        stmt.bind(&[(1usize, Value::String(id.to_string()))][..])
            .map_err(|e| e.to_string())?;
        match stmt.next().map_err(|e| e.to_string())? {
            State::Row => Ok(Some(read_google_session_row(&stmt)?)),
            State::Done => Ok(None),
        }
    }

    pub fn list_google_sessions(&self) -> Result<Vec<GoogleImportSession>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM google_import_sessions ORDER BY started_at DESC LIMIT 50")
            .map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        while let State::Row = stmt.next().map_err(|e| e.to_string())? {
            out.push(read_google_session_row(&stmt)?);
        }
        Ok(out)
    }

    // ── Vault ───────────────────────────────────────────────────────────────

    pub fn get_vault_info(&self) -> Result<VaultInfo, String> {
        let mut v = VaultInfo {
            channel_title: "TelegramPhotos_Vault".to_string(),
            is_private: true,
            ..Default::default()
        };
        if let Some(meta) = self.get_vault_meta()? {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&meta) {
                v.channel_id = json.get("channel_id").and_then(|x| x.as_i64());
                if let Some(t) = json.get("channel_title").and_then(|x| x.as_str()) {
                    v.channel_title = t.to_string();
                }
            }
        }
        let mut stmt = self
            .conn
            .prepare(
                "SELECT COUNT(*), COALESCE(SUM(file_size_bytes),0) FROM media_items
                 WHERE sync_status IN ('BACKED_UP','CLOUD_ONLY')",
            )
            .map_err(|e| e.to_string())?;
        if let State::Row = stmt.next().map_err(|e| e.to_string())? {
            v.total_backed_up_files = stmt.read::<i64, _>(0).map_err(|e| e.to_string())?;
            v.total_storage_used_bytes = stmt.read::<i64, _>(1).map_err(|e| e.to_string())?;
        }
        v.last_sync_timestamp = chrono::Utc::now().timestamp_millis();
        Ok(v)
    }

    // ── Uploads (PRD Part 2 §6.2: state machine + resume) ───────────────────

    pub fn upsert_upload(&self, u: &Upload) -> Result<(), String> {
        self.exec(
            "INSERT INTO uploads (id, media_id, message_id, file_id, hash_sha256, status,
                retry_count, last_error, uploaded_bytes, total_bytes, created_at, updated_at)
             VALUES (?,?,?,?,?,?,?,?,?,?,?,?)
             ON CONFLICT(id) DO UPDATE SET
                message_id=excluded.message_id, file_id=excluded.file_id,
                hash_sha256=excluded.hash_sha256, status=excluded.status,
                retry_count=excluded.retry_count, last_error=excluded.last_error,
                uploaded_bytes=excluded.uploaded_bytes, total_bytes=excluded.total_bytes,
                updated_at=excluded.updated_at",
            &[
                (1usize, Value::String(u.id.clone())),
                (2usize, Value::String(u.media_id.clone())),
                (3usize, u.message_id.map(Value::Integer).unwrap_or(Value::Null)),
                (4usize, u.file_id.clone().map(Value::String).unwrap_or(Value::Null)),
                (5usize, u.hash_sha256.clone().map(Value::String).unwrap_or(Value::Null)),
                (6usize, Value::String(u.status.clone())),
                (7usize, Value::Integer(u.retry_count)),
                (8usize, u.last_error.clone().map(Value::String).unwrap_or(Value::Null)),
                (9usize, Value::Integer(u.uploaded_bytes)),
                (10usize, Value::Integer(u.total_bytes)),
                (11usize, Value::Integer(u.created_at)),
                (12usize, Value::Integer(u.updated_at)),
            ],
        )
        .map(|_| ())
    }

    /// Mark a failed/paused upload back to PENDING so the upload manager picks
    /// it up again (PRD Part 2 §6.2: retry = reset state machine).
    pub fn retry_upload(&self, upload_id: &str) -> Result<(), String> {
        self.exec(
            "UPDATE uploads SET status='PENDING', retry_count=retry_count+1,
                last_error=NULL, uploaded_bytes=0, updated_at=? WHERE id=?",
            &[
                (1usize, Value::Integer(chrono::Utc::now().timestamp_millis())),
                (2usize, Value::String(upload_id.to_string())),
            ],
        )
        .map(|_| ())
    }

    pub fn get_upload(&self, media_id: &str) -> Result<Option<Upload>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM uploads WHERE media_id = ?")
            .map_err(|e| e.to_string())?;
        stmt.bind(&[(1usize, Value::String(media_id.to_string()))][..])
            .map_err(|e| e.to_string())?;
        match stmt.next().map_err(|e| e.to_string())? {
            State::Row => Ok(Some(read_upload_row(&stmt)?)),
            State::Done => Ok(None),
        }
    }

    pub fn list_uploads_by_status(&self, status: &str) -> Result<Vec<Upload>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM uploads WHERE status = ? ORDER BY created_at ASC")
            .map_err(|e| e.to_string())?;
        stmt.bind(&[(1usize, Value::String(status.to_string()))][..])
            .map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        while let State::Row = stmt.next().map_err(|e| e.to_string())? {
            out.push(read_upload_row(&stmt)?);
        }
        Ok(out)
    }

    /// Aggregate for the backup banner (G4): count + bytes per status, from a
    /// single indexed GROUP BY instead of JS-side loops.
    pub fn uploads_summary(&self) -> Result<UploadsSummary, String> {
        let mut out = UploadsSummary::default();
        let mut stmt = self
            .conn
            .prepare(
                "SELECT status, COUNT(*), COALESCE(SUM(total_bytes),0)
                 FROM uploads GROUP BY status",
            )
            .map_err(|e| e.to_string())?;
        while let State::Row = stmt.next().map_err(|e| e.to_string())? {
            let status: String = stmt.read(0).map_err(|e| e.to_string())?;
            let count: i64 = stmt.read(1).map_err(|e| e.to_string())?;
            let bytes: i64 = stmt.read(2).map_err(|e| e.to_string())?;
            match status.as_str() {
                "PENDING" | "QUEUED" => {
                    out.queued_count = count;
                    out.queued_bytes = bytes;
                }
                "UPLOADING" => {
                    out.uploading_count = count;
                    out.uploading_bytes = bytes;
                }
                "FAILED" => {
                    out.failed_count = count;
                    out.failed_bytes = bytes;
                }
                "BACKED_UP" => {
                    out.backed_up_count = count;
                    out.backed_up_bytes = bytes;
                }
                _ => {}
            }
        }
        Ok(out)
    }

    pub fn record_upload_error(&self, upload_id: &str, code: &str, message: &str) -> Result<(), String> {
        self.exec(
            "INSERT INTO upload_errors (upload_id, error_code, message, at) VALUES (?,?,?,?)",
            &[
                (1usize, Value::String(upload_id.to_string())),
                (2usize, Value::String(code.to_string())),
                (3usize, Value::String(message.to_string())),
                (4usize, Value::Integer(chrono::Utc::now().timestamp_millis())),
            ],
        )
        .map(|_| ())
    }

    pub fn list_upload_errors(&self, upload_id: &str) -> Result<Vec<UploadError>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM upload_errors WHERE upload_id = ? ORDER BY at DESC LIMIT 20")
            .map_err(|e| e.to_string())?;
        stmt.bind(&[(1usize, Value::String(upload_id.to_string()))][..])
            .map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        while let State::Row = stmt.next().map_err(|e| e.to_string())? {
            out.push(UploadError {
                id: stmt.read::<i64, _>(0).map_err(|e| e.to_string())?,
                upload_id: stmt.read::<String, _>(1).map_err(|e| e.to_string())?,
                error_code: stmt.read::<Option<String>, _>(2).map_err(|e| e.to_string())?,
                message: stmt.read::<String, _>(3).map_err(|e| e.to_string())?,
                at: stmt.read::<i64, _>(4).map_err(|e| e.to_string())?,
            });
        }
        Ok(out)
    }

    // ── Captions & hashtags (PRD Part 2 §6.3, T4) ───────────────────────────

    pub fn upsert_caption(&self, media_id: &str, text: &str) -> Result<(), String> {
        let id = uuid::Uuid::new_v4().to_string();
        self.exec(
            "INSERT INTO captions (id, media_id, text, updated_at) VALUES (?,?,?,?)
             ON CONFLICT(media_id) DO UPDATE SET text=excluded.text, updated_at=excluded.updated_at",
            &[
                (1usize, Value::String(id)),
                (2usize, Value::String(media_id.to_string())),
                (3usize, Value::String(text.to_string())),
                (4usize, Value::Integer(chrono::Utc::now().timestamp_millis())),
            ],
        )
        .map(|_| ())
    }

    pub fn get_caption(&self, media_id: &str) -> Result<Option<String>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT text FROM captions WHERE media_id = ?")
            .map_err(|e| e.to_string())?;
        stmt.bind(&[(1usize, Value::String(media_id.to_string()))][..])
            .map_err(|e| e.to_string())?;
        match stmt.next().map_err(|e| e.to_string())? {
            State::Row => Ok(Some(stmt.read::<String, _>(0).map_err(|e| e.to_string())?)),
            State::Done => Ok(None),
        }
    }

    pub fn add_caption_tag(&self, media_id: &str, tag: &str) -> Result<(), String> {
        let id = uuid::Uuid::new_v4().to_string();
        self.exec(
            "INSERT OR IGNORE INTO caption_tags (id, media_id, tag) VALUES (?,?,?)",
            &[
                (1usize, Value::String(id)),
                (2usize, Value::String(media_id.to_string())),
                (3usize, Value::String(tag.trim_start_matches('#').to_string())),
            ],
        )
        .map(|_| ())
    }

    pub fn search_by_hashtag(&self, tag: &str) -> Result<Vec<String>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT media_id FROM caption_tags WHERE tag = ? ORDER BY rowid DESC",
            )
            .map_err(|e| e.to_string())?;
        stmt.bind(&[(1usize, Value::String(tag.trim_start_matches('#').to_string()))][..])
            .map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        while let State::Row = stmt.next().map_err(|e| e.to_string())? {
            out.push(stmt.read::<String, _>(0).map_err(|e| e.to_string())?);
        }
        Ok(out)
    }

    // ── Collections (PRD Part 2 §6.4, T5) ───────────────────────────────────

    pub fn create_collection(&self, name: &str) -> Result<Collection, String> {
        let c = Collection {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.to_string(),
            cover_media_id: None,
            is_cloud: false,
            sort_order: 0,
            created_at: chrono::Utc::now().timestamp_millis(),
            item_count: 0,
        };
        self.exec(
            "INSERT INTO collections (id, name, cover_media_id, is_cloud, sort_order, created_at)
             VALUES (?,?,?,?,?,?)",
            &[
                (1usize, Value::String(c.id.clone())),
                (2usize, Value::String(c.name.clone())),
                (3usize, Value::Null),
                (4usize, Value::Integer(0)),
                (5usize, Value::Integer(0)),
                (6usize, Value::Integer(c.created_at)),
            ],
        )
        .map(|_| c)
    }

    pub fn list_collections(&self) -> Result<Vec<Collection>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM collections ORDER BY sort_order ASC, created_at DESC")
            .map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        while let State::Row = stmt.next().map_err(|e| e.to_string())? {
            let mut c = Collection {
                id: stmt.read::<String, _>(0).map_err(|e| e.to_string())?,
                name: stmt.read::<String, _>(1).map_err(|e| e.to_string())?,
                cover_media_id: stmt.read::<Option<String>, _>(2).map_err(|e| e.to_string())?,
                is_cloud: stmt.read::<i64, _>(3).map_err(|e| e.to_string())? != 0,
                sort_order: stmt.read::<i64, _>(4).map_err(|e| e.to_string())?,
                created_at: stmt.read::<i64, _>(5).map_err(|e| e.to_string())?,
                item_count: 0,
            };
            c.item_count = self.count_collection_items(&c.id)?;
            out.push(c);
        }
        Ok(out)
    }

    pub fn add_to_collection(&self, collection_id: &str, media_id: &str) -> Result<(), String> {
        self.exec(
            "INSERT OR IGNORE INTO collection_items (collection_id, media_id, added_at) VALUES (?,?,?)",
            &[
                (1usize, Value::String(collection_id.to_string())),
                (2usize, Value::String(media_id.to_string())),
                (3usize, Value::Integer(chrono::Utc::now().timestamp_millis())),
            ],
        )
        .map(|_| ())
    }

    pub fn remove_from_collection(&self, collection_id: &str, media_id: &str) -> Result<(), String> {
        self.exec(
            "DELETE FROM collection_items WHERE collection_id = ? AND media_id = ?",
            &[
                (1usize, Value::String(collection_id.to_string())),
                (2usize, Value::String(media_id.to_string())),
            ],
        )
        .map(|_| ())
    }

    pub fn list_collection_items(&self, collection_id: &str) -> Result<Vec<MediaItem>, String> {
        self.query_media(
            "WHERE is_trashed = 0 AND id IN (SELECT media_id FROM collection_items WHERE collection_id = ?)
             ORDER BY date_taken DESC",
            &[(1usize, Value::String(collection_id.to_string()))],
        )
    }

    pub fn count_collection_items(&self, collection_id: &str) -> Result<i64, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT COUNT(*) FROM collection_items WHERE collection_id = ?")
            .map_err(|e| e.to_string())?;
        stmt.bind(&[(1usize, Value::String(collection_id.to_string()))][..])
            .map_err(|e| e.to_string())?;
        if let State::Row = stmt.next().map_err(|e| e.to_string())? {
            Ok(stmt.read::<i64, _>(0).map_err(|e| e.to_string())?)
        } else {
            Ok(0)
        }
    }

    // ── Thumbnail status (G1) ───────────────────────────────────────────────

    pub fn set_thumb_status(&self, media_id: &str, status: &str) -> Result<(), String> {
        self.exec(
            "UPDATE media_items SET thumb_status = ? WHERE id = ?",
            &[
                (1usize, Value::String(status.to_string())),
                (2usize, Value::String(media_id.to_string())),
            ],
        )
        .map(|_| ())
    }

    /// Stores a generated thumbnail path and marks it CACHED (G1).
    pub fn set_thumbnail_path(&self, media_id: &str, path: &str) -> Result<(), String> {
        self.exec(
            "UPDATE media_items SET thumbnail_path = ?, thumb_status = 'CACHED' WHERE id = ?",
            &[
                (1usize, Value::String(path.to_string())),
                (2usize, Value::String(media_id.to_string())),
            ],
        )
        .map(|_| ())
    }

    /// List media ids whose thumbnail is not yet cached (G1 lazy generation:
    /// only request what is missing).
    pub fn list_media_without_thumb(&self, limit: i64) -> Result<Vec<String>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT id FROM media_items WHERE thumb_status IS NULL OR thumb_status != 'CACHED' LIMIT ?")
            .map_err(|e| e.to_string())?;
        stmt.bind(&[(1usize, Value::Integer(limit))][..])
            .map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        while let State::Row = stmt.next().map_err(|e| e.to_string())? {
            out.push(stmt.read::<String, _>(0).map_err(|e| e.to_string())?);
        }
        Ok(out)
    }

    // ── Aggregation queries (avoid loading all rows into Dart) ─────────────

    /// SQL-side search by file name, caption text, or caption tag.
    /// Returns at most `limit` matching MediaItems.
    pub fn search_media(&self, query: &str, limit: i64) -> Result<Vec<MediaItem>, String> {
        let pattern = format!("%{}%", query);
        self.query_media(
            "WHERE is_trashed = 0 AND (
                file_name LIKE ?1
                OR id IN (SELECT media_id FROM captions WHERE text LIKE ?1)
                OR id IN (SELECT media_id FROM caption_tags WHERE tag LIKE ?1)
            ) ORDER BY date_taken DESC LIMIT ?2",
            &[
                (1usize, Value::String(pattern)),
                (2usize, Value::Integer(limit)),
            ],
        )
    }

    /// Sum file_size_bytes + count for backed-up media (Free Up Space screen).
    /// Returns (total_bytes, total_count).
    pub fn sum_reclaimable_space(&self) -> Result<(i64, i64), String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT COALESCE(SUM(file_size_bytes), 0), COUNT(*)
                 FROM media_items WHERE sync_status = 'BACKED_UP' AND is_trashed = 0",
            )
            .map_err(|e| e.to_string())?;
        if let State::Row = stmt.next().map_err(|e| e.to_string())? {
            let bytes: i64 = stmt.read(0).map_err(|e| e.to_string())?;
            let count: i64 = stmt.read(1).map_err(|e| e.to_string())?;
            Ok((bytes, count))
        } else {
            Ok((0, 0))
        }
    }

    /// List media filtered by sync status (Upload screen: BACKED_UP / FAILED).
    pub fn list_media_by_status(&self, status: &str, limit: i64) -> Result<Vec<MediaItem>, String> {
        self.query_media(
            "WHERE sync_status = ?1 AND is_trashed = 0 ORDER BY date_added DESC LIMIT ?2",
            &[
                (1usize, Value::String(status.to_string())),
                (2usize, Value::Integer(limit)),
            ],
        )
    }
}

fn read_upload_row(stmt: &Statement) -> Result<Upload, String> {
    Ok(Upload {
        id: stmt.read::<String, _>(0).map_err(|e| e.to_string())?,
        media_id: stmt.read::<String, _>(1).map_err(|e| e.to_string())?,
        message_id: stmt.read::<Option<i64>, _>(2).map_err(|e| e.to_string())?,
        file_id: stmt.read::<Option<String>, _>(3).map_err(|e| e.to_string())?,
        hash_sha256: stmt.read::<Option<String>, _>(4).map_err(|e| e.to_string())?,
        status: stmt.read::<String, _>(5).map_err(|e| e.to_string())?,
        retry_count: stmt.read::<i64, _>(6).map_err(|e| e.to_string())?,
        last_error: stmt.read::<Option<String>, _>(7).map_err(|e| e.to_string())?,
        uploaded_bytes: stmt.read::<i64, _>(8).map_err(|e| e.to_string())?,
        total_bytes: stmt.read::<i64, _>(9).map_err(|e| e.to_string())?,
        created_at: stmt.read::<i64, _>(10).map_err(|e| e.to_string())?,
        updated_at: stmt.read::<i64, _>(11).map_err(|e| e.to_string())?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// v1 database (old schema, user_version=1, one media row) must migrate to
    /// v2 without losing data: new tables exist, thumb_status column added.
    #[test]
    fn migrate_v1_to_v2_keeps_data() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("telegram_photos.db");

        // Simulate a v1 database: create the media_items table + one row.
        {
            let conn = sqlite::Connection::open(&path).unwrap();
            conn.execute(
                "CREATE TABLE media_items (
                    id TEXT PRIMARY KEY, local_identifier TEXT UNIQUE,
                    file_name TEXT NOT NULL, file_path TEXT, mime_type TEXT NOT NULL,
                    media_type TEXT NOT NULL DEFAULT 'image', file_size_bytes INTEGER NOT NULL DEFAULT 0,
                    sha256_hash TEXT NOT NULL, date_taken INTEGER NOT NULL, date_added INTEGER NOT NULL,
                    width INTEGER, height INTEGER, orientation INTEGER DEFAULT 0, duration_ms INTEGER DEFAULT 0,
                    camera_make TEXT, camera_model TEXT, focal_length REAL, aperture REAL, iso INTEGER,
                    exposure_time TEXT, latitude REAL, longitude REAL, geo_city TEXT, geo_country TEXT,
                    sync_status TEXT NOT NULL DEFAULT 'NOT_BACKED_UP', upload_progress INTEGER DEFAULT 0,
                    error_message TEXT, tg_channel_id INTEGER, tg_message_id INTEGER, tg_file_id TEXT,
                    tg_access_hash INTEGER, imported_from_google_photos INTEGER DEFAULT 0,
                    google_photos_media_id TEXT, google_cleanup_status TEXT DEFAULT 'NONE',
                    thumbnail_path TEXT, preview_path TEXT, blur_hash TEXT, is_favorite INTEGER DEFAULT 0,
                    is_archived INTEGER DEFAULT 0, is_trashed INTEGER DEFAULT 0, trashed_timestamp INTEGER,
                    is_encrypted INTEGER DEFAULT 0, album_ids TEXT DEFAULT '[]', device_folder TEXT
                );",
            )
            .unwrap();
            conn.execute(
                "INSERT INTO media_items (id, file_name, mime_type, sha256_hash, date_taken, date_added)
                 VALUES ('m1', 'a.jpg', 'image/jpeg', 'h1', 1000, 1000)",
            )
            .unwrap();
            conn.execute("PRAGMA user_version = 1").unwrap();
        }

        // Open via Db (runs migration).
        let db = Db::open(&path).unwrap();
        assert_eq!(db.user_version().unwrap(), 2);
        assert_eq!(db.count_media().unwrap(), 1, "existing row must survive");

        // v2 tables work.
        let c = db.create_collection("Trip").unwrap();
        db.add_to_collection(&c.id, "m1").unwrap();
        assert_eq!(db.count_collection_items(&c.id).unwrap(), 1);

        db.upsert_caption("m1", "sunset #beach").unwrap();
        db.add_caption_tag("m1", "beach").unwrap();
        assert_eq!(db.search_by_hashtag("beach").unwrap(), vec!["m1".to_string()]);

        let up = Upload {
            id: "u1".into(),
            media_id: "m1".into(),
            message_id: None,
            file_id: None,
            hash_sha256: Some("h1".into()),
            status: "QUEUED".into(),
            retry_count: 0,
            last_error: None,
            uploaded_bytes: 0,
            total_bytes: 1000,
            created_at: 1000,
            updated_at: 1000,
        };
        db.upsert_upload(&up).unwrap();
        assert_eq!(db.list_uploads_by_status("QUEUED").unwrap().len(), 1);
        let summary = db.uploads_summary().unwrap();
        assert_eq!(summary.queued_count, 1);

        // thumb_status column exists (G1) and is writable.
        db.set_thumb_status("m1", "CACHED").unwrap();
    }

    /// The JSON emitted by the native MediaStore scanner (MethodChannel) must
    /// deserialize into MediaItem. The struct uses `rename_all = "camelCase"`
    /// (the same contract as the old Tauri frontend), so the scanner emits
    /// camelCase keys with scan-time defaults for required fields.
    #[test]
    fn scan_json_parses_into_media_items() {
        let json = r#"[{"id":"content://media/external/images/media_1","localIdentifier":"1","fileName":"IMG_1.jpg","mimeType":"image/jpeg","mediaType":"image","fileSizeBytes":1024,"dateTaken":1700000000000,"dateAdded":1700000000000,"width":800,"height":600,"durationMs":null,"deviceFolder":"DCIM","latitude":null,"longitude":null,"filePath":null,"sha256Hash":"","syncStatus":"NOT_BACKED_UP","importedFromGooglePhotos":false,"isFavorite":false,"isArchived":false,"isTrashed":false,"isEncrypted":false,"albumIds":[]}]"#;
        let items: Vec<MediaItem> = serde_json::from_str(json)
            .map_err(|e| panic!("scan JSON failed to parse: {e}"))
            .unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].file_name, "IMG_1.jpg");
        assert_eq!(items[0].sync_status, "NOT_BACKED_UP");
        assert_eq!(items[0].album_ids.len(), 0);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Row readers
// ─────────────────────────────────────────────────────────────────────────────

fn read_media_row(stmt: &Statement) -> Result<MediaItem, String> {
    let read_i = |idx: usize| -> Result<Option<i64>, String> {
        stmt.read::<Option<i64>, _>(idx).map_err(|e| e.to_string())
    };
    let read_f = |idx: usize| -> Result<Option<f64>, String> {
        stmt.read::<Option<f64>, _>(idx).map_err(|e| e.to_string())
    };
    let read_s = |idx: usize| -> Result<Option<String>, String> {
        stmt.read::<Option<String>, _>(idx).map_err(|e| e.to_string())
    };
    let read_b = |idx: usize| -> Result<bool, String> {
        Ok(stmt.read::<i64, _>(idx).map_err(|e| e.to_string())? != 0)
    };

    let album_ids_raw: String = stmt.read::<String, _>(42).unwrap_or_else(|_| "[]".into());
    let album_ids: Vec<String> = serde_json::from_str(&album_ids_raw).unwrap_or_default();

    Ok(MediaItem {
        id: stmt.read::<String, _>(0).map_err(|e| e.to_string())?,
        local_identifier: read_s(1)?,
        file_name: stmt.read::<String, _>(2).map_err(|e| e.to_string())?,
        file_path: read_s(3)?,
        mime_type: stmt.read::<String, _>(4).map_err(|e| e.to_string())?,
        media_type: stmt.read::<String, _>(5).map_err(|e| e.to_string())?,
        file_size_bytes: stmt.read::<i64, _>(6).map_err(|e| e.to_string())?,
        sha256_hash: stmt.read::<String, _>(7).map_err(|e| e.to_string())?,
        date_taken: stmt.read::<i64, _>(8).map_err(|e| e.to_string())?,
        date_added: stmt.read::<i64, _>(9).map_err(|e| e.to_string())?,
        width: read_i(10)?,
        height: read_i(11)?,
        orientation: read_i(12)?,
        duration_ms: read_i(13)?,
        camera_make: read_s(14)?,
        camera_model: read_s(15)?,
        focal_length: read_f(16)?,
        aperture: read_f(17)?,
        iso: read_i(18)?,
        exposure_time: read_s(19)?,
        latitude: read_f(20)?,
        longitude: read_f(21)?,
        geo_city: read_s(22)?,
        geo_country: read_s(23)?,
        sync_status: stmt.read::<String, _>(24).map_err(|e| e.to_string())?,
        upload_progress: read_i(25)?,
        error_message: read_s(26)?,
        tg_channel_id: read_i(27)?,
        tg_message_id: read_i(28)?,
        tg_file_id: read_s(29)?,
        tg_access_hash: read_i(30)?,
        imported_from_google_photos: read_b(31)?,
        google_photos_media_id: read_s(32)?,
        google_cleanup_status: read_s(33)?,
        thumbnail_path: read_s(34)?,
        preview_path: read_s(35)?,
        blur_hash: read_s(36)?,
        is_favorite: read_b(37)?,
        is_archived: read_b(38)?,
        is_trashed: read_b(39)?,
        trashed_timestamp: read_i(40)?,
        is_encrypted: read_b(41)?,
        album_ids,
        device_folder: read_s(43)?,
    })
}

fn read_album_row(stmt: &Statement) -> Result<Album, String> {
    Ok(Album {
        id: stmt.read::<String, _>(0).map_err(|e| e.to_string())?,
        name: stmt.read::<String, _>(1).map_err(|e| e.to_string())?,
        created_at: stmt.read::<i64, _>(2).map_err(|e| e.to_string())?,
        cover_media_id: stmt
            .read::<Option<String>, _>(3)
            .map_err(|e| e.to_string())?,
        is_pinned: stmt.read::<i64, _>(4).map_err(|e| e.to_string())? != 0,
        source_type: stmt.read::<String, _>(5).map_err(|e| e.to_string())?,
        item_count: 0,
    })
}

fn read_google_session_row(stmt: &Statement) -> Result<GoogleImportSession, String> {
    Ok(GoogleImportSession {
        session_id: stmt.read::<String, _>(0).map_err(|e| e.to_string())?,
        google_account_email: stmt.read::<String, _>(1).map_err(|e| e.to_string())?,
        started_at: stmt.read::<i64, _>(2).map_err(|e| e.to_string())?,
        completed_at: stmt.read::<Option<i64>, _>(3).map_err(|e| e.to_string())?,
        total_items_found: stmt.read::<i64, _>(4).map_err(|e| e.to_string())?,
        items_imported_success: stmt.read::<i64, _>(5).map_err(|e| e.to_string())?,
        items_imported_failed: stmt.read::<i64, _>(6).map_err(|e| e.to_string())?,
        total_bytes_migrated: stmt.read::<i64, _>(7).map_err(|e| e.to_string())?,
        post_cleanup_choice: stmt
            .read::<Option<String>, _>(8)
            .map_err(|e| e.to_string())?,
        cleanup_completed_at: stmt
            .read::<Option<i64>, _>(9)
            .map_err(|e| e.to_string())?,
        status: stmt.read::<String, _>(10).map_err(|e| e.to_string())?,
        current_speed_mbps: None,
        eta_seconds: None,
    })
}
