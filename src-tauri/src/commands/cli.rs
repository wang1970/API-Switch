use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

const REMOTE_URL: &str = "https://raw.githubusercontent.com/wang1970/API-Switch/master/cli.json";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CachedCliData {
    pub fetched_at: u64,
    pub data: serde_json::Value,
}

fn cache_path() -> PathBuf {
    let dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    dir.join("cli-cache.json")
}

fn read_local_cache() -> Option<CachedCliData> {
    let data = std::fs::read_to_string(cache_path()).ok()?;
    serde_json::from_str(&data).ok()
}

fn write_local_cache(data: &CachedCliData) {
    if let Ok(json) = serde_json::to_string_pretty(data) {
        let _ = std::fs::write(cache_path(), json);
    }
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[tauri::command]
pub async fn get_cli_data() -> Result<Option<serde_json::Value>, AppError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| AppError::Network(e.to_string()))?;

    let resp = client
        .get(REMOTE_URL)
        .header("User-Agent", "api-switch")
        .send()
        .await;

    match resp {
        Ok(r) if r.status().is_success() => {
            let body = r.text().await.unwrap_or_default();
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&body) {
                let cached = CachedCliData {
                    fetched_at: now_unix(),
                    data: value.clone(),
                };
                write_local_cache(&cached);
                return Ok(Some(value));
            }
        }
        _ => {}
    }

    if let Some(cached) = read_local_cache() {
        return Ok(Some(cached.data));
    }

    Ok(None)
}

#[derive(Deserialize)]
pub struct EnvVarParam {
    pub key: String,
    pub value: String,
}

#[tauri::command]
pub fn set_user_env_vars(vars: Vec<EnvVarParam>) -> Result<(), AppError> {
    for var in vars {
        if var.key.trim().is_empty() {
            continue;
        }
        #[cfg(target_os = "windows")]
        {
            Command::new("setx")
                .arg(&var.key)
                .arg(&var.value)
                .output()
                .map_err(|e| AppError::Internal(format!("setx {} failed: {}", var.key, e)))?;
        }

        #[cfg(not(target_os = "windows"))]
        {
            return Err(AppError::Internal(
                "Setting persistent user environment variables is only implemented on Windows".to_string(),
            ));
        }
    }
    Ok(())
}
