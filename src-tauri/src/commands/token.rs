use crate::database::AccessKey;
use crate::error::AppError;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub fn list_access_keys(state: State<'_, AppState>) -> Result<Vec<AccessKey>, AppError> {
    state.db.list_access_keys()
}

#[tauri::command]
pub fn create_access_key(state: State<'_, AppState>, name: String) -> Result<AccessKey, AppError> {
    state.db.create_access_key(&name)
}

#[tauri::command]
pub fn delete_access_key(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    state.db.delete_access_key(&id)
}

#[tauri::command]
pub fn toggle_access_key(
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
) -> Result<(), AppError> {
    state.db.toggle_access_key(&id, enabled)
}
