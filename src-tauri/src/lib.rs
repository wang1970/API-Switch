mod commands;
mod database;
mod error;
mod proxy;

use database::{AppSettings, Database};
use proxy::ProxyServer;
use std::sync::Arc;
use tauri::{Emitter, Manager};
use tauri::menu::{Menu, MenuItem, CheckMenuItem, PredefinedMenuItem};

pub use error::AppError;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Database>,
    pub settings: Arc<tokio::sync::RwLock<AppSettings>>,
    pub proxy: Arc<tokio::sync::RwLock<Option<ProxyServer>>>,
    pub failure_counts: Arc<tokio::sync::RwLock<std::collections::HashMap<String, u32>>>,
}

pub(crate) const TRAY_ID: &str = "api-switch-tray";
pub(crate) const EXPERIMENTAL_LAZY_TRAY_REFRESH: bool = false;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            // Initialize database
            let db = Database::open()?;
            db.create_tables()?;
            let settings_cache = db.get_settings().unwrap_or_default();

            let state = AppState {
                db: Arc::new(db),
                settings: Arc::new(tokio::sync::RwLock::new(settings_cache)),
                proxy: Arc::new(tokio::sync::RwLock::new(None)),
                failure_counts: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
            };
            app.manage(state);

            // Auto-start proxy if proxy_enabled is set
            let handle = app.handle().clone();
            tauri::async_runtime::block_on(async {
                let app_state = handle.state::<AppState>();
                let settings = app_state.settings.read().await.clone();
                if settings.proxy_enabled {
                    let port = settings.listen_port;
                    let server = ProxyServer::new(
                        port,
                        app_state.db.clone(),
                        app_state.settings.clone(),
                        handle.clone(),
                        app_state.failure_counts.clone(),
                    );
                    if let Err(e) = server.start().await {
                        log::error!("Failed to auto-start proxy: {e}");
                    } else {
                        let mut proxy_guard = app_state.proxy.write().await;
                        *proxy_guard = Some(server);
                        log::info!("Proxy auto-started on port {port}");
                    }
                }
            });

            // Read settings to decide startup behavior
            let settings = app.state::<AppState>().settings.blocking_read().clone();

            // Build tray icon (ref: cc-switch/src/lib.rs)
            let tray_menu = build_tray_menu(app.handle())?;
            let _tray = tauri::tray::TrayIconBuilder::with_id(TRAY_ID)
                .icon(app.default_window_icon().cloned().unwrap())
                .menu(&tray_menu)
                .show_menu_on_left_click(true)
                .on_tray_icon_event(|tray, event| match event {
                    tauri::tray::TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Right,
                        button_state: tauri::tray::MouseButtonState::Up,
                        ..
                    } => {
                        if EXPERIMENTAL_LAZY_TRAY_REFRESH {
                            let app = tray.app_handle().clone();
                            tauri::async_runtime::spawn(async move {
                                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                                if let Some(tray) = app.tray_by_id(TRAY_ID) {
                                    if let Ok(new_menu) = build_tray_menu(&app) {
                                        let _ = tray.set_menu(Some(new_menu));
                                    }
                                }
                            });
                        }
                    }
                    tauri::tray::TrayIconEvent::DoubleClick { .. } => {
                        if let Some(window) = tray.app_handle().get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    _ => {}
                })
                .on_menu_event(move |app, event| {
                    handle_tray_menu_event(app, &event.id.0);
                })
                .build(app)?;

            // Show or keep hidden based on settings
            if let Some(window) = app.get_webview_window("main") {
                if !settings.start_minimized {
                    let _ = window.show();
                }

                // Intercept window close → hide to tray instead of exiting
                let win = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = win.hide();
                    }
                });
            }

            log::info!("API Switch initialized");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::channel::list_channels,
            commands::channel::create_channel,
            commands::channel::update_channel,
            commands::channel::update_channel_response_ms,
            commands::channel::delete_channel,
            commands::channel::fetch_models,
            commands::channel::fetch_models_direct,
            commands::channel::probe_url,
            commands::channel::select_models,
            commands::pool::list_entries,
            commands::pool::toggle_entry,
            commands::pool::reorder_entries,
            commands::pool::delete_entry,
            commands::pool::create_entry,
            commands::pool::backfill_entry_catalog_meta,
            commands::pool::test_entry_latency,
            commands::pool::update_entry_response_ms,
            commands::token::list_access_keys,
            commands::token::create_access_key,
            commands::token::delete_access_key,
            commands::token::toggle_access_key,
            commands::usage::get_usage_logs,
            commands::usage::get_dashboard_stats,
            commands::usage::get_model_consumption,
            commands::usage::get_call_trend,
            commands::usage::get_model_distribution,
            commands::usage::get_model_ranking,
            commands::usage::get_user_ranking,
            commands::usage::get_user_trend,
            commands::config::get_settings,
            commands::config::update_settings,
            commands::config::check_update,
            commands::proxy_cmd::start_proxy,
            commands::proxy_cmd::stop_proxy,
            commands::proxy_cmd::get_proxy_status,
            commands::proxy_cmd::refresh_tray_menu,
            commands::test_chat::test_chat,
            commands::cli::set_user_env_vars,
            commands::cli::get_cli_data,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

