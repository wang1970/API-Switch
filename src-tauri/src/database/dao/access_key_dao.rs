use crate::database::{lock_conn, Database};
use crate::error::AppError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessKey {
    pub id: String,
    pub name: String,
    pub key: String,
    pub enabled: bool,
    pub created_at: i64,
}

impl Database {
    pub fn list_access_keys(&self) -> Result<Vec<AccessKey>, AppError> {
        let conn = lock_conn!(self.conn);
        let mut stmt = conn.prepare(
            "SELECT id, name, key, enabled, created_at FROM access_keys ORDER BY created_at",
        )?;

        let keys = stmt
            .query_map([], |row| {
                let enabled: i32 = row.get(3)?;
                Ok(AccessKey {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    key: row.get(2)?,
                    enabled: enabled != 0,
                    created_at: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| AppError::Database(e.to_string()))?;

        Ok(keys)
    }

    pub fn create_access_key(&self, name: &str) -> Result<AccessKey, AppError> {
        let conn = lock_conn!(self.conn);
        let id = uuid::Uuid::new_v4().to_string();
        let key = format!("sk-{}", uuid::Uuid::new_v4().to_string().replace("-", ""));
        let now = chrono::Utc::now().timestamp();

        conn.execute(
            "INSERT INTO access_keys (id, name, key, enabled, created_at) VALUES (?1, ?2, ?3, 1, ?4)",
            rusqlite::params![id, name, key, now],
        )?;

        Ok(AccessKey {
            id,
            name: name.to_string(),
            key,
            enabled: true,
            created_at: now,
        })
    }

    pub fn delete_access_key(&self, id: &str) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);
        conn.execute("DELETE FROM access_keys WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn toggle_access_key(&self, id: &str, enabled: bool) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);
        conn.execute(
            "UPDATE access_keys SET enabled = ?1 WHERE id = ?2",
            rusqlite::params![enabled as i32, id],
        )?;
        Ok(())
    }

    pub fn find_access_key_by_key(&self, key: &str) -> Result<Option<AccessKey>, AppError> {
        let conn = lock_conn!(self.conn);
        let result = conn.query_row(
            "SELECT id, name, key, enabled, created_at FROM access_keys WHERE key = ?1",
            [key],
            |row| {
                let enabled: i32 = row.get(3)?;
                Ok(AccessKey {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    key: row.get(2)?,
                    enabled: enabled != 0,
                    created_at: row.get(4)?,
                })
            },
        );

        match result {
            Ok(ak) => Ok(Some(ak)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(AppError::Database(e.to_string())),
        }
    }
}
