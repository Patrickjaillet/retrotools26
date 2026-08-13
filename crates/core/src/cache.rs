use crate::hash::{FileHashes, HashResult};
use crate::header::{HeaderInfo, RomHeaderKind};
use crate::model::GameSet;
use retrotools_common::error::{AppError, AppResult};
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;

pub struct DatCache {
    conn: Connection,
}

impl DatCache {
    pub fn open(path: &Path) -> AppResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(AppError::Io)?;
        }
        let conn = Connection::open(path)
            .map_err(|e| AppError::DatParsing(format!("cannot open DAT cache: {e}")))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS dat_cache (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                platform TEXT NOT NULL,
                dat_name TEXT NOT NULL,
                dat_version TEXT NOT NULL,
                dat_type TEXT NOT NULL,
                source_path TEXT NOT NULL,
                content_json TEXT NOT NULL,
                imported_at TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(platform, dat_name, dat_version)
            );",
        )
        .map_err(|e| AppError::DatParsing(format!("cannot initialize DAT cache schema: {e}")))?;
        Ok(Self { conn })
    }

    pub fn open_in_memory() -> AppResult<Self> {
        let conn = Connection::open_in_memory()
            .map_err(|e| AppError::DatParsing(format!("cannot open in-memory cache: {e}")))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS dat_cache (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                platform TEXT NOT NULL,
                dat_name TEXT NOT NULL,
                dat_version TEXT NOT NULL,
                dat_type TEXT NOT NULL,
                source_path TEXT NOT NULL,
                content_json TEXT NOT NULL,
                imported_at TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(platform, dat_name, dat_version)
            );",
        )
        .map_err(|e| AppError::DatParsing(format!("cannot initialize DAT cache schema: {e}")))?;
        Ok(Self { conn })
    }

    pub fn store(&self, source_path: &Path, gameset: &GameSet) -> AppResult<()> {
        let content_json = serde_json::to_string(gameset).map_err(AppError::Json)?;
        self.conn
            .execute(
                "INSERT INTO dat_cache (platform, dat_name, dat_version, dat_type, source_path, content_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(platform, dat_name, dat_version)
                 DO UPDATE SET source_path = excluded.source_path,
                               content_json = excluded.content_json,
                               imported_at = datetime('now')",
                params![
                    gameset.platform,
                    gameset.dat_name,
                    gameset.dat_version,
                    gameset.dat_type.to_string(),
                    source_path.to_string_lossy(),
                    content_json,
                ],
            )
            .map_err(|e| AppError::DatParsing(format!("cannot store DAT in cache: {e}")))?;
        Ok(())
    }

    pub fn load_all(&self) -> AppResult<Vec<GameSet>> {
        let mut stmt = self
            .conn
            .prepare("SELECT content_json FROM dat_cache ORDER BY platform ASC")
            .map_err(|e| AppError::DatParsing(format!("cannot query cache: {e}")))?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| AppError::DatParsing(format!("cannot read cache rows: {e}")))?;

        let mut result = Vec::new();
        for row in rows {
            let json = row.map_err(|e| AppError::DatParsing(format!("cache row error: {e}")))?;
            let gameset: GameSet = serde_json::from_str(&json).map_err(AppError::Json)?;
            result.push(gameset);
        }
        Ok(result)
    }

    pub fn load_by_platform(&self, platform: &str) -> AppResult<Option<GameSet>> {
        let mut stmt = self
            .conn
            .prepare("SELECT content_json FROM dat_cache WHERE platform = ?1 LIMIT 1")
            .map_err(|e| AppError::DatParsing(format!("cannot query cache: {e}")))?;
        let mut rows = stmt
            .query(params![platform])
            .map_err(|e| AppError::DatParsing(format!("cannot read cache rows: {e}")))?;

        if let Some(row) = rows
            .next()
            .map_err(|e| AppError::DatParsing(format!("cache row error: {e}")))?
        {
            let json: String = row
                .get(0)
                .map_err(|e| AppError::DatParsing(format!("cache column error: {e}")))?;
            let gameset: GameSet = serde_json::from_str(&json).map_err(AppError::Json)?;
            Ok(Some(gameset))
        } else {
            Ok(None)
        }
    }

    pub fn clear(&self) -> AppResult<()> {
        self.conn
            .execute("DELETE FROM dat_cache", [])
            .map_err(|e| AppError::DatParsing(format!("cannot clear cache: {e}")))?;
        Ok(())
    }
}

/// Incremental scan cache: remembers hashes already computed for a given
/// (path, archive entry, mtime, size) tuple so unchanged files are not re-hashed.
pub struct ScanCache {
    conn: Mutex<Connection>,
}

const SCAN_CACHE_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS scan_hash_cache (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL,
    archive_entry TEXT,
    mtime INTEGER NOT NULL,
    size INTEGER NOT NULL,
    header_kind TEXT NOT NULL,
    header_size INTEGER NOT NULL,
    full_json TEXT NOT NULL,
    headerless_json TEXT,
    UNIQUE(path, archive_entry)
);";

