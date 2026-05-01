use crate::database::{lock_conn, Database};
use crate::error::AppError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub proxy_enabled: bool,
    pub listen_port: i32,
    pub access_key_required: bool,
    pub circuit_failure_threshold: i32,
    pub circuit_recovery_secs: i64,
    pub circuit_disable_codes: String,
    pub circuit_retry_codes: String,
    pub disable_keywords: String,
    pub locale: String,
    pub theme: String,
    pub autostart: bool,
    pub start_minimized: bool,
    pub show_guide: bool,
    pub default_sort_mode: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            proxy_enabled: true,
            listen_port: 9090,
            access_key_required: false,
            circuit_failure_threshold: 3,
            circuit_recovery_secs: 300,
            locale: "zh".to_string(),
            theme: "light".to_string(),
            circuit_disable_codes: "401,403,410".to_string(),
            circuit_retry_codes: "100-199,300-399,401-407,409-499,500-503,505-523,525-599".to_string(),
            disable_keywords: "Your credit balance is too low\nThis organization has been disabled.\nYou exceeded your current quota\nPermission denied\nThe security token included in the request is invalid\nOperation not allowed\nYour account is not authorized".to_string(),
            autostart: false,
            start_minimized: false,
            show_guide: true,
            default_sort_mode: "custom".to_string(),
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
            settings.circuit_failure_threshold = v.parse().unwrap_or(1);
        }
        if let Some(v) = kv.get("circuit_recovery_secs") {
            settings.circuit_recovery_secs = v.parse().unwrap_or(300);
        }
        if let Some(v) = kv.get("circuit_disable_codes") {
            settings.circuit_disable_codes = v.clone();
        }
        if let Some(v) = kv.get("circuit_retry_codes") {
            settings.circuit_retry_codes = v.clone();
        }
        if let Some(v) = kv.get("disable_keywords") {
            settings.disable_keywords = v.clone();
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
        if let Some(v) = kv.get("show_guide") {
            settings.show_guide = v == "1";
        }
        if let Some(v) = kv.get("default_sort_mode") {
            settings.default_sort_mode = v.clone();
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
            ("circuit_disable_codes", &updates.circuit_disable_codes),
            ("circuit_retry_codes", &updates.circuit_retry_codes),
            ("disable_keywords", &updates.disable_keywords),
            ("locale", &updates.locale),
            ("theme", &updates.theme),
            ("autostart", if updates.autostart { "1" } else { "0" }),
            (
                "start_minimized",
                if updates.start_minimized { "1" } else { "0" },
            ),
            ("show_guide", if updates.show_guide { "1" } else { "0" }),
            ("default_sort_mode", &updates.default_sort_mode),
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
