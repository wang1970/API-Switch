use crate::database::AppSettings;
use crate::error::AppError;
use crate::AppState;
use serde::Deserialize;
use tauri::{Emitter, State};

const GITHUB_REPO: &str = "wang1970/API-Switch";

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
    body: Option<String>,
}

#[tauri::command]
pub async fn check_update(app: tauri::AppHandle) -> Result<Option<serde_json::Value>, AppError> {
    let current = env!("CARGO_PKG_VERSION");

    let url = format!("https://api.github.com/repos/{}/releases/latest", GITHUB_REPO);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| AppError::Network(e.to_string()))?;

    let resp = client
        .get(&url)
        .header("User-Agent", "api-switch")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| AppError::Network(e.to_string()))?;

    if !resp.status().is_success() {
        return Ok(None);
    }

    let release: GithubRelease = resp
        .json()
        .await
        .map_err(|e| AppError::Network(e.to_string()))?;

    let latest = release.tag_name.trim_start_matches('v').to_string();

    if latest == current {
        return Ok(None);
    }

    let update_info = serde_json::json!({
        "current": current,
        "latest": latest,
        "url": release.html_url,
    });

    let _ = app.emit("update-available", &update_info);
    Ok(Some(update_info))
}

fn sync_autostart(settings: &AppSettings) {
    let app_name = "API Switch";
    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(e) => {
            log::error!("Failed to get exe path: {e}");
            return;
        }
    };
    let exe_path = exe.to_string_lossy().to_string();

    let auto = match auto_launch::AutoLaunchBuilder::new()
        .set_app_name(app_name)
        .set_app_path(&exe_path)
        .build()
    {
        Ok(a) => a,
        Err(e) => {
            log::error!("Failed to create AutoLaunch: {e}");
            return;
        }
    };

    let result = if settings.autostart {
        auto.enable()
    } else {
        auto.disable()
    };

    if let Err(e) = result {
        log::error!("Failed to sync autostart: {e}");
    }
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, AppError> {
    state.db.get_settings()
}

#[tauri::command]
pub fn update_settings(state: State<'_, AppState>, settings: AppSettings) -> Result<(), AppError> {
    state.db.update_settings(&settings)?;
    sync_autostart(&settings);
    Ok(())
}