pub(crate) fn build_tray_menu(app: &tauri::AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let app_state = app.state::<AppState>();
    let mut entries = app_state
        .db.get_enabled_entries_for_auto()
        .unwrap_or_default();
    let sort_mode = app_state
        .settings
        .try_read()
        .map(|settings| settings.default_sort_mode.clone())
        .unwrap_or_else(|_| {
            app_state
                .db
                .get_settings()
                .map(|settings| settings.default_sort_mode)
                .unwrap_or_else(|_| "custom".to_string())
        });
    proxy::apply_sort_mode(&mut entries, &sort_mode);
    let top5: Vec<_> = entries.into_iter().take(5).collect();

    // 1. Show main window (top of menu)
    let show_item = MenuItem::with_id(app, "show_main", "Open Main Window", true, None::<String>)?;
    let separator1 = PredefinedMenuItem::separator(app)?;

    // 2. CheckMenuItems for top 5 entries
    let check_items: Vec<CheckMenuItem<tauri::Wry>> = top5
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let checked = i == 0;
            let label = match &entry.channel_name {
                Some(ch) => format!("{} / {}", entry.display_name, ch),
                None => entry.display_name.clone(),
            };
            CheckMenuItem::with_id(app, &entry.id, &label, true, checked, None::<String>).unwrap()
        })
        .collect();

    // 3. Quit
    let separator2 = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Exit", true, None::<String>)?;

    // Assemble menu
    let mut all: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> = Vec::with_capacity(top5.len() + 4);
    all.push(&show_item as &dyn tauri::menu::IsMenuItem<_>);
    all.push(&separator1 as &dyn tauri::menu::IsMenuItem<_>);
    for item in &check_items {
        all.push(item);
    }
    all.push(&separator2 as &dyn tauri::menu::IsMenuItem<_>);
    all.push(&quit as &dyn tauri::menu::IsMenuItem<_>);

    Menu::with_items(app, &all)
}

fn handle_tray_menu_event(app: &tauri::AppHandle, event_id: &str) {
    log::info!("[tray] menu event: {event_id}");

    match event_id {
        "quit" => {
            app.exit(0);
        }
        "show_main" => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        _ => {
            // Provider entry click — set as top priority
            let entry_id = event_id.to_string();
            log::info!("[tray] setting priority for entry={entry_id}");

            // Update sort_index: set clicked entry to 0, increment others
            {
                let app_state = app.state::<AppState>();
                let guard = app_state.db.conn.lock();
                if let Ok(conn) = guard {
                    let now = chrono::Utc::now().timestamp();
                    let _ = conn.execute(
                        "UPDATE api_entries SET sort_index = sort_index + 1, updated_at = ?1 WHERE id != ?2",
                        rusqlite::params![now, entry_id],
                    );
                    let _ = conn.execute(
                        "UPDATE api_entries SET sort_index = 0, updated_at = ?1 WHERE id = ?2",
                        rusqlite::params![now, entry_id],
                    );
                }
            }

            // Rebuild tray menu with updated priority
            if let Ok(new_menu) = build_tray_menu(app) {
                if let Some(tray) = app.tray_by_id(TRAY_ID) {
                    let _ = tray.set_menu(Some(new_menu));
                }
            }

            // Notify frontend to refresh API Pool list
            let _ = app.emit("tray-priority-changed", ());
        }
    }
}
