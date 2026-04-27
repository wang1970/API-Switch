use crate::database::{Channel, ModelInfo};
use crate::error::AppError;
use crate::proxy::protocol::get_adapter;
use crate::AppState;
use crate::TRAY_ID;
use crate::build_tray_menu;
use serde::{Deserialize, Serialize};
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
pub fn create_channel(state: State<'_, AppState>, params: CreateChannelParams) -> Result<Channel, AppError> {
    state.db.create_channel(
        &params.name,
        &params.api_type,
        &params.base_url,
        &params.api_key,
        params.notes.as_deref(),
    )
}

#[tauri::command]
pub fn update_channel(app: tauri::AppHandle, state: State<'_, AppState>, params: UpdateChannelParams) -> Result<Channel, AppError> {
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
pub fn delete_channel(app: tauri::AppHandle, state: State<'_, AppState>, id: String) -> Result<(), AppError> {
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
    let models = fetch_models_from_api(&channel.api_type, &channel.base_url, &channel.api_key).await?;

    // Update available_models in DB
    state.db.update_channel_models(&channel_id, &models, &channel.selected_models)?;

    Ok(models)
}

#[derive(Serialize)]
pub struct ProbeResult {
    pub reachable: bool,
    pub status_code: Option<u16>,
    pub latency_ms: u64,
    pub detected_type: Option<String>,
    pub message: String,
}

#[tauri::command]
pub async fn probe_url(url: String) -> Result<ProbeResult, AppError> {
    let url = url.trim_end_matches('/').trim();
    if url.is_empty() {
        return Ok(ProbeResult { reachable: false, status_code: None, latency_ms: 0,
            detected_type: None, message: "Empty URL".into() });
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|e| AppError::Network(format!("HTTP client: {e}")))?;

    let start = std::time::Instant::now();
    match client.head(url).send().await {
        Ok(r) => {
            let s = r.status().as_u16();
            let ms = start.elapsed().as_millis() as u64;
            Ok(ProbeResult { reachable: s < 500, status_code: Some(s), latency_ms: ms,
                detected_type: None, message: format!("{s} ({ms}ms)") })
        }
        Err(_) => {
            let _start2 = std::time::Instant::now();
            match client.get(url).send().await {
                Ok(r) => {
                    let s = r.status().as_u16();
                    let ms = start.elapsed().as_millis() as u64;
                    Ok(ProbeResult { reachable: s < 500, status_code: Some(s), latency_ms: ms,
                        detected_type: None, message: format!("{s} ({ms}ms)") })
                }
                Err(e) => {
                    let ms = start.elapsed().as_millis() as u64;
                    Ok(ProbeResult { reachable: false, status_code: None, latency_ms: ms,
                        detected_type: None, message: e.to_string() })
                }
            }
        }
    }
}

#[derive(Serialize)]
pub struct DetectApiResult {
    pub detected_type: Option<String>,
    pub models: Vec<ModelInfo>,
    pub message: String,
}

/// Try each API type against the given base_url to auto-detect the correct one.
/// Returns detected type and any models found during detection.
#[tauri::command]
pub async fn detect_api_type(
    base_url: String,
    api_key: String,
) -> Result<DetectApiResult, AppError> {
    let base_url = base_url.trim().trim_end_matches('/').to_string();
    if base_url.is_empty() {
        return Ok(DetectApiResult { detected_type: None, models: vec![], message: "Empty URL".into() });
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|e| AppError::Network(format!("HTTP client: {e}")))?;

    // 1. OpenAI / Custom: try /v1/models variants (most common)
    {
        let adapter = get_adapter("openai");
        let urls = build_models_url_variants(adapter.as_ref(), &base_url, &api_key);
        for url in &urls {
            match try_models_endpoint(&client, adapter.as_ref(), url, &api_key).await {
                Ok(models) if !models.is_empty() => {
                    let count = models.len();
                    let detected = if base_url.contains("api.openai.com") { "openai" } else { "custom" };
                    log::info!("[detect] OpenAI-compatible via {url}, type={detected} ({count} models)");
                    return Ok(DetectApiResult {
                        detected_type: Some(detected.into()),
                        models: dedup_models(models),
                        message: format!("Detected: {detected} ({count} models)"),
                    });
                }
                _ => {}
            }
        }
    }

    // 2. Gemini: try /v1beta/models with query auth
    {
        let adapter = get_adapter("gemini");
        let urls = build_models_url_variants(adapter.as_ref(), &base_url, &api_key);
        for url in &urls {
            match try_models_endpoint(&client, adapter.as_ref(), url, &api_key).await {
                Ok(models) if !models.is_empty() => {
                    let count = models.len();
                    log::info!("[detect] Gemini via {url} ({count} models)");
                    return Ok(DetectApiResult {
                        detected_type: Some("gemini".into()),
                        models: dedup_models(models),
                        message: format!("Detected: gemini ({count} models)"),
                    });
                }
                _ => {}
            }
        }
    }

    // 3. Claude/Anthropic: chat probe (no public models listing endpoint)
    {
        let adapter = get_adapter("claude");
        if let Some(models) = try_chat_probe(&client, adapter.as_ref(), &base_url, &api_key, "claude").await {
            let count = models.len();
            log::info!("[detect] Claude chat probe OK ({count} known models)");
            return Ok(DetectApiResult {
                detected_type: Some("claude".into()),
                models,
                message: format!("Detected: claude ({count} known models)"),
            });
        }
    }

    // 4. Azure: try Azure deployment endpoint
    {
        let adapter = get_adapter("azure");
        let urls = build_models_url_variants(adapter.as_ref(), &base_url, &api_key);
        for url in &urls {
            match try_models_endpoint(&client, adapter.as_ref(), url, &api_key).await {
                Ok(models) if !models.is_empty() => {
                    let count = models.len();
                    log::info!("[detect] Azure via {url} ({count} models)");
                    return Ok(DetectApiResult {
                        detected_type: Some("azure".into()),
                        models: dedup_models(models),
                        message: format!("Detected: azure ({count} models)"),
                    });
                }
                _ => {}
            }
        }
    }

    Ok(DetectApiResult {
        detected_type: None,
        models: vec![],
        message: "Could not detect API type. Please select manually.".into(),
    })
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
    state.db.update_channel_models(&channel_id, &channel.available_models, &model_names)?;
    state.db.sync_entries_for_channel(&channel_id, &model_names)?;
    if let Ok(new_menu) = build_tray_menu(&app) {
        if let Some(tray) = app.tray_by_id(TRAY_ID) {
            let _ = tray.set_menu(Some(new_menu));
        }
    }
    Ok(())
}

/// Generate model list URL candidates: adapter standard + common variants
fn build_models_url_variants(
    adapter: &(dyn crate::proxy::protocol::ProtocolAdapter + Send + Sync),
    base_url: &str,
    api_key: &str,
) -> Vec<String> {
    let mut urls = vec![adapter.build_models_url(base_url, api_key)];
    let base = base_url.trim_end_matches('/');
    for v in &["/models", "/api/models", "/api/v1/models", "/v2/models"] {
        let u = format!("{base}{v}");
        if !urls.contains(&u) { urls.push(u); }
    }
    urls
}

/// Try a single models endpoint, return parsed models or error string
async fn try_models_endpoint(
    client: &reqwest::Client,
    adapter: &(dyn crate::proxy::protocol::ProtocolAdapter + Send + Sync),
    url: &str,
    api_key: &str,
) -> Result<Vec<ModelInfo>, String> {
    let resp = adapter.apply_auth(client.get(url), api_key)
        .send().await.map_err(|e| format!("{e}"))?;
    let status = resp.status();
    if !status.is_success() {
        // Try to extract model list from error body (some APIs include it)
        let body = resp.text().await.unwrap_or_default();
        if let Some(m) = extract_models_from_json(&body) {
            return Ok(m);
        }
        return Err(format!("HTTP {status}"));
    }
    let body: serde_json::Value = resp.json().await.map_err(|e| format!("{e}"))?;
    Ok(adapter.parse_models_response(&body).into_iter()
        .map(|(id, owned_by)| ModelInfo { name: id.clone(), id, owned_by })
        .collect())
}

/// Try to extract model list from a JSON body (even error responses)
fn extract_models_from_json(body: &str) -> Option<Vec<ModelInfo>> {
    let json: serde_json::Value = serde_json::from_str(body).ok()?;
    let arr = json.get("data")?.as_array()?;
    let models: Vec<ModelInfo> = arr.iter()
        .filter_map(|m| {
            let id = m.get("id")?.as_str()?.to_string();
            let owned = m.get("owned_by").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
            Some(ModelInfo { name: id.clone(), id, owned_by: Some(owned) })
        })
        .collect();
    if models.is_empty() { None } else { Some(models) }
}

/// Chat probe: send a tiny request to verify the API works
async fn try_chat_probe(
    client: &reqwest::Client,
    adapter: &(dyn crate::proxy::protocol::ProtocolAdapter + Send + Sync),
    base_url: &str,
    api_key: &str,
    api_type: &str,
) -> Option<Vec<ModelInfo>> {
    let test_model = match api_type {
        "claude" => "claude-3-5-sonnet-20241022",
        "gemini" => "gemini-2.0-flash",
        _ => "gpt-4o-mini",
    };
    let chat_url = adapter.build_chat_url(base_url, test_model);
    let body = serde_json::json!({"model": test_model, "messages": [{"role":"user","content":"hi"}], "max_tokens": 1});
    let req = adapter.apply_auth(
        client.post(&chat_url).header("Content-Type", "application/json"), api_key);
    match req.json(&body).send().await {
        Ok(resp) => {
            let s = resp.status().as_u16();
            if s < 500 {
                // Server responded → API works, return known models
                if let Ok(text) = resp.text().await {
                    if let Some(m) = extract_models_from_json(&text) { return Some(m); }
                }
                return Some(known_models_for_type(api_type));
            }
            None
        }
        Err(_) => None,
    }
}

/// Commonly known models per API type
fn known_models_for_type(api_type: &str) -> Vec<ModelInfo> {
    let list: &[(&str, &str)] = match api_type {
        "openai" => &[
            ("gpt-4o","openai"),("gpt-4o-mini","openai"),("gpt-4-turbo","openai"),
            ("gpt-3.5-turbo","openai"),("o1","openai"),("o1-mini","openai"),
            ("o1-preview","openai"),("o3-mini","openai"),("o4-mini","openai"),
        ],
        "claude" => &[
            ("claude-sonnet-4-20250514","anthropic"),("claude-3-5-sonnet-20241022","anthropic"),
            ("claude-3-5-haiku-20241022","anthropic"),("claude-3-opus-20240229","anthropic"),
        ],
        "gemini" => &[
            ("gemini-2.5-pro-preview-05-06","google"),("gemini-2.0-flash","google"),
            ("gemini-1.5-pro","google"),("gemini-1.5-flash","google"),
        ],
        "azure" => &[
            ("gpt-4o","azure"),("gpt-4o-mini","azure"),("gpt-4-turbo","azure"),
        ],
        _ => &[
            ("gpt-4o","openai"),("gpt-4o-mini","openai"),("gpt-3.5-turbo","openai"),
            ("claude-3-5-sonnet-20241022","anthropic"),("claude-3-5-haiku-20241022","anthropic"),
            ("gemini-2.0-flash","google"),("deepseek-chat","deepseek"),("deepseek-reasoner","deepseek"),
            ("qwen-turbo","alibaba"),("glm-4-flash","zhipu"),
        ],
    };
    list.iter().map(|&(name, owner)| ModelInfo {
        name: name.into(), id: name.into(), owned_by: Some(owner.into()),
    }).collect()
}

fn dedup_models(models: Vec<ModelInfo>) -> Vec<ModelInfo> {
    let mut seen = std::collections::HashSet::new();
    models.into_iter().filter(|m| seen.insert(m.name.clone())).collect()
}

/// Fetch models with multi-layer fallback:
/// 1. Adapter standard URL + common path variants
/// 2. Chat probe → verify API works → return known models
async fn fetch_models_from_api(
    api_type: &str,
    base_url: &str,
    api_key: &str,
) -> Result<Vec<ModelInfo>, AppError> {
    let adapter = get_adapter(api_type);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|e| AppError::Network(format!("HTTP client: {e}")))?;

    // Layer 1: Try adapter URL + common variants
    let urls = build_models_url_variants(adapter.as_ref(), base_url, api_key);
    let mut last_err = String::new();
    for url in &urls {
        match try_models_endpoint(&client, adapter.as_ref(), url, api_key).await {
            Ok(models) if !models.is_empty() => {
                log::info!("[fetch_models] OK via {url} ({} models)", models.len());
                return Ok(dedup_models(models));
            }
            Ok(_) => {}
            Err(e) => { last_err = e; }
        }
    }
    log::warn!("[fetch_models] All model endpoints failed. Last: {last_err}");

    // Layer 2: Chat probe
    if let Some(models) = try_chat_probe(&client, adapter.as_ref(), base_url, api_key, api_type).await {
        log::info!("[fetch_models] Chat probe OK, {} known models", models.len());
        return Ok(models);
    }

    Err(AppError::Network(format!(
        "Could not fetch models (tried {} endpoints). Last: {last_err}", urls.len()
    )))
}
