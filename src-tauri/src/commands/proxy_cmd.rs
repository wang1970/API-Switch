use crate::error::AppError;
use crate::proxy::ProxyStatus;
use crate::AppState;
use crate::TRAY_ID;
use tauri::{Manager, State};

#[tauri::command]
pub fn refresh_tray_menu(app: tauri::AppHandle) -> Result<(), AppError> {
    let new_menu = crate::build_tray_menu(&app)?;
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        tray.set_menu(Some(new_menu))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn start_proxy(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<ProxyStatus, AppError> {
    let settings = state.settings.read().await.clone();
    let port = settings.listen_port;

    let mut proxy_guard = state.proxy.write().await;
    if proxy_guard.is_some() {
        return Err(AppError::Proxy("Proxy already running".to_string()));
    }

    let server = crate::proxy::ProxyServer::new(port, state.db.clone(), state.settings.clone(), app, state.failure_counts.clone());
    server.start().await.map_err(|e| AppError::Proxy(e.to_string()))?;

    let status = ProxyStatus {
        running: true,
        address: "127.0.0.1".to_string(),
        port,
    };

    *proxy_guard = Some(server);

    // Update config and L1 settings cache
    state.db.set_config_value("proxy_enabled", "1")?;
    state.settings.write().await.proxy_enabled = true;

    Ok(status)
}

#[tauri::command]
pub async fn stop_proxy(state: State<'_, AppState>) -> Result<(), AppError> {
    let mut proxy_guard = state.proxy.write().await;
    if let Some(server) = proxy_guard.take() {
        server.stop().await.map_err(|e| AppError::Proxy(e.to_string()))?;
    }
    state.db.set_config_value("proxy_enabled", "0")?;
    state.settings.write().await.proxy_enabled = false;
    Ok(())
}

#[tauri::command]
pub async fn get_proxy_status(state: State<'_, AppState>) -> Result<ProxyStatus, AppError> {
    let proxy_guard = state.proxy.read().await;
    let settings = state.settings.read().await.clone();

    Ok(match proxy_guard.as_ref() {
        Some(server) => server.get_status(),
        None => ProxyStatus {
            running: false,
            address: "127.0.0.1".to_string(),
            port: settings.listen_port,
        },
    })
}
