use crate::error::AppError;
use crate::AppState;
use crate::proxy::protocol::get_adapter;
use serde::{Deserialize, Serialize};
use tauri::State;
use std::time::Instant;
use serde_json::json;

#[derive(Debug, Serialize)]
pub struct TestChatResponse {
    pub content: String,
    pub latency_ms: u64,
    pub usage: Option<TestChatUsage>,
}

#[derive(Debug, Serialize)]
pub struct TestChatUsage {
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TestChatMessage {
    pub role: String,
    pub content: String,
}

#[tauri::command]
pub async fn test_chat(
    state: State<'_, AppState>,
    entry_id: String,
    messages: Vec<TestChatMessage>,
) -> Result<TestChatResponse, AppError> {
    let db = state.db.clone();

    // Get the entry directly (all entries, not just enabled ones)
    let entries = db.get_entries_for_routing_all()?;
    let entry = entries
        .iter()
        .find(|e| e.id == entry_id)
        .ok_or_else(|| AppError::NotFound(format!("Entry {entry_id} not found")))?
        .clone();

    // Get channel info
    let channel = db.get_channel(&entry.channel_id)?;

    // Get protocol adapter
    let adapter = get_adapter(&channel.api_type);

    // Build URL and transform request
    let url = adapter.build_chat_url(&channel.base_url, &entry.model);
    let mut upstream_body = json!({
        "model": entry.model,
        "messages": messages,
        "stream": false,
    });
    adapter.transform_request(&mut upstream_body, &entry.model);

    let start = Instant::now();

    // Send request directly to upstream
    let client = reqwest::Client::new();
    let request = adapter.apply_auth(client.post(&url), &channel.api_key).json(&upstream_body);

    let response = request
        .send()
        .await
        .map_err(|e| AppError::Network(format!("Request failed: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::Proxy(format!("Upstream error {status}: {body}")));
    }

    let latency_ms = start.elapsed().as_millis() as u64;

    let json_body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to parse response: {e}")))?;

    // Transform response if needed (e.g. Claude → OpenAI format)
    let mut json_body = json_body;
    adapter.transform_response(&mut json_body);

    // Extract content
    let content = json_body
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();

    // Extract usage
    let usage = json_body.get("usage").map(|u| TestChatUsage {
        prompt_tokens: u.get("prompt_tokens").and_then(|v| v.as_i64()).unwrap_or(0),
        completion_tokens: u.get("completion_tokens").and_then(|v| v.as_i64()).unwrap_or(0),
        total_tokens: u.get("total_tokens").and_then(|v| v.as_i64()).unwrap_or(0),
    });

    Ok(TestChatResponse { content, latency_ms, usage })
}
