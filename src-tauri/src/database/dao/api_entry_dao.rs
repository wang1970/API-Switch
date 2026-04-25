use crate::database::{lock_conn, Database};
use crate::error::AppError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiEntry {
    pub id: String,
    pub channel_id: String,
    pub model: String,
    pub display_name: String,
    pub sort_index: i32,
    pub enabled: bool,
    #[serde(default = "default_circuit_state")]
    pub circuit_state: String,
    pub created_at: i64,
    pub updated_at: i64,
    // Joined from channel
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel_api_type: Option<String>,
    // Model's owned_by from channel_api_type mapping
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub owned_by: Option<String>,
}

fn default_circuit_state() -> String {
    "closed".to_string()
}

impl Database {
    pub fn list_entries(&self) -> Result<Vec<ApiEntry>, AppError> {
        let conn = lock_conn!(self.conn);
        let mut stmt = conn.prepare(
            "SELECT e.id, e.channel_id, e.model, e.display_name, e.sort_index, e.enabled,
                    e.created_at, e.updated_at, c.name, c.api_type
             FROM api_entries e
             LEFT JOIN channels c ON e.channel_id = c.id
             ORDER BY e.sort_index, e.created_at",
        )?;

        let entries = stmt
            .query_map([], |row| {
                let enabled: i32 = row.get(5)?;
                Ok(ApiEntry {
                    id: row.get(0)?,
                    channel_id: row.get(1)?,
                    model: row.get(2)?,
                    display_name: row.get(3)?,
                    sort_index: row.get(4)?,
                    enabled: enabled != 0,
                    circuit_state: "closed".to_string(),
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                    channel_name: row.get(8).ok(),
                    channel_api_type: row.get(9).ok(),
                    owned_by: None,
                })
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| AppError::Database(e.to_string()))?;

        Ok(entries)
    }

    pub fn create_entry(
        &self,
        channel_id: &str,
        model: &str,
        display_name: &str,
        sort_index: i32,
    ) -> Result<ApiEntry, AppError> {
        let conn = lock_conn!(self.conn);
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();

        conn.execute(
            "INSERT INTO api_entries (id, channel_id, model, display_name, sort_index, enabled, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?6)",
            rusqlite::params![id, channel_id, model, display_name, sort_index, now],
        )?;

        Ok(ApiEntry {
            id,
            channel_id: channel_id.to_string(),
            model: model.to_string(),
            display_name: display_name.to_string(),
            sort_index,
            enabled: true,
            circuit_state: "closed".to_string(),
            created_at: now,
            updated_at: now,
            channel_name: None,
            channel_api_type: None,
            owned_by: None,
        })
    }

    pub fn create_entry_auto(
        &self,
        channel_id: &str,
        model: &str,
        display_name: &str,
    ) -> Result<ApiEntry, AppError> {
        if let Some(existing) = self.find_entry_by_channel_and_model(channel_id, model)? {
            return Ok(existing);
        }

        let conn = lock_conn!(self.conn);
        let next_sort: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(sort_index), -1) + 1 FROM api_entries",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        drop(conn);

        self.create_entry(channel_id, model, display_name, next_sort)
    }

