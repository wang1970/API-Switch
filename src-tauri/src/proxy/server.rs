use super::circuit_breaker::CircuitBreaker;
use super::handlers;
use crate::database::Database;
use axum::Router;
use axum::routing::{get, post};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tauri::Emitter;
use tokio::sync::{oneshot, RwLock};
use tower_http::cors::{Any, CorsLayer};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyStatus {
    pub running: bool,
    pub address: String,
    pub port: i32,
}

/// Shared proxy state
#[derive(Clone)]
pub struct ProxyState {
    pub db: Arc<Database>,
    pub circuit_breakers: Arc<RwLock<HashMap<String, CircuitBreaker>>>,
    pub app_handle: tauri::AppHandle,
}

/// HTTP proxy server
pub struct ProxyServer {
    port: i32,
    state: ProxyState,
    shutdown_tx: Arc<RwLock<Option<oneshot::Sender<()>>>>,
}

impl ProxyServer {
    pub fn new(port: i32, db: Arc<Database>, app_handle: tauri::AppHandle) -> Self {
        let state = ProxyState {
            db,
            circuit_breakers: Arc::new(RwLock::new(HashMap::new())),
            app_handle,
        };

        Self {
            port,
            state,
            shutdown_tx: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn start(&self) -> Result<(), String> {
        if self.shutdown_tx.read().await.is_some() {
            return Err("Proxy already running".to_string());
        }

        let addr: SocketAddr = format!("0.0.0.0:{}", self.port)
            .parse()
            .map_err(|e| format!("Invalid address: {e}"))?;

        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);

        let app = Router::new()
            .route("/health", get(handlers::health_check))
            .route("/v1/chat/completions", post(handlers::handle_chat_completions))
            .route("/v1/models", get(handlers::handle_list_models))
            .layer(cors)
            .with_state(self.state.clone());

        let listener = tokio::net::TcpListener::bind(&addr)
            .await
            .map_err(|e| format!("Failed to bind: {e}"))?;

        log::info!("Proxy server started on {addr}");

        *self.shutdown_tx.write().await = Some(shutdown_tx);

        tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await
                .unwrap_or_else(|e| {
                    log::error!("Proxy server error: {e}");
                });

            log::info!("Proxy server stopped");
        });

        Ok(())
    }

    pub async fn stop(&self) -> Result<(), String> {
        if let Some(tx) = self.shutdown_tx.write().await.take() {
            let _ = tx.send(());
            Ok(())
        } else {
            Err("Proxy not running".to_string())
        }
    }

    pub fn get_status(&self) -> ProxyStatus {
        let running = self
            .shutdown_tx
            .try_read()
            .map(|guard| guard.is_some())
            .unwrap_or(true);

        ProxyStatus {
            running,
            address: "127.0.0.1".to_string(),
            port: self.port,
        }
    }
}
