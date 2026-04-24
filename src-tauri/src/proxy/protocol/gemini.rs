use super::{join_url, ProtocolAdapter};
/// Google Gemini protocol adapter.
///
/// Google provides an **OpenAI-compatible endpoint** at `/v1beta/openai/`:
///   - Chat: `{base_url}/v1beta/openai/chat/completions`
///   - Models: `{base_url}/v1beta/openai/models`
///   - Auth: `?key=<API_KEY>` query parameter
///
/// Through this endpoint, the request/response body is standard OpenAI format.
/// No body transformation is needed.
///
/// **Important**: The user should set `base_url` to
/// `https://generativelanguage.googleapis.com` (without any version path).
/// The adapter appends the full `/v1beta/openai/...` path.
///
/// References:
/// - https://ai.google.dev/gemini-api/docs/openai
/// - https://ai.google.dev/api/models
use serde_json::{json, Value};

pub struct GeminiAdapter;

impl ProtocolAdapter for GeminiAdapter {
    fn build_chat_url(&self, base_url: &str, _model: &str) -> String {
        // Google's OpenAI-compatible endpoint
        join_url(base_url, "v1beta/openai/chat/completions")
    }

    fn build_models_url(&self, base_url: &str, api_key: &str) -> String {
        // Gemini uses query-param auth for all requests
        format!(
            "{}?key={}",
            join_url(base_url, "v1beta/openai/models"),
            api_key
        )
    }

    fn uses_query_auth(&self) -> bool {
        true
    }

    fn build_auth_headers(&self, _api_key: &str) -> Vec<(String, String)> {
        vec![] // Gemini uses query-param auth exclusively
    }

    fn apply_auth(
        &self,
        builder: reqwest::RequestBuilder,
        api_key: &str,
    ) -> reqwest::RequestBuilder {
        builder.query(&[("key", api_key)])
    }

    fn transform_request(&self, body: &mut Value, actual_model: &str) {
        if let Some(obj) = body.as_object_mut() {
            obj.insert("model".to_string(), Value::String(actual_model.to_string()));
        }
    }

    fn transform_response(&self, _body: &mut Value) {
        // Google's OpenAI-compatible endpoint returns standard OpenAI format.
        // No transformation needed.
    }

    fn needs_sse_transform(&self) -> bool {
        false
    }

    fn extract_sse_usage(&self, data_line: &str) -> (i64, i64) {
        if data_line == "[DONE]" {
            return (0, 0);
        }
        let Ok(value) = serde_json::from_str::<Value>(data_line) else {
            return (0, 0);
        };
        let prompt = value
            .get("usage")
            .and_then(|u| u.get("prompt_tokens"))
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let completion = value
            .get("usage")
            .and_then(|u| u.get("completion_tokens"))
            .and_then(Value::as_i64)
            .unwrap_or(0);
        (prompt, completion)
    }

    fn transform_sse_line(&self, data_line: &str) -> Option<String> {
        // Should never be called (needs_sse_transform = false).
        Some(data_line.to_string())
    }

