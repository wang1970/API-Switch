use crate::build_tray_menu;
use crate::database::{Channel, ModelInfo};
use crate::error::AppError;
use crate::proxy::protocol::get_adapter;
use crate::AppState;
use crate::TRAY_ID;
use serde::Deserialize;
use tauri::{Manager, State};

#[derive(Deserialize)]
pub struct CreateChannelParams {
    pub name: String,
    pub api_type: String,
    pub base_url: String,
    pub api_key: String,
    pub notes: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateChannelParams {
    pub id: String,
    pub name: Option<String>,
    pub api_type: Option<String>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub enabled: Option<bool>,
    pub notes: Option<String>,
}

#[tauri::command]
pub fn list_channels(state: State<'_, AppState>) -> Result<Vec<Channel>, AppError> {
    state.db.list_channels()
}

#[tauri::command]
pub fn create_channel(
    state: State<'_, AppState>,
    params: CreateChannelParams,
) -> Result<Channel, AppError> {
    state.db.create_channel(
        &params.name,
        &params.api_type,
        &params.base_url,
        &params.api_key,
        params.notes.as_deref(),
    )
}

#[tauri::command]
pub fn update_channel(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    params: UpdateChannelParams,
) -> Result<Channel, AppError> {
    // If disabling, also disable all associated API pool entries
    if let Some(false) = params.enabled {
        state.db.disable_entries_for_channel(&params.id)?;
    }

    state.db.update_channel(
        &params.id,
        params.name.as_deref(),
        params.api_type.as_deref(),
        params.base_url.as_deref(),
        params.api_key.as_deref(),
        params.enabled,
        params.notes.as_deref(),
    )?;
    if let Ok(new_menu) = build_tray_menu(&app) {
        if let Some(tray) = app.tray_by_id(TRAY_ID) {
            let _ = tray.set_menu(Some(new_menu));
        }
    }
    state.db.get_channel(&params.id)
}

#[tauri::command]
pub fn delete_channel(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<(), AppError> {
    state.db.delete_channel(&id)?;
    if let Ok(new_menu) = build_tray_menu(&app) {
        if let Some(tray) = app.tray_by_id(TRAY_ID) {
            let _ = tray.set_menu(Some(new_menu));
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn fetch_models(
    state: State<'_, AppState>,
    channel_id: String,
) -> Result<Vec<ModelInfo>, AppError> {
    let channel = state.db.get_channel(&channel_id)?;

    // Fetch models from upstream API via protocol adapter
    let models =
        fetch_models_from_api(&channel.api_type, &channel.base_url, &channel.api_key).await?;

    // Update available_models in DB
    state
        .db
        .update_channel_models(&channel_id, &models, &channel.selected_models)?;

    Ok(models)
}

/// Fetch models directly from upstream API without needing a saved channel.
/// Used by the "add channel" dialog to preview models before saving.
#[tauri::command]
pub async fn fetch_models_direct(
    _state: State<'_, AppState>,
    api_type: String,
    base_url: String,
    api_key: String,
) -> Result<Vec<ModelInfo>, AppError> {
    fetch_models_from_api(&api_type, &base_url, &api_key).await
}

#[tauri::command]
pub fn select_models(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    channel_id: String,
    model_names: Vec<String>,
) -> Result<(), AppError> {
    let channel = state.db.get_channel(&channel_id)?;
    state
        .db
        .update_channel_models(&channel_id, &channel.available_models, &model_names)?;
    state
        .db
        .sync_entries_for_channel(&channel_id, &model_names)?;
    if let Ok(new_menu) = build_tray_menu(&app) {
        if let Some(tray) = app.tray_by_id(TRAY_ID) {
            let _ = tray.set_menu(Some(new_menu));
        }
    }
    Ok(())
}

async fn fetch_models_from_api(
    api_type: &str,
    base_url: &str,
    api_key: &str,
) -> Result<Vec<ModelInfo>, AppError> {
    let adapter = get_adapter(api_type);

    // Use adapter to build URL, auth, and parse response
    let url = adapter.build_models_url(base_url, api_key);

    let client = reqwest::Client::new();
    let request = adapter.apply_auth(client.get(&url), api_key);

    let response = request
        .send()
        .await
        .map_err(|e| AppError::Network(format!("Failed to fetch models: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::Network(format!(
            "Failed to fetch models: {status} - {body}"
        )));
    }

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| AppError::Network(format!("Failed to parse models response: {e}")))?;

    // Use adapter to parse models response
    let models = adapter
        .parse_models_response(&body)
        .into_iter()
        .map(|(id, owned_by)| ModelInfo {
            name: id.clone(),
            id,
            owned_by,
        })
        .collect::<Vec<_>>();

    // Deduplicate by name
    let mut seen = std::collections::HashSet::new();
    let deduped: Vec<ModelInfo> = models
        .into_iter()
        .filter(|m| seen.insert(m.name.clone()))
        .collect();

    Ok(deduped)
}