fn header_kind_to_str(kind: RomHeaderKind) -> &'static str {
    match kind {
        RomHeaderKind::None => "none",
        RomHeaderKind::INes => "ines",
        RomHeaderKind::Lynx => "lynx",
        RomHeaderKind::Atari7800 => "a78",
    }
}

fn header_kind_from_str(s: &str) -> RomHeaderKind {
    match s {
        "ines" => RomHeaderKind::INes,
        "lynx" => RomHeaderKind::Lynx,
        "a78" => RomHeaderKind::Atari7800,
        _ => RomHeaderKind::None,
    }
}

impl ScanCache {
    pub fn open(path: &Path) -> AppResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(AppError::Io)?;
        }
        let conn = Connection::open(path)
            .map_err(|e| AppError::Scan(format!("cannot open scan cache: {e}")))?;
        conn.execute_batch(SCAN_CACHE_SCHEMA)
            .map_err(|e| AppError::Scan(format!("cannot initialize scan cache schema: {e}")))?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn open_in_memory() -> AppResult<Self> {
        let conn = Connection::open_in_memory()
            .map_err(|e| AppError::Scan(format!("cannot open in-memory scan cache: {e}")))?;
        conn.execute_batch(SCAN_CACHE_SCHEMA)
            .map_err(|e| AppError::Scan(format!("cannot initialize scan cache schema: {e}")))?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn get(
        &self,
        path: &str,
        archive_entry: Option<&str>,
        mtime: i64,
        size: u64,
    ) -> AppResult<Option<HashResult>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::Scan("scan cache mutex poisoned".into()))?;
        let mut stmt = conn
            .prepare(
                "SELECT header_kind, header_size, full_json, headerless_json
                 FROM scan_hash_cache
                 WHERE path = ?1 AND archive_entry IS ?2 AND mtime = ?3 AND size = ?4",
            )
            .map_err(|e| AppError::Scan(format!("cannot query scan cache: {e}")))?;

        let mut rows = stmt
            .query(params![path, archive_entry, mtime, size as i64])
            .map_err(|e| AppError::Scan(format!("cannot read scan cache rows: {e}")))?;

        if let Some(row) = rows
            .next()
            .map_err(|e| AppError::Scan(format!("scan cache row error: {e}")))?
        {
            let header_kind: String = row
                .get(0)
                .map_err(|e| AppError::Scan(format!("scan cache column error: {e}")))?;
            let header_size: i64 = row
                .get(1)
                .map_err(|e| AppError::Scan(format!("scan cache column error: {e}")))?;
            let full_json: String = row
                .get(2)
                .map_err(|e| AppError::Scan(format!("scan cache column error: {e}")))?;
            let headerless_json: Option<String> = row
                .get(3)
                .map_err(|e| AppError::Scan(format!("scan cache column error: {e}")))?;

            let full: FileHashes = serde_json::from_str(&full_json).map_err(AppError::Json)?;
            let headerless = headerless_json
                .map(|j| serde_json::from_str::<FileHashes>(&j))
                .transpose()
                .map_err(AppError::Json)?;

            Ok(Some(HashResult {
                full,
                headerless,
                header: HeaderInfo {
                    kind: header_kind_from_str(&header_kind),
                    header_size: header_size as usize,
                },
            }))
        } else {
            Ok(None)
        }
    }

    pub fn put(
        &self,
        path: &str,
        archive_entry: Option<&str>,
        mtime: i64,
        size: u64,
        result: &HashResult,
    ) -> AppResult<()> {
        let full_json = serde_json::to_string(&result.full).map_err(AppError::Json)?;
        let headerless_json = result
            .headerless
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(AppError::Json)?;

        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::Scan("scan cache mutex poisoned".into()))?;
        conn.execute(
                "INSERT INTO scan_hash_cache (path, archive_entry, mtime, size, header_kind, header_size, full_json, headerless_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(path, archive_entry)
                 DO UPDATE SET mtime = excluded.mtime,
                               size = excluded.size,
                               header_kind = excluded.header_kind,
                               header_size = excluded.header_size,
                               full_json = excluded.full_json,
                               headerless_json = excluded.headerless_json",
                params![
                    path,
                    archive_entry,
                    mtime,
                    size as i64,
                    header_kind_to_str(result.header.kind),
                    result.header.header_size as i64,
                    full_json,
                    headerless_json,
                ],
            )
            .map_err(|e| AppError::Scan(format!("cannot store scan cache entry: {e}")))?;
        Ok(())
    }

    pub fn clear(&self) -> AppResult<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::Scan("scan cache mutex poisoned".into()))?;
        conn.execute("DELETE FROM scan_hash_cache", [])
            .map_err(|e| AppError::Scan(format!("cannot clear scan cache: {e}")))?;
        Ok(())
    }
}
