pub mod dao;
mod schema;

use crate::error::AppError;
use rusqlite::Connection;
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
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
    /// Open the database in portable mode when possible and fall back to the
    /// per-user application data directory for packaged installs.
    pub fn open() -> Result<Self, AppError> {
        let exe_path = std::env::current_exe()
            .map_err(|e| AppError::Database(format!("Failed to get exe path: {e}")))?
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from("."));

        let db_dir = resolve_db_dir(&exe_path)?;
        fs::create_dir_all(&db_dir)
            .map_err(|e| AppError::Database(format!("Failed to create db dir: {e}")))?;

        let db_path = db_dir.join("api-switch.db");
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

fn resolve_db_dir(exe_path: &Path) -> Result<PathBuf, AppError> {
    if should_use_portable_dir(exe_path) {
        if let Some(exe_dir) = exe_path.parent() {
            if is_dir_writable(exe_dir) {
                return Ok(exe_dir.to_path_buf());
            }
        }
    }

    platform_data_dir()
}

fn should_use_portable_dir(exe_path: &Path) -> bool {
    #[cfg(target_os = "macos")]
    {
        if is_macos_app_bundle_executable(exe_path) {
            return false;
        }
    }

    #[cfg(target_os = "linux")]
    {
        if std::env::var_os("APPIMAGE").is_some() {
            return false;
        }
    }

    true
}

fn is_dir_writable(dir: &Path) -> bool {
    if !dir.is_dir() {
        return false;
    }

    let probe = dir.join(".api-switch-write-test");
    match OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&probe)
    {
        Ok(_) => {
            let _ = fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

#[cfg(target_os = "macos")]
fn is_macos_app_bundle_executable(exe_path: &Path) -> bool {
    matches!(
        (
            exe_path.parent().and_then(Path::file_name),
            exe_path.parent().and_then(Path::parent).and_then(Path::file_name),
            exe_path
                .parent()
                .and_then(Path::parent)
                .and_then(Path::parent)
                .and_then(Path::extension),
        ),
        (Some(macos), Some(contents), Some(app_ext))
            if macos == "MacOS" && contents == "Contents" && app_ext == "app"
    )
}

fn platform_data_dir() -> Result<PathBuf, AppError> {
    let base = platform_data_base_dir()
        .ok_or_else(|| AppError::Database("Failed to resolve app data directory".into()))?;

    Ok(base.join("API Switch"))
}

fn platform_data_base_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA").map(PathBuf::from)
    }

    #[cfg(target_os = "macos")]
    {
        home_dir().map(|dir| dir.join("Library").join("Application Support"))
    }

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| home_dir().map(|dir| dir.join(".local").join("share")))
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}
