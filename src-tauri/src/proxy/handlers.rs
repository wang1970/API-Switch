use super::auth;
use super::forwarder;
use super::router;
use super::server::ProxyState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde_json::{json, Value};

/// Health check endpoint
pub async fn health_check() -> (StatusCode, Json<Value>) {
    (
        StatusCode::OK,
        Json(json!({
            "status": "healthy",
            "timestamp": chrono::Utc::now().to_rfc3339(),
        })),
    )
}

/// Handle /v1/chat/completions
pub async fn handle_chat_completions(
    State(state): State<ProxyState>,
    request: axum::extract::Request,
) -> Result<axum::response::Response, ProxyError> {
    let (parts, body) = request.into_parts();
    let headers = &parts.headers;

    // Extract Access Key
    let access_key = auth::extract_access_key(headers, &state).await.map_err(|err| match err {
        crate::error::AppError::Validation(_) => ProxyError::Unauthorized,
        other => ProxyError::from(other),
    })?;

    // Read request body
    let body_bytes = axum::body::to_bytes(body, 32 * 1024 * 1024)
        .await
        .map_err(|e| ProxyError::Internal(format!("Failed to read body: {e}")))?;

    let body: Value = serde_json::from_slice(&body_bytes)
        .map_err(|e| ProxyError::Internal(format!("Failed to parse JSON: {e}")))?;

    let requested_model = body
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("auto")
        .to_string();
    let requested_model = if requested_model.is_empty() { "auto".to_string() } else { requested_model };

    let is_stream = body
        .get("stream")
        .and_then(|s| s.as_bool())
        .unwrap_or(false);

    // Resolve target entries
    // - AUTO: only enabled entries enter the auto pool
    // - exact model name: ALL entries (including disabled) are routable
    let all_entries = state.db.get_entries_for_routing()?;
    let auto_entries = state.db.get_enabled_entries_for_auto()?;
    let sort_mode = state.settings.read().await.default_sort_mode.clone();
    let resolved = router::resolve(&requested_model, &all_entries, &auto_entries, &state.circuit_breakers, &sort_mode).await;

    if resolved.is_empty() {
        return Err(ProxyError::NoAvailableProvider(requested_model));
    }

    // Forward with retry
    forwarder::forward_with_retry(
        &state,
        &resolved,
        &body,
        headers,
        &requested_model,
        access_key.as_ref(),
        is_stream,
    )
    .await
}

/// Handle /v1/models - list ALL models from the pool (including disabled).
/// disabled only means "skip in AUTO", the model is still usable when requested by name.
pub async fn handle_list_models(
    State(state): State<ProxyState>,
) -> Result<Json<Value>, ProxyError> {
    let mut entries = state.db.get_entries_for_routing()?;
    let sort_mode = state.settings.read().await.default_sort_mode.clone();
    router::apply_sort_mode(&mut entries, &sort_mode);

    let models: Vec<Value> = entries
        .iter()
        .map(|e| {
            json!({
                "id": e.model,
                "object": "model",
                "owned_by": e.channel_name,
            })
        })
        .collect();

    Ok(Json(json!({
        "object": "list",
        "data": models,
    })))
}

#[derive(Debug, thiserror::Error)]
pub enum ProxyError {
    #[error("No available provider for model: {0}")]
    NoAvailableProvider(String),

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("All providers failed")]
    AllProvidersFailed,

    #[error("Upstream error {status}: {message}")]
    Upstream { status: u16, message: String },
}

impl IntoResponse for ProxyError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match &self {
            ProxyError::NoAvailableProvider(model) => (
                StatusCode::NOT_FOUND,
                format!("No available provider for model: {model}"),
            ),
            ProxyError::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized".to_string()),
            ProxyError::AllProvidersFailed => (
                StatusCode::BAD_GATEWAY,
                "All providers failed".to_string(),
            ),
            ProxyError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            ProxyError::Upstream { status, message } => {
                let code = StatusCode::from_u16(*status).unwrap_or(StatusCode::BAD_GATEWAY);
                (code, message.clone())
            }
        };

        let body = json!({
            "error": {
                "message": message,
                "type": "proxy_error",
                "code": status.as_u16(),
            }
        });

        (status, Json(body)).into_response()
    }
}

impl From<crate::error::AppError> for ProxyError {
    fn from(e: crate::error::AppError) -> Self {
        ProxyError::Internal(e.to_string())
    }
}
