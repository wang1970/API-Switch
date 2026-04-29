pub mod dao;
mod schema;

use crate::error::AppError;
use chrono::Utc;
use rusqlite::Connection;
use std::{path::Path, sync::Mutex};

/// Macro to safely lock the database connection
macro_rules! lock_conn {
    ($mutex:expr) => {
        $mutex
            .lock()
            .map_err(|e| AppError::Database(format!("Mutex lock failed: {}", e)))?
    };
}

pub(crate) use lock_conn;

pub use dao::*;

/// Database connection wrapper
pub struct Database {
    pub(crate) conn: Mutex<Connection>,
}

impl Database {
    /// Open database next to the executable (portable mode)
    pub fn open() -> Result<Self, AppError> {
        let exe_dir = std::env::current_exe()
            .map_err(|e| AppError::Database(format!("Failed to get exe path: {e}")))?
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::path::PathBuf::from("."));

        let db_path = exe_dir.join("api-switch.db");
        let conn = open_or_recover(&db_path)?;

        // Enable WAL mode for better concurrent read performance
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .map_err(|e| AppError::Database(format!("Failed to set pragmas: {e}")))?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Create all tables
    pub fn create_tables(&self) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);
        schema::create_tables(&conn)
    }
}

fn open_or_recover(db_path: &Path) -> Result<Connection, AppError> {
    let conn = Connection::open(db_path)
        .map_err(|e| AppError::Database(format!("Failed to open db: {e}")))?;

    if !is_database_healthy(&conn) {
        drop(conn);
        backup_corrupt_database(db_path)?;
        return Connection::open(db_path)
            .map_err(|e| AppError::Database(format!("Failed to recreate db: {e}")));
    }

    Ok(conn)
}

fn is_database_healthy(conn: &Connection) -> bool {
    conn.query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
        .map(|result| result.eq_ignore_ascii_case("ok"))
        .unwrap_or(false)
}

fn backup_corrupt_database(db_path: &Path) -> Result<(), AppError> {
    if !db_path.exists() {
        return Ok(());
    }

    let timestamp = Utc::now().format("%Y%m%d%H%M%S");
    let backup_path = db_path.with_file_name(format!("api-switch.corrupt.{timestamp}.db"));

    std::fs::rename(db_path, &backup_path).map_err(|e| {
        AppError::Database(format!(
            "Database is corrupted and failed to move it to {}: {e}",
            backup_path.display()
        ))
    })?;

    log::warn!(
        "Database was corrupted and has been moved to {}. A new database will be created.",
        backup_path.display()
    );

    for suffix in ["-wal", "-shm"] {
        let file_name = db_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("api-switch.db");
        let sidecar_path = db_path.with_file_name(format!("{file_name}{suffix}"));
        if sidecar_path.exists() {
            let backup_file_name = backup_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("api-switch.corrupt.db");
            let sidecar_backup_path = backup_path.with_file_name(format!("{backup_file_name}{suffix}"));
            let _ = std::fs::rename(sidecar_path, sidecar_backup_path);
        }
    }

    Ok(())
}

