pub mod dao;
mod schema;

use crate::error::AppError;
use rusqlite::Connection;
use std::sync::Mutex;

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
        let conn = Connection::open(&db_path)
            .map_err(|e| AppError::Database(format!("Failed to open db: {e}")))?;

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
