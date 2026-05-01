use crate::database::AppSettings;
use crate::error::AppError;
use crate::AppState;
use serde::Deserialize;
use tauri::{Manager, State};

const GITHUB_REPO: &str = "wang1970/API-Switch";

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
    body: Option<String>,
}

#[tauri::command]
pub async fn check_update() -> Result<Option<serde_json::Value>, AppError> {
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

    Ok(Some(serde_json::json!({
        "current": current,
        "latest": latest,
        "url": release.html_url,
    })))
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
pub async fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, AppError> {
    Ok(state.settings.read().await.clone())
}

pub async fn refresh_settings_l1(state: &AppState) -> Result<AppSettings, AppError> {
    // Settings writes are rare and settings are small.
    // Keep DB as the source of truth: after every settings write,
    // rebuild the L1 settings cache from DB instead of patching fields manually.
    let settings = state.db.get_settings()?;
    *state.settings.write().await = settings.clone();
    Ok(settings)
}

#[tauri::command]
pub async fn update_settings(app: tauri::AppHandle, state: State<'_, AppState>, settings: AppSettings) -> Result<(), AppError> {
    state.db.update_settings(&settings)?;
    let settings = refresh_settings_l1(&state).await?;
    sync_autostart(&settings);
    // Rebuild tray menu to reflect updated sort_mode
    if let Ok(new_menu) = crate::build_tray_menu(&app) {
        if let Some(tray) = app.tray_by_id(crate::TRAY_ID) {
            let _ = tray.set_menu(Some(new_menu));
        }
    }
    Ok(())
}
