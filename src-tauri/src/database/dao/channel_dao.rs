use crate::database::{lock_conn, Database};
use crate::error::AppError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    pub id: String,
    pub name: String,
    pub api_type: String,
    pub base_url: String,
    pub api_key: String,
    pub available_models: Vec<ModelInfo>,
    pub selected_models: Vec<String>,
    pub enabled: bool,
    pub last_fetch_at: i64,
    pub notes: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owned_by: Option<String>,
}

impl Database {
    pub fn list_channels(&self) -> Result<Vec<Channel>, AppError> {
        let conn = lock_conn!(self.conn);
        let mut stmt = conn.prepare(
            "SELECT id, name, api_type, base_url, api_key, available_models, selected_models,
                    enabled, last_fetch_at, notes, created_at, updated_at
             FROM channels ORDER BY created_at",
        )?;

        let channels = stmt
            .query_map([], |row| {
                let available_models_str: String = row.get(5)?;
                let selected_models_str: String = row.get(6)?;
                let enabled: i32 = row.get(7)?;

                Ok(Channel {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    api_type: row.get(2)?,
                    base_url: row.get(3)?,
                    api_key: row.get(4)?,
                    available_models: serde_json::from_str(&available_models_str)
                        .unwrap_or_default(),
                    selected_models: serde_json::from_str(&selected_models_str).unwrap_or_default(),
                    enabled: enabled != 0,
                    last_fetch_at: row.get(8)?,
                    notes: row.get(9)?,
                    created_at: row.get(10)?,
                    updated_at: row.get(11)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| AppError::Database(e.to_string()))?;

        Ok(channels)
    }

    pub fn create_channel(
        &self,
        name: &str,
        api_type: &str,
        base_url: &str,
        api_key: &str,
        notes: Option<&str>,
    ) -> Result<Channel, AppError> {
        let conn = lock_conn!(self.conn);
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();

        conn.execute(
            "INSERT INTO channels (id, name, api_type, base_url, api_key, available_models, selected_models, enabled, last_fetch_at, notes, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, '[]', '[]', 1, 0, ?6, ?7, ?8)",
            rusqlite::params![id, name, api_type, base_url, api_key, notes.unwrap_or(""), now, now],
        )?;

        Ok(Channel {
            id,
            name: name.to_string(),
            api_type: api_type.to_string(),
            base_url: base_url.to_string(),
            api_key: api_key.to_string(),
            available_models: vec![],
            selected_models: vec![],
            enabled: true,
            last_fetch_at: 0,
            notes: notes.unwrap_or("").to_string(),
            created_at: now,
            updated_at: now,
        })
    }

    pub fn update_channel(
        &self,
        id: &str,
        name: Option<&str>,
        api_type: Option<&str>,
        base_url: Option<&str>,
        api_key: Option<&str>,
        enabled: Option<bool>,
        notes: Option<&str>,
    ) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);
        let now = chrono::Utc::now().timestamp();

        let current: Channel = {
            let mut stmt = conn.prepare(
                "SELECT id, name, api_type, base_url, api_key, available_models, selected_models,
                        enabled, last_fetch_at, notes, created_at, updated_at
                 FROM channels WHERE id = ?1",
            )?;
            stmt.query_row([id], |row| {
                let available_models_str: String = row.get(5)?;
                let selected_models_str: String = row.get(6)?;
                let enabled: i32 = row.get(7)?;
                Ok(Channel {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    api_type: row.get(2)?,
                    base_url: row.get(3)?,
                    api_key: row.get(4)?,
                    available_models: serde_json::from_str(&available_models_str)
                        .unwrap_or_default(),
                    selected_models: serde_json::from_str(&selected_models_str).unwrap_or_default(),
                    enabled: enabled != 0,
                    last_fetch_at: row.get(8)?,
                    notes: row.get(9)?,
                    created_at: row.get(10)?,
                    updated_at: row.get(11)?,
                })
            })
            .map_err(|e| AppError::NotFound(format!("Channel {id}: {e}")))?
        };

        let name = name.unwrap_or(&current.name);
        let api_type = api_type.unwrap_or(&current.api_type);
        let base_url = base_url.unwrap_or(&current.base_url);
        let api_key = api_key.unwrap_or(&current.api_key);
        let enabled_val = enabled.unwrap_or(current.enabled) as i32;
        let notes = notes.unwrap_or(&current.notes);

        conn.execute(
            "UPDATE channels SET name=?1, api_type=?2, base_url=?3, api_key=?4, enabled=?5, notes=?6, updated_at=?7
             WHERE id=?8",
            rusqlite::params![name, api_type, base_url, api_key, enabled_val, notes, now, id],
        )?;

        Ok(())
    }

    pub fn delete_channel(&self, id: &str) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);
        conn.execute("DELETE FROM channels WHERE id = ?1", [id])?;
        // CASCADE will delete related api_entries
        Ok(())
    }

    pub fn update_channel_models(
        &self,
        id: &str,
        available_models: &[ModelInfo],
        selected_models: &[String],
    ) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);
        let now = chrono::Utc::now().timestamp();
        let available_json = serde_json::to_string(available_models)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let selected_json = serde_json::to_string(selected_models)
            .map_err(|e| AppError::Internal(e.to_string()))?;

        conn.execute(
            "UPDATE channels SET available_models=?1, selected_models=?2, last_fetch_at=?3, updated_at=?4
             WHERE id=?5",
            rusqlite::params![available_json, selected_json, now, now, id],
        )?;

        Ok(())
    }

    pub fn get_channel(&self, id: &str) -> Result<Channel, AppError> {
        let conn = lock_conn!(self.conn);
        let mut stmt = conn.prepare(
            "SELECT id, name, api_type, base_url, api_key, available_models, selected_models,
                    enabled, last_fetch_at, notes, created_at, updated_at
             FROM channels WHERE id = ?1",
        )?;

        stmt.query_row([id], |row| {
            let available_models_str: String = row.get(5)?;
            let selected_models_str: String = row.get(6)?;
            let enabled: i32 = row.get(7)?;

            Ok(Channel {
                id: row.get(0)?,
                name: row.get(1)?,
                api_type: row.get(2)?,
                base_url: row.get(3)?,
                api_key: row.get(4)?,
                available_models: serde_json::from_str(&available_models_str).unwrap_or_default(),
                selected_models: serde_json::from_str(&selected_models_str).unwrap_or_default(),
                enabled: enabled != 0,
                last_fetch_at: row.get(8)?,
                notes: row.get(9)?,
                created_at: row.get(10)?,
                updated_at: row.get(11)?,
            })
        })
        .map_err(|e| AppError::NotFound(format!("Channel {id}: {e}")))
    }

    /// Disable a channel by ID (sets enabled=0 and cascades to api_entries).
    pub fn disable_channel(&self, channel_id: &str) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "UPDATE channels SET enabled = 0, updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, channel_id],
        )?;
        conn.execute(
            "UPDATE api_entries SET enabled = 0, updated_at = ?1 WHERE channel_id = ?2",
            rusqlite::params![now, channel_id],
        )?;
        log::warn!("Channel disabled by keyword match: {channel_id}");
        Ok(())
    }
}