    fn parse_models_response(&self, body: &Value) -> Vec<(String, Option<String>)> {
        // Google's OpenAI-compatible endpoint returns standard OpenAI format:
        // { data: [{ id: "gemini-1.5-pro", owned_by: "google", ... }] }
        body.get("data")
            .and_then(|d| d.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| {
                        let id = m.get("id")?.as_str()?.to_string();
                        let owned_by = m.get("owned_by").and_then(|v| v.as_str()).map(String::from);
                        Some((id, owned_by))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

// ==================== Native Gemini format conversion ====================
//
// The code below handles Gemini's *native* API format, used when the user
// sets base_url to the native endpoint rather than the OpenAI-compatible one.
// This is kept as a fallback / future option — it is NOT used in the current
// trait implementation which targets the OpenAI-compatible endpoint.

/// Build URL for Gemini native `generateContent` endpoint.
/// Format: `{base_url}/v1beta/models/{model}:generateContent`
pub fn build_gemini_native_chat_url(base_url: &str, model: &str) -> String {
    join_url(
        base_url,
        &format!("v1beta/models/{}:generateContent", model),
    )
}

/// Build URL for Gemini native `streamGenerateContent` endpoint.
/// Format: `{base_url}/v1beta/models/{model}:streamGenerateContent?alt=sse`
pub fn build_gemini_native_stream_url(base_url: &str, model: &str) -> String {
    join_url(
        base_url,
        &format!("v1beta/models/{}:streamGenerateContent?alt=sse", model),
    )
}

/// Build URL for Gemini native model listing.
/// Format: `{base_url}/v1beta/models?key=<api_key>`
pub fn build_gemini_native_models_url(base_url: &str, api_key: &str) -> String {
    format!("{}?key={}", join_url(base_url, "v1beta/models"), api_key)
}

/// Parse Gemini native `listModels` response.
/// Format: { models: [{ name: "models/gemini-pro", displayName: "Gemini Pro", ... }] }
pub fn parse_gemini_native_models(body: &Value) -> Vec<(String, Option<String>)> {
    body.get("models")
        .and_then(|d| d.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|m| {
                    let name = m.get("name")?.as_str()?.to_string();
                    // Strip "models/" prefix → "gemini-pro"
                    let id = name.strip_prefix("models/").unwrap_or(&name).to_string();
                    let display_name = m
                        .get("displayName")
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    Some((id, display_name))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Transform an OpenAI-format request into Gemini native format.
///
/// OpenAI format:
/// ```json
/// { "model": "...", "messages": [...], "stream": true, "temperature": 0.7, "max_tokens": 1024 }
/// ```
///
/// Gemini format:
/// ```json
/// { "contents": [{"role": "user", "parts": [{"text": "..."}]}], "generationConfig": {"temperature": 0.7, "maxOutputTokens": 1024} }
/// ```
pub fn transform_request_to_gemini(body: &mut Value, actual_model: &str) {
    let Some(obj) = body.as_object_mut() else {
        return;
    };

    // Extract system instruction
    let mut system_instruction = None;
    let mut contents = Vec::new();

    if let Some(msgs) = obj.remove("messages").and_then(|v| v.as_array().cloned()) {
        for msg in msgs {
            let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("user");
            match role {
                "system" => {
                    // Gemini puts system in top-level "systemInstruction"
                    if let Some(content) = msg.get("content") {
                        let text = extract_gemini_text(content);
                        if !text.is_empty() {
                            system_instruction = Some(json!({"parts": [{"text": text}]}));
                        }
                    }
                }
                _ => {
                    contents.push(convert_message_to_gemini(&msg));
                }
            }
        }
    }

    // Build Gemini request
    let mut gemini = json!({
        "contents": contents,
        "model": actual_model,
    });

    if let Some(sys) = system_instruction {
        gemini["systemInstruction"] = sys;
    }

    // generationConfig mapping
    let mut gen_config = json!({});
    if let Some(temp) = obj.remove("temperature") {
        gen_config["temperature"] = temp;
    }
    if let Some(top_p) = obj.remove("top_p") {
        gen_config["topP"] = top_p;
    }
    if let Some(max_tokens) = obj.remove("max_tokens") {
        gen_config["maxOutputTokens"] = max_tokens;
    }
    if let Some(stop) = obj.remove("stop") {
        gen_config["stopSequences"] = stop;
    }
    if !gen_config.as_object().map(|o| o.is_empty()).unwrap_or(true) {
        gemini["generationConfig"] = gen_config;
    }

    // Stream flag
    if let Some(stream) = obj.remove("stream") {
        gemini["stream"] = stream;
    }

    // Tools (function calling) — Gemini uses a different format
    if let Some(tools) = obj.remove("tools") {
        let converted = convert_tools_to_gemini(&tools);
        if !converted.as_array().map(|a| a.is_empty()).unwrap_or(true) {
            gemini["tools"] = converted;
        }
    }

    *body = gemini;
}

/// Transform Gemini native response into OpenAI format.
pub fn transform_response_from_gemini(body: &mut Value) {
    let Some(obj) = body.as_object_mut() else {
        return;
    };

    let model = obj
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("gemini")
        .to_string();

    // Extract text from Gemini's "candidates[].content.parts[].text"
    let mut text_parts = Vec::new();
    let mut finish_reason = String::from("stop");

    if let Some(candidates) = obj.get("candidates").and_then(|c| c.as_array()) {
        if let Some(first) = candidates.first() {
            // finishReason: "STOP" → "stop", "MAX_TOKENS" → "length", "SAFETY" → "content_filter"
            finish_reason = match first
                .get("finishReason")
                .and_then(|r| r.as_str())
                .unwrap_or("STOP")
            {
                "STOP" => "stop".to_string(),
                "MAX_TOKENS" => "length".to_string(),
                "SAFETY" => "content_filter".to_string(),
                "RECITATION" => "content_filter".to_string(),
                other => other.to_lowercase(),
            };

            if let Some(content) = first.get("content") {
                if let Some(parts) = content.get("parts").and_then(|p| p.as_array()) {
                    for part in parts {
                        if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                            text_parts.push(text.to_string());
                        }
                    }
                }
            }
        }
    }

    // Token usage — Gemini uses "usageMetadata.promptTokenCount" / "candidatesTokenCount"
    let prompt_tokens = obj
        .get("usageMetadata")
        .and_then(|u| u.get("promptTokenCount"))
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let completion_tokens = obj
        .get("usageMetadata")
        .and_then(|u| u.get("candidatesTokenCount"))
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let total_tokens = obj
        .get("usageMetadata")
        .and_then(|u| u.get("totalTokenCount"))
        .and_then(Value::as_i64)
        .unwrap_or(prompt_tokens + completion_tokens);

    *body = json!({
        "id": format!("chatcmpl-gemini-{}", chrono::Utc::now().timestamp_millis()),
        "object": "chat.completion",
        "created": chrono::Utc::now().timestamp(),
        "model": model,
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": text_parts.join("")
            },
            "finish_reason": finish_reason,
        }],
        "usage": {
            "prompt_tokens": prompt_tokens,
            "completion_tokens": completion_tokens,
            "total_tokens": total_tokens,
        }
    });
}

/// Transform a Gemini native SSE line into OpenAI SSE format.
/// Gemini streaming returns JSON objects (not `data: ` prefixed).
/// Each object: `{ "candidates": [...], "usageMetadata": {...} }`
pub fn transform_gemini_sse_line(data_line: &str) -> Option<String> {
    let Ok(value) = serde_json::from_str::<Value>(data_line) else {
        return None;
    };

    let model = value
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("gemini")
        .to_string();

    // Extract text from candidates[0].content.parts[].text
    let mut text = String::new();
    let mut finish_reason = None;

    if let Some(candidates) = value.get("candidates").and_then(|c| c.as_array()) {
        if let Some(first) = candidates.first() {
            let fr = first
                .get("finishReason")
                .and_then(|r| r.as_str())
                .unwrap_or("");
            if !fr.is_empty() {
                finish_reason = Some(match fr {
                    "STOP" => "stop".to_string(),
                    "MAX_TOKENS" => "length".to_string(),
                    "SAFETY" => "content_filter".to_string(),
                    other => other.to_lowercase(),
                });
            }
            if let Some(content) = first.get("content") {
                if let Some(parts) = content.get("parts").and_then(|p| p.as_array()) {
                    for part in parts {
                        if let Some(t) = part.get("text").and_then(|v| v.as_str()) {
                            text.push_str(t);
                        }
                    }
                }
            }
        }
    }

    let mut chunk = json!({
        "id": format!("chatcmpl-gemini-{}", chrono::Utc::now().timestamp_millis()),
        "object": "chat.completion.chunk",
        "created": chrono::Utc::now().timestamp(),
        "model": model,
        "choices": [{
            "index": 0,
            "delta": {},
            "finish_reason": null
        }]
    });

    if !text.is_empty() {
        chunk["choices"][0]["delta"]["content"] = json!(text);
    }

    if let Some(fr) = finish_reason {
        chunk["choices"][0]["finish_reason"] = json!(fr);
    }

    // Usage if present in the last chunk
    if let Some(usage) = value.get("usageMetadata") {
        chunk["usage"] = json!({
            "prompt_tokens": usage.get("promptTokenCount").and_then(Value::as_i64).unwrap_or(0),
            "completion_tokens": usage.get("candidatesTokenCount").and_then(Value::as_i64).unwrap_or(0),
            "total_tokens": usage.get("totalTokenCount").and_then(Value::as_i64).unwrap_or(0),
        });
    }

    Some(serde_json::to_string(&chunk).unwrap())
}

// ── Gemini native conversion helpers ──────────────────────────────────

fn extract_gemini_text(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(arr) => arr
            .iter()
            .filter_map(|part| {
                if part.get("type")?.as_str()? == "text" {
                    part.get("text")?.as_str().map(String::from)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    }
}

fn convert_message_to_gemini(msg: &Value) -> Value {
    let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("user");
    let content = msg.get("content");

    // Gemini uses "model" role for assistant
    let gemini_role = match role {
        "assistant" => "model",
        _ => "user",
    };

    match content {
        None => json!({"role": gemini_role, "parts": []}),
        Some(Value::String(s)) => json!({"role": gemini_role, "parts": [{"text": s}]}),
        Some(Value::Array(arr)) => {
            let mut parts = Vec::new();

            for part in arr {
                let part_type = part.get("type").and_then(|t| t.as_str()).unwrap_or("");
                match part_type {
                    "text" => {
                        if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                            parts.push(json!({"text": text}));
                        }
                    }
                    "image_url" => {
                        // Convert base64 data URLs to Gemini inline data
                        if let Some(url) = part
                            .get("image_url")
                            .and_then(|u| u.get("url"))
                            .and_then(|v| v.as_str())
                        {
                            if let Some(data) = url.strip_prefix("data:") {
                                let items: Vec<&str> = data.splitn(2, ";base64,").collect();
                                if items.len() == 2 {
                                    parts.push(json!({
                                        "inlineData": {
                                            "mimeType": items[0],
                                            "data": items[1]
                                        }
                                    }));
                                }
                            }
                        }
                    }
                    "tool_call_id" => {
                        // OpenAI tool result → Gemini functionResponse
                        if role == "tool" {
                            let func_resp = json!({
                                "functionResponse": {
                                    "name": msg.get("tool_call_id").and_then(|v| v.as_str()).unwrap_or(""),
                                    "response": extract_gemini_text(content.expect("tool result content"))
                                }
                            });
                            return json!({"role": "user", "parts": [func_resp]});
                        }
                    }
                    _ => {}
                }
            }

            // Handle tool_calls in assistant messages
            if role == "assistant" {
                if let Some(tcs) = msg.get("tool_calls").and_then(|v| v.as_array()) {
                    let mut fc_parts = Vec::new();
                    for tc in tcs {
                        let fn_body = tc.get("function");
                        if let (Some(name), Some(args_str)) = (
                            fn_body.and_then(|f| f.get("name")).and_then(|n| n.as_str()),
                            fn_body
                                .and_then(|f| f.get("arguments"))
                                .and_then(|a| a.as_str()),
                        ) {
                            let args: Value = serde_json::from_str(args_str).unwrap_or(json!({}));
                            fc_parts.push(json!({
                                "functionCall": {
                                    "name": name,
                                    "args": args
                                }
                            }));
                        }
                    }
                    // Text parts first, then function calls
                    let mut all_parts = parts;
                    all_parts.extend(fc_parts);
                    return json!({"role": "model", "parts": all_parts});
                }
            }

            json!({"role": gemini_role, "parts": parts})
        }
        _ => json!({"role": gemini_role, "parts": []}),
    }
}

fn convert_tools_to_gemini(openai_tools: &Value) -> Value {
    let Some(tools_arr) = openai_tools.as_array() else {
        return json!([]);
    };

    let declarations: Vec<Value> = tools_arr
        .iter()
        .filter_map(|tool| {
            let func = tool.get("function")?;
            let name = func.get("name")?.as_str()?;
            let description = func
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("");
            let parameters = func.get("parameters").cloned().unwrap_or(json!({}));
            Some(json!({
                "name": name,
                "description": description,
                "parameters": parameters
            }))
        })
        .collect();

    json!([{"functionDeclarations": declarations}])
}
