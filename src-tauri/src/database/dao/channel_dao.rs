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
    pub response_ms: String,
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
                    enabled, last_fetch_at, notes, response_ms, created_at, updated_at
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
                    response_ms: row.get(10)?,
                    created_at: row.get(11)?,
                    updated_at: row.get(12)?,
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
            response_ms: String::new(),
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
                        enabled, last_fetch_at, notes, response_ms, created_at, updated_at
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
                    response_ms: row.get(10)?,
                    created_at: row.get(11)?,
                    updated_at: row.get(12)?,
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
            rusqlite::params![
                name,
                api_type,
                base_url,
                api_key,
                enabled_val,
                notes,
                now,
                id
            ],
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

    pub fn add_channel_model_if_missing(&self, channel_id: &str, model: &str, owned_by: Option<&str>) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);
        let now = chrono::Utc::now().timestamp();

        let (available_models_str, selected_models_str): (String, String) = conn.query_row(
            "SELECT available_models, selected_models FROM channels WHERE id = ?1",
            [channel_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;

        let mut available_models: Vec<ModelInfo> = serde_json::from_str(&available_models_str).unwrap_or_default();
        let mut selected_models: Vec<String> = serde_json::from_str(&selected_models_str).unwrap_or_default();

        if !available_models.iter().any(|m| m.id == model || m.name == model) {
            available_models.push(ModelInfo {
                id: model.to_string(),
                name: model.to_string(),
                owned_by: owned_by.map(str::to_string),
            });
        }

        if !selected_models.iter().any(|m| m == model) {
            selected_models.push(model.to_string());
        }

        let available_json = serde_json::to_string(&available_models)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let selected_json = serde_json::to_string(&selected_models)
            .map_err(|e| AppError::Internal(e.to_string()))?;

        conn.execute(
            "UPDATE channels SET available_models=?1, selected_models=?2, updated_at=?3 WHERE id=?4",
            rusqlite::params![available_json, selected_json, now, channel_id],
        )?;

        Ok(())
    }

    pub fn get_channel(&self, id: &str) -> Result<Channel, AppError> {
        let conn = lock_conn!(self.conn);
        let mut stmt = conn.prepare(
            "SELECT id, name, api_type, base_url, api_key, available_models, selected_models,
                    enabled, last_fetch_at, notes, response_ms, created_at, updated_at
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
                response_ms: row.get(10)?,
                created_at: row.get(11)?,
                updated_at: row.get(12)?,
            })
        })
        .map_err(|e| AppError::NotFound(format!("Channel {id}: {e}")))
    }

    pub fn update_channel_endpoint(
        &self,
        id: &str,
        api_type: &str,
        base_url: &str,
    ) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "UPDATE channels SET api_type=?1, base_url=?2, updated_at=?3 WHERE id=?4",
            rusqlite::params![api_type, base_url, now, id],
        )?;
        Ok(())
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

    /// Update channel response time.
    pub fn update_channel_response_ms(
        &self,
        channel_id: &str,
        response_ms: &str,
    ) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "UPDATE channels SET response_ms = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![response_ms, now, channel_id],
        )?;
        Ok(())
    }
}
