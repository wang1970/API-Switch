use crate::database::{lock_conn, Database};
use crate::error::AppError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub proxy_enabled: bool,
    pub listen_port: i32,
    pub access_key_required: bool,
    pub circuit_failure_threshold: i32,
    pub circuit_recovery_secs: i64,
    pub locale: String,
    pub theme: String,
    pub autostart: bool,
    pub start_minimized: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            proxy_enabled: false,
            listen_port: 9090,
            access_key_required: false,
            circuit_failure_threshold: 4,
            circuit_recovery_secs: 60,
            locale: "zh".to_string(),
            theme: "system".to_string(),
            autostart: false,
            start_minimized: false,
        }
    }
}

impl Database {
    pub fn get_settings(&self) -> Result<AppSettings, AppError> {
        let conn = lock_conn!(self.conn);
        let mut stmt = conn.prepare("SELECT key, value FROM config")?;

        let kv: std::collections::HashMap<String, String> = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .filter_map(|r| r.ok())
            .collect();

        let mut settings = AppSettings::default();

        if let Some(v) = kv.get("proxy_enabled") {
            settings.proxy_enabled = v == "1";
        }
        if let Some(v) = kv.get("listen_port") {
            settings.listen_port = v.parse().unwrap_or(9090);
        }
        if let Some(v) = kv.get("access_key_required") {
            settings.access_key_required = v == "1";
        }
        if let Some(v) = kv.get("circuit_failure_threshold") {
            settings.circuit_failure_threshold = v.parse().unwrap_or(4);
        }
        if let Some(v) = kv.get("circuit_recovery_secs") {
            settings.circuit_recovery_secs = v.parse().unwrap_or(60);
        }
        if let Some(v) = kv.get("locale") {
            settings.locale = v.clone();
        }
        if let Some(v) = kv.get("theme") {
            settings.theme = v.clone();
        }
        if let Some(v) = kv.get("autostart") {
            settings.autostart = v == "1";
        }
        if let Some(v) = kv.get("start_minimized") {
            settings.start_minimized = v == "1";
        }

        Ok(settings)
    }

    pub fn update_settings(&self, updates: &AppSettings) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);

        let kv = [
            (
                "proxy_enabled",
                if updates.proxy_enabled { "1" } else { "0" },
            ),
            ("listen_port", &updates.listen_port.to_string()),
            (
                "access_key_required",
                if updates.access_key_required {
                    "1"
                } else {
                    "0"
                },
            ),
            (
                "circuit_failure_threshold",
                &updates.circuit_failure_threshold.to_string(),
            ),
            (
                "circuit_recovery_secs",
                &updates.circuit_recovery_secs.to_string(),
            ),
            ("locale", &updates.locale),
            ("theme", &updates.theme),
            ("autostart", if updates.autostart { "1" } else { "0" }),
            (
                "start_minimized",
                if updates.start_minimized { "1" } else { "0" },
            ),
        ];

        for (key, value) in kv {
            conn.execute(
                "INSERT OR REPLACE INTO config (key, value) VALUES (?1, ?2)",
                rusqlite::params![key, value],
            )?;
        }

        Ok(())
    }

    pub fn get_config_value(&self, key: &str) -> Result<Option<String>, AppError> {
        let conn = lock_conn!(self.conn);
        let result = conn.query_row("SELECT value FROM config WHERE key = ?1", [key], |row| {
            row.get(0)
        });

        match result {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(AppError::Database(e.to_string())),
        }
    }

    pub fn set_config_value(&self, key: &str, value: &str) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);
        conn.execute(
            "INSERT OR REPLACE INTO config (key, value) VALUES (?1, ?2)",
            rusqlite::params![key, value],
        )?;
        Ok(())
    }
}
