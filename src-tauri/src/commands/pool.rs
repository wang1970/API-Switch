use crate::build_tray_menu;
use crate::database::ApiEntry;
use crate::error::AppError;
use crate::AppState;
use crate::TRAY_ID;
use serde::Deserialize;
use tauri::{Manager, State};

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
pub fn toggle_entry(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
) -> Result<(), AppError> {
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