    pub fn toggle_entry(&self, id: &str, enabled: bool) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "UPDATE api_entries SET enabled=?1, updated_at=?2 WHERE id=?3",
            rusqlite::params![enabled as i32, now, id],
        )?;
        Ok(())
    }

    /// Set a single entry's sort_index to 0 and shift others down to keep relative order.
    pub fn set_entry_priority(&self, entry_id: &str, sort_index: i32) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);
        // Set all entries' sort_index to sort_index + 1 first, then set the target to sort_index
        conn.execute(
            "UPDATE api_entries SET sort_index = sort_index + 1, updated_at = (SELECT strftime('%s','now')) WHERE id != ?1",
            rusqlite::params![entry_id],
        )?;
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "UPDATE api_entries SET sort_index = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![sort_index, now, entry_id],
        )?;
        Ok(())
    }

    pub fn reorder_entries(&self, ordered_ids: &[String]) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);
        let now = chrono::Utc::now().timestamp();
        for (i, id) in ordered_ids.iter().enumerate() {
            conn.execute(
                "UPDATE api_entries SET sort_index=?1, updated_at=?2 WHERE id=?3",
                rusqlite::params![i as i32, now, id],
            )?;
        }
        Ok(())
    }

    pub fn delete_entries_by_channel(&self, channel_id: &str) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);
        conn.execute(
            "DELETE FROM api_entries WHERE channel_id = ?1",
            [channel_id],
        )?;
        Ok(())
    }

    pub fn delete_entry(&self, id: &str) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);
        conn.execute("DELETE FROM api_entries WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn disable_entries_for_channel(&self, channel_id: &str) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);
        conn.execute(
            "UPDATE api_entries SET enabled = 0 WHERE channel_id = ?1 AND enabled = 1",
            [channel_id],
        )?;
        Ok(())
    }

    pub fn find_entry_by_channel_and_model(
        &self,
        channel_id: &str,
        model: &str,
    ) -> Result<Option<ApiEntry>, AppError> {
        let conn = lock_conn!(self.conn);
        let mut stmt = conn.prepare(
            "SELECT id, channel_id, model, display_name, sort_index, enabled, created_at, updated_at
             FROM api_entries WHERE channel_id = ?1 AND model = ?2"
        )?;

        let result = stmt.query_row(rusqlite::params![channel_id, model], |row| {
            let enabled: i32 = row.get(5)?;
            Ok(ApiEntry {
                id: row.get(0)?,
                channel_id: row.get(1)?,
                model: row.get(2)?,
                display_name: row.get(3)?,
                sort_index: row.get(4)?,
                enabled: enabled != 0,
                circuit_state: "closed".to_string(),
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
                channel_name: None,
                channel_api_type: None,
                owned_by: None,
            })
        });

        match result {
            Ok(entry) => Ok(Some(entry)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(AppError::Database(e.to_string())),
        }
    }

    /// Sync api_entries with selected_models of a channel:
    /// - Add entries for newly selected models
    /// - Remove entries for unselected models
    pub fn sync_entries_for_channel(
        &self,
        channel_id: &str,
        selected_models: &[String],
    ) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);

        // Get current entries for this channel
        let mut stmt = conn.prepare("SELECT model FROM api_entries WHERE channel_id = ?1")?;
        let current_models: Vec<String> = stmt
            .query_map([channel_id], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        // Add new entries
        let now = chrono::Utc::now().timestamp();
        let max_sort: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(sort_index), -1) FROM api_entries",
                [],
                |row| row.get(0),
            )
            .unwrap_or(-1);
        let mut next_sort = max_sort + 1;

        for model in selected_models {
            if !current_models.contains(model) {
                let id = uuid::Uuid::new_v4().to_string();
                conn.execute(
                    "INSERT INTO api_entries (id, channel_id, model, display_name, sort_index, enabled, created_at, updated_at)
                      VALUES (?1, ?2, ?3, ?3, ?4, 0, ?5, ?5)",
                    rusqlite::params![id, channel_id, model, next_sort, now],
                )?;
                next_sort += 1;
            }
        }

        // Remove unselected entries
        for model in &current_models {
            if !selected_models.contains(model) {
                conn.execute(
                    "DELETE FROM api_entries WHERE channel_id = ?1 AND model = ?2",
                    rusqlite::params![channel_id, model],
                )?;
            }
        }

        Ok(())
    }

    /// Get all enabled entries for proxy routing (with channel info).
    pub fn get_enabled_entries_for_routing(&self) -> Result<Vec<ApiEntry>, AppError> {
        let conn = lock_conn!(self.conn);
        let mut stmt = conn.prepare(
            "SELECT e.id, e.channel_id, e.model, e.display_name, e.sort_index, e.enabled,
                    e.created_at, e.updated_at, c.name, c.api_type
             FROM api_entries e
             LEFT JOIN channels c ON e.channel_id = c.id
             WHERE e.enabled = 1 AND c.enabled = 1
              ORDER BY e.sort_index, e.created_at",
        )?;

        let entries = stmt
            .query_map([], |row| {
                let enabled: i32 = row.get(5)?;
                let owned_by =
                    row.get::<_, String>(9)
                        .ok()
                        .and_then(|api_type| match api_type.as_str() {
                            "openai" | "anthropic" => Some("openai".to_string()),
                            "claude" => Some("anthropic".to_string()),
                            "gemini" => Some("google".to_string()),
                            "azure" => Some("openai".to_string()),
                            "custom" => Some("custom".to_string()),
                            _ => Some(api_type),
                        });
                Ok(ApiEntry {
                    id: row.get(0)?,
                    channel_id: row.get(1)?,
                    model: row.get(2)?,
                    display_name: row.get(3)?,
                    sort_index: row.get(4)?,
                    enabled: enabled != 0,
                    circuit_state: "closed".to_string(),
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                    channel_name: row.get(8).ok(),
                    channel_api_type: row.get(9).ok(),
                    owned_by,
                })
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| AppError::Database(e.to_string()))?;

        Ok(entries)
    }

    /// Get all entries (including disabled) with channel info. Used for test chat.
    pub fn get_entries_for_routing_all(&self) -> Result<Vec<ApiEntry>, AppError> {
        let conn = lock_conn!(self.conn);
        let mut stmt = conn.prepare(
            "SELECT e.id, e.channel_id, e.model, e.display_name, e.sort_index, e.enabled,
                    e.created_at, e.updated_at, c.name, c.api_type
             FROM api_entries e
             LEFT JOIN channels c ON e.channel_id = c.id
              ORDER BY e.sort_index, e.created_at",
        )?;

        let entries = stmt
            .query_map([], |row| {
                let enabled: i32 = row.get(5)?;
                let owned_by =
                    row.get::<_, String>(9)
                        .ok()
                        .and_then(|api_type| match api_type.as_str() {
                            "openai" | "anthropic" => Some("openai".to_string()),
                            "claude" => Some("anthropic".to_string()),
                            "gemini" => Some("google".to_string()),
                            "azure" => Some("openai".to_string()),
                            "custom" => Some("custom".to_string()),
                            _ => Some(api_type),
                        });
                Ok(ApiEntry {
                    id: row.get(0)?,
                    channel_id: row.get(1)?,
                    model: row.get(2)?,
                    display_name: row.get(3)?,
                    sort_index: row.get(4)?,
                    enabled: enabled != 0,
                    circuit_state: "closed".to_string(),
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                    channel_name: row.get(8).ok(),
                    channel_api_type: row.get(9).ok(),
                    owned_by,
                })
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| AppError::Database(e.to_string()))?;

        Ok(entries)
    }
}
