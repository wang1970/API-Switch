use crate::database::ApiEntry;
use crate::error::AppError;
use crate::AppState;
use crate::TRAY_ID;
use crate::build_tray_menu;
use crate::proxy::protocol::get_adapter;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Instant;
use tauri::{Manager, State};

#[derive(Serialize)]
pub struct TestResult {
    pub status: String,
    pub response_ms: String,
}

#[derive(Deserialize)]
pub struct CreateEntryParams {
    pub channel_id: String,
    pub model: String,
    pub display_name: Option<String>,
}

#[tauri::command]
pub fn list_entries(state: State<'_, AppState>) -> Result<Vec<ApiEntry>, AppError> {
    state.db.list_entries()
}

#[tauri::command]
pub fn toggle_entry(app: tauri::AppHandle, state: State<'_, AppState>, id: String, enabled: bool) -> Result<(), AppError> {
    state.db.toggle_entry(&id, enabled)?;
    if let Ok(new_menu) = build_tray_menu(&app) {
        if let Some(tray) = app.tray_by_id(TRAY_ID) {
            let _ = tray.set_menu(Some(new_menu));
        }
    }
    Ok(())
}

#[tauri::command]
pub fn reorder_entries(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    ordered_ids: Vec<String>,
) -> Result<(), AppError> {
    state.db.reorder_entries(&ordered_ids)?;
    if let Ok(new_menu) = build_tray_menu(&app) {
        if let Some(tray) = app.tray_by_id(TRAY_ID) {
            let _ = tray.set_menu(Some(new_menu));
        }
    }
    Ok(())
}

#[tauri::command]
pub fn create_entry(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    params: CreateEntryParams,
) -> Result<ApiEntry, AppError> {
    let display_name = params.display_name.as_deref().unwrap_or(&params.model);
    let entry = state
        .db
        .create_entry_auto(&params.channel_id, &params.model, display_name)?;
    if let Ok(new_menu) = build_tray_menu(&app) {
        if let Some(tray) = app.tray_by_id(TRAY_ID) {
            let _ = tray.set_menu(Some(new_menu));
        }
    }
    Ok(entry)
}

#[tauri::command]
pub async fn test_entry_latency(
    state: State<'_, AppState>,
    entry_id: String,
) -> Result<TestResult, AppError> {
    let db = state.db.clone();

    let entries = db.get_entries_for_routing_all()?;
    let entry = entries
        .iter()
        .find(|e| e.id == entry_id)
        .ok_or_else(|| AppError::NotFound(format!("Entry {entry_id} not found")))?
        .clone();

    let channel = db.get_channel(&entry.channel_id)?;
    let adapter = get_adapter(&channel.api_type);
    let url = adapter.build_chat_url(&channel.base_url, &entry.model);

    let mut upstream_body = json!({
        "model": entry.model,
        "messages": [{"role": "user", "content": "hi"}],
        "stream": false,
    });
    adapter.transform_request(&mut upstream_body, &entry.model);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|e| AppError::Network(format!("HTTP client: {e}")))?;

    let request = adapter
        .apply_auth(client.post(&url), &channel.api_key)
        .json(&upstream_body);

    let start = Instant::now();
    let response = request
        .send()
        .await
        .map_err(|e| AppError::Network(format!("Request failed: {e}")))?;

    let latency_ms = start.elapsed().as_millis() as u64;

    if !response.status().is_success() {
        let _error_body = response.text().await.unwrap_or_default();
        let _ = db.update_entry_response_ms(&entry_id, "X");
        return Ok(TestResult {
            status: "cooldown".to_string(),
            response_ms: "X".to_string(),
        });
    }

    // Consume body to ensure complete response
    let _ = response.bytes().await;

    let response_ms = if latency_ms >= 1000 {
        format!("{:.1}s", latency_ms as f64 / 1000.0)
    } else {
        format!("{}ms", latency_ms)
    };

    db.update_entry_response_ms(&entry_id, &response_ms)?;

    Ok(TestResult {
        status: "ok".to_string(),
        response_ms,
    })
}

#[tauri::command]
pub fn update_entry_response_ms(
    state: State<'_, AppState>,
    entry_id: String,
    response_ms: String,
) -> Result<(), AppError> {
    state.db.update_entry_response_ms(&entry_id, &response_ms)
}
