/// Protocol adapter module.
///
/// Each API type (openai, claude, gemini, azure, custom) has its own adapter file.
/// A shared [`ProtocolAdapter`] trait defines the interface; [`get_adapter()`] returns
/// the concrete implementation for a given `api_type` string.
///
/// Callers never match on `api_type` themselves — they go through the trait.
mod azure;
mod claude;
mod common;
mod custom;
mod gemini;
mod openai;

pub use common::join_url;

use serde_json::Value;

/// Every API type must implement this trait.
pub trait ProtocolAdapter {
    // ── URL building ────────────────────────────────────────────────
    /// Build the full URL for a chat completions request.
    /// `base_url` is the user-provided base URL, `model` is the resolved model name.
    fn build_chat_url(&self, base_url: &str, model: &str) -> String;

    /// Build the full URL for a models list request.
    /// May include query params (e.g. Gemini `?key=...`, Azure `?api-version=...`).
    fn build_models_url(&self, base_url: &str, api_key: &str) -> String;

    // ── Authentication ─────────────────────────────────────────────
    /// Whether this type authenticates via URL query parameter instead of headers.
    fn uses_query_auth(&self) -> bool;

    /// Return auth headers as `(name, value)` pairs.
    fn build_auth_headers(&self, api_key: &str) -> Vec<(String, String)>;

    /// Apply auth to an existing `reqwest::RequestBuilder`.
    fn apply_auth(
        &self,
        builder: reqwest::RequestBuilder,
        api_key: &str,
    ) -> reqwest::RequestBuilder;

    // ── Request body ───────────────────────────────────────────────
    /// Transform an OpenAI-format request into this type's upstream format.
    /// `actual_model` is the resolved model name from the API entry.
    fn transform_request(&self, body: &mut Value, actual_model: &str);

    // ── Non-streaming response ─────────────────────────────────────
    /// Transform an upstream JSON response into OpenAI format (in-place).
    fn transform_response(&self, body: &mut Value);

    // ── Streaming (SSE) ────────────────────────────────────────────
    /// Whether the SSE stream needs transformation (false = passthrough original bytes).
    fn needs_sse_transform(&self) -> bool;

    /// Extract `(prompt_tokens, completion_tokens)` from a single SSE data line.
    fn extract_sse_usage(&self, data_line: &str) -> (i64, i64);

    /// Transform a single upstream SSE `data: ...` line into OpenAI SSE format.
    /// Returns `None` to drop the line.
    /// Only called when [`needs_sse_transform`] returns `true`.
    fn transform_sse_line(&self, data_line: &str) -> Option<String>;

    // ── Models list parsing ────────────────────────────────────────
    /// Parse a models list response into `[(model_id, owned_by)]`.
    fn parse_models_response(&self, body: &Value) -> Vec<(String, Option<String>)>;
}

/// Return the adapter for a given `api_type` string.
/// Unknown types fall back to the OpenAI adapter (broad compatibility).
pub fn get_adapter(api_type: &str) -> Box<dyn ProtocolAdapter + Send + Sync> {
    match api_type {
        "claude" | "anthropic" => Box::new(claude::ClaudeAdapter),
        "gemini" => Box::new(gemini::GeminiAdapter),
        "azure" => Box::new(azure::AzureAdapter),
        "custom" => Box::new(custom::CustomAdapter),
        _ => Box::new(openai::OpenAiAdapter), // openai + anything else
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ─── Helper ───────────────────────────────────────────────────────

    fn parse_json(s: &str) -> serde_json::Value {
        serde_json::from_str(s).expect("valid JSON")
    }

    // ================================================================
    //  OpenAI Adapter
    // ================================================================

    #[test]
    fn openai_chat_url_basic() {
        let a = openai::OpenAiAdapter;
        assert_eq!(
            a.build_chat_url("https://api.openai.com", "gpt-4"),
            "https://api.openai.com/v1/chat/completions"
        );
    }

    #[test]
    fn openai_chat_url_dedup_v1() {
        let a = openai::OpenAiAdapter;
        assert_eq!(
            a.build_chat_url("https://api.openai.com/v1", "gpt-4"),
            "https://api.openai.com/v1/chat/completions"
        );
    }

    #[test]
    fn openai_chat_url_dedup_trailing_slash() {
        let a = openai::OpenAiAdapter;
        assert_eq!(
            a.build_chat_url("https://api.openai.com/", "gpt-4"),
            "https://api.openai.com/v1/chat/completions"
        );
    }

    #[test]
    fn openai_models_url() {
        let a = openai::OpenAiAdapter;
        assert_eq!(
            a.build_models_url("https://api.openai.com", "sk-test"),
            "https://api.openai.com/v1/models"
        );
    }

    #[test]
    fn openai_auth_headers() {
        let a = openai::OpenAiAdapter;
        let headers = a.build_auth_headers("sk-12345");
        assert_eq!(headers.len(), 1);
        assert_eq!(
            headers[0],
            ("Authorization".into(), "Bearer sk-12345".into())
        );
    }

    #[test]
    fn openai_transform_request_sets_model() {
        let a = openai::OpenAiAdapter;
        let mut body = json!({
            "model": "auto",
            "messages": [{"role": "user", "content": "hello"}]
        });
        a.transform_request(&mut body, "gpt-4o");
        assert_eq!(body["model"], "gpt-4o");
        assert_eq!(body["messages"][0]["content"], "hello");
    }

    #[test]
    fn openai_transform_response_passthrough() {
        let a = openai::OpenAiAdapter;
        let mut body = json!({
            "id": "chatcmpl-123",
            "object": "chat.completion",
            "choices": [{"message": {"role": "assistant", "content": "Hi"}}],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5}
        });
        let original = body.clone();
        a.transform_response(&mut body);
        assert_eq!(body, original);
    }

    #[test]
    fn openai_needs_sse_transform() {
        let a = openai::OpenAiAdapter;
        assert!(!a.needs_sse_transform());
    }

    #[test]
    fn openai_extract_sse_usage() {
        let a = openai::OpenAiAdapter;
        assert_eq!(a.extract_sse_usage("[DONE]"), (0, 0));
        assert_eq!(a.extract_sse_usage("invalid json"), (0, 0));

        let line =
            r#"{"id":"1","choices":[],"usage":{"prompt_tokens":100,"completion_tokens":50}}"#;
        assert_eq!(a.extract_sse_usage(line), (100, 50));
    }

    #[test]
    fn openai_parse_models_response() {
        let a = openai::OpenAiAdapter;
        let body = json!({
            "data": [
                {"id": "gpt-4o", "owned_by": "openai"},
                {"id": "gpt-3.5-turbo"}
            ]
        });
        let models = a.parse_models_response(&body);
        assert_eq!(models.len(), 2);
        assert_eq!(models[0], ("gpt-4o".into(), Some("openai".into())));
        assert_eq!(models[1], ("gpt-3.5-turbo".into(), None));
    }

    // ================================================================
    //  Claude (Anthropic) Adapter
    // ================================================================

    #[test]
    fn claude_chat_url_basic() {
        let a = claude::ClaudeAdapter;
        assert_eq!(
            a.build_chat_url("https://api.anthropic.com", "claude-3-opus"),
            "https://api.anthropic.com/v1/messages"
        );
    }

    #[test]
    fn claude_chat_url_dedup_v1() {
        let a = claude::ClaudeAdapter;
        assert_eq!(
            a.build_chat_url("https://api.anthropic.com/v1", "claude-3-opus"),
            "https://api.anthropic.com/v1/messages"
        );
    }

    #[test]
    fn claude_models_url() {
        let a = claude::ClaudeAdapter;
        assert_eq!(
            a.build_models_url("https://api.anthropic.com", "sk-ant-test"),
            "https://api.anthropic.com/v1/models"
        );
    }

    #[test]
    fn claude_auth_headers() {
        let a = claude::ClaudeAdapter;
        let headers = a.build_auth_headers("sk-ant-12345");
        assert_eq!(headers.len(), 3);
        assert_eq!(headers[0], ("x-api-key".into(), "sk-ant-12345".into()));
        assert_eq!(
            headers[1],
            ("anthropic-version".into(), "2023-06-01".into())
        );
        assert_eq!(
            headers[2],
            (
                "anthropic-dangerous-direct-browser-access".into(),
                "true".into()
            )
        );
    }

    #[test]
    fn claude_transform_request_basic() {
        let a = claude::ClaudeAdapter;
        let mut body = json!({
            "model": "auto",
            "messages": [
                {"role": "system", "content": "You are helpful."},
                {"role": "user", "content": "hello"}
            ],
            "max_tokens": 1024,
            "temperature": 0.7
        });
        a.transform_request(&mut body, "claude-3-opus");

        assert_eq!(body["model"], "claude-3-opus");
        assert_eq!(body["system"], "You are helpful.");
        assert_eq!(body["messages"].as_array().unwrap().len(), 1);
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["max_tokens"], 1024);
        assert_eq!(body["temperature"], 0.7);
        assert!(body.get("stream").is_none());
    }

    #[test]
    fn claude_transform_request_with_stream() {
        let a = claude::ClaudeAdapter;
        let mut body = json!({
            "model": "auto",
            "messages": [{"role": "user", "content": "hi"}],
            "stream": true
        });
        a.transform_request(&mut body, "claude-3-sonnet");
        assert_eq!(body["stream"], true);
    }

    #[test]
    fn claude_transform_request_with_stop() {
        let a = claude::ClaudeAdapter;
        let mut body = json!({
            "model": "auto",
            "messages": [{"role": "user", "content": "hi"}],
            "stop": ["\n"]
        });
        a.transform_request(&mut body, "claude-3-sonnet");
        assert_eq!(body["stop_sequences"], json!(["\n"]));
        assert!(body.get("stop").is_none());
    }

    #[test]
    fn claude_transform_request_with_tools() {
        let a = claude::ClaudeAdapter;
        let mut body = json!({
            "model": "auto",
            "messages": [{"role": "user", "content": "What's the weather?"}],
            "tools": [{
                "type": "function",
                "function": {
                    "name": "get_weather",
                    "description": "Get weather",
                    "parameters": {"type": "object", "properties": {"city": {"type": "string"}}}
                }
            }]
        });
        a.transform_request(&mut body, "claude-3-opus");

        let tools = body["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["name"], "get_weather");
        assert_eq!(tools[0]["input_schema"]["type"], "object");
        assert!(tools[0].get("function").is_none());
    }

    #[test]
    fn claude_transform_request_assistant_with_tool_calls() {
        let a = claude::ClaudeAdapter;
        let mut body = json!({
            "model": "auto",
            "messages": [
                {"role": "user", "content": "What's the weather?"},
                {
                    "role": "assistant",
                    "content": "Let me check.",
                    "tool_calls": [{
                        "id": "call_abc",
                        "type": "function",
                        "function": {"name": "get_weather", "arguments": "{\"city\":\"Tokyo\"}"}
                    }]
                },
                {
                    "role": "tool",
                    "tool_call_id": "call_abc",
                    "content": "Sunny, 25°C"
                }
            ],
            "max_tokens": 1024
        });
        a.transform_request(&mut body, "claude-3-opus");

        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 3);

        assert_eq!(msgs[0]["role"], "user");
        assert_eq!(msgs[0]["content"], "What's the weather?");

        assert_eq!(msgs[1]["role"], "assistant");
        let content = msgs[1]["content"].as_array().unwrap();
        let tool_use_block = content.iter().find(|b| b["type"] == "tool_use").unwrap();
        assert_eq!(tool_use_block["id"], "call_abc");
        assert_eq!(tool_use_block["name"], "get_weather");
        assert_eq!(tool_use_block["input"]["city"], "Tokyo");

        assert_eq!(msgs[2]["role"], "user");
        let tool_result = &msgs[2]["content"].as_array().unwrap()[0];
        assert_eq!(tool_result["type"], "tool_result");
        assert_eq!(tool_result["tool_use_id"], "call_abc");
        assert_eq!(tool_result["content"], "Sunny, 25°C");
    }

    #[test]
    fn claude_transform_response_basic() {
        let a = claude::ClaudeAdapter;
        let mut body = json!({
            "id": "msg_123",
            "type": "message",
            "role": "assistant",
            "model": "claude-3-opus-20240229",
            "content": [
                {"type": "text", "text": "Hello!"}
            ],
            "stop_reason": "end_turn",
            "usage": {
                "input_tokens": 25,
                "output_tokens": 10
            }
        });
        a.transform_response(&mut body);

        assert_eq!(body["object"], "chat.completion");
        assert_eq!(body["model"], "claude-3-opus-20240229");
        assert_eq!(body["choices"][0]["message"]["content"], "Hello!");
        assert_eq!(body["choices"][0]["finish_reason"], "stop");
        assert_eq!(body["usage"]["prompt_tokens"], 25);
        assert_eq!(body["usage"]["completion_tokens"], 10);
        assert_eq!(body["usage"]["total_tokens"], 35);
    }

    #[test]
    fn claude_transform_response_max_tokens() {
        let a = claude::ClaudeAdapter;
        let mut body = json!({
            "id": "msg_456",
            "role": "assistant",
            "model": "claude-3-opus",
            "content": [{"type": "text", "text": "I can help with that."}],
            "stop_reason": "max_tokens",
            "usage": {"input_tokens": 50, "output_tokens": 4096}
        });
        a.transform_response(&mut body);
        assert_eq!(body["choices"][0]["finish_reason"], "length");
    }

    #[test]
    fn claude_transform_response_tool_use() {
        let a = claude::ClaudeAdapter;
        let mut body = json!({
            "id": "msg_789",
            "role": "assistant",
            "model": "claude-3-opus",
            "content": [
                {"type": "text", "text": "Let me check."},
                {"type": "tool_use", "id": "toolu_01", "name": "get_weather", "input": {"city": "Tokyo"}}
            ],
            "stop_reason": "tool_use",
            "usage": {"input_tokens": 100, "output_tokens": 50}
        });
        a.transform_response(&mut body);

        assert_eq!(body["choices"][0]["finish_reason"], "tool_calls");
        let tool_calls = body["choices"][0]["message"]["tool_calls"]
            .as_array()
            .unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0]["id"], "toolu_01");
        assert_eq!(tool_calls[0]["type"], "function");
        assert_eq!(tool_calls[0]["function"]["name"], "get_weather");
        assert_eq!(
            tool_calls[0]["function"]["arguments"],
            serde_json::to_string(&json!({"city": "Tokyo"})).unwrap()
        );
    }

    #[test]
    fn claude_transform_response_multiple_text_blocks() {
        let a = claude::ClaudeAdapter;
        let mut body = json!({
            "id": "msg_multi",
            "role": "assistant",
            "model": "claude-3-opus",
            "content": [
                {"type": "text", "text": "Hello"},
                {"type": "text", "text": " world"}
            ],
            "stop_reason": "end_turn",
            "usage": {"input_tokens": 10, "output_tokens": 5}
        });
        a.transform_response(&mut body);
        assert_eq!(body["choices"][0]["message"]["content"], "Hello world");
    }

    #[test]
    fn claude_needs_sse_transform() {
        let a = claude::ClaudeAdapter;
        assert!(a.needs_sse_transform());
    }

    #[test]
    fn claude_sse_message_start() {
        let a = claude::ClaudeAdapter;
        let line = r#"{"type":"message_start","message":{"id":"msg_abc123","model":"claude-3-opus","role":"assistant","content":[],"usage":{"input_tokens":25}}}"#;
        let result = a.transform_sse_line(line).unwrap();
        let v: serde_json::Value = parse_json(&result);
        assert_eq!(v["object"], "chat.completion.chunk");
        assert_eq!(v["model"], "claude-3-opus");
        assert_eq!(v["id"], "msg_abc123");
        assert_eq!(v["choices"][0]["delta"]["role"], "assistant");
    }

    #[test]
    fn claude_sse_content_block_start_text() {
        let a = claude::ClaudeAdapter;
        let line = r#"{"type":"content_block_start","index":0,"content_block":{"type":"text","text":"Hello"}}"#;
        let result = a.transform_sse_line(line).unwrap();
        let v: serde_json::Value = parse_json(&result);
        assert_eq!(v["choices"][0]["delta"]["content"], "Hello");
    }

    #[test]
    fn claude_sse_text_delta() {
        let a = claude::ClaudeAdapter;
        let line = r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":" world"}}"#;
        let result = a.transform_sse_line(line).unwrap();
        let v: serde_json::Value = parse_json(&result);
        assert_eq!(v["choices"][0]["delta"]["content"], " world");
    }

    #[test]
    fn claude_sse_empty_text_delta_dropped() {
        let a = claude::ClaudeAdapter;
        let line =
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":""}}"#;
        assert!(a.transform_sse_line(line).is_none());
    }

    #[test]
    fn claude_sse_tool_use_start() {
        let a = claude::ClaudeAdapter;
        let line = r#"{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu_abc","name":"get_weather"}}"#;
        let result = a.transform_sse_line(line).unwrap();
        let v: serde_json::Value = parse_json(&result);
        assert_eq!(v["choices"][0]["delta"]["role"], "assistant");
        let tc = &v["choices"][0]["delta"]["tool_calls"][0];
        assert_eq!(tc["id"], "toolu_abc");
        assert_eq!(tc["function"]["name"], "get_weather");
        assert_eq!(tc["function"]["arguments"], "");
    }

    #[test]
    fn claude_sse_tool_use_delta() {
        let a = claude::ClaudeAdapter;
        let line = r#"{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"city\":"}}"#;
        let result = a.transform_sse_line(line).unwrap();
        let v: serde_json::Value = parse_json(&result);
        assert_eq!(v["choices"][0]["delta"]["tool_calls"][0]["index"], 1);
        assert_eq!(
            v["choices"][0]["delta"]["tool_calls"][0]["function"]["arguments"],
            "{\"city\":"
        );
    }

    #[test]
    fn claude_sse_content_block_stop_dropped() {
        let a = claude::ClaudeAdapter;
        assert!(a
            .transform_sse_line(r#"{"type":"content_block_stop","index":0}"#)
            .is_none());
    }

    #[test]
    fn claude_sse_message_delta_stop() {
        let a = claude::ClaudeAdapter;
        let line = r#"{"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":10}}"#;
        let result = a.transform_sse_line(line).unwrap();
        let v: serde_json::Value = parse_json(&result);
        assert_eq!(v["choices"][0]["finish_reason"], "stop");
        assert_eq!(v["usage"]["completion_tokens"], 10);
    }

    #[test]
    fn claude_sse_message_delta_max_tokens() {
        let a = claude::ClaudeAdapter;
        let line = r#"{"type":"message_delta","delta":{"stop_reason":"max_tokens"},"usage":{"output_tokens":4096}}"#;
        let result = a.transform_sse_line(line).unwrap();
        let v: serde_json::Value = parse_json(&result);
        assert_eq!(v["choices"][0]["finish_reason"], "length");
    }

    #[test]
    fn claude_sse_message_delta_tool_use() {
        let a = claude::ClaudeAdapter;
        let line = r#"{"type":"message_delta","delta":{"stop_reason":"tool_use"}}"#;
        let result = a.transform_sse_line(line).unwrap();
        let v: serde_json::Value = parse_json(&result);
        assert_eq!(v["choices"][0]["finish_reason"], "tool_calls");
    }

    #[test]
    fn claude_sse_message_stop() {
        let a = claude::ClaudeAdapter;
        assert_eq!(
            a.transform_sse_line(r#"{"type":"message_stop"}"#),
            Some("[DONE]".into())
        );
    }

    #[test]
    fn claude_sse_ping_dropped() {
        let a = claude::ClaudeAdapter;
        assert!(a.transform_sse_line(r#"{"type":"ping"}"#).is_none());
    }

    #[test]
    fn claude_extract_sse_usage() {
        let a = claude::ClaudeAdapter;
        assert_eq!(a.extract_sse_usage("[DONE]"), (0, 0));
        let line = r#"{"type":"message_delta","usage":{"input_tokens":100,"output_tokens":50}}"#;
        assert_eq!(a.extract_sse_usage(line), (100, 50));
    }

    #[test]
    fn claude_parse_models_response() {
        let a = claude::ClaudeAdapter;
        let body = json!({
            "data": [
                {"id": "claude-3-opus-20240229", "display_name": "Claude 3 Opus"},
                {"id": "claude-3-sonnet-20240229"}
            ]
        });
        let models = a.parse_models_response(&body);
        assert_eq!(models.len(), 2);
        assert_eq!(
            models[0],
            (
                "claude-3-opus-20240229".into(),
                Some("Claude 3 Opus".into())
            )
        );
        assert_eq!(models[1], ("claude-3-sonnet-20240229".into(), None));
    }

    // ================================================================
    //  Custom Adapter
    // ================================================================

    #[test]
    fn custom_chat_url_no_v1_prefix() {
        let a = custom::CustomAdapter;
        assert_eq!(
            a.build_chat_url("https://api.deepseek.com/v1", "deepseek-chat"),
            "https://api.deepseek.com/v1/chat/completions"
        );
    }

    #[test]
    fn custom_chat_url_plain_base() {
        let a = custom::CustomAdapter;
        assert_eq!(
            a.build_chat_url("https://open.bigmodel.cn/api/paas/v4", "glm-4"),
            "https://open.bigmodel.cn/api/paas/v4/chat/completions"
        );
    }

    #[test]
    fn custom_models_url() {
        let a = custom::CustomAdapter;
        assert_eq!(
            a.build_models_url("https://api.deepseek.com/v1", "sk-test"),
            "https://api.deepseek.com/v1/models"
        );
    }

    #[test]
    fn custom_auth_headers() {
        let a = custom::CustomAdapter;
        let headers = a.build_auth_headers("sk-deepseek-12345");
        assert_eq!(headers.len(), 1);
        assert_eq!(
            headers[0],
            ("Authorization".into(), "Bearer sk-deepseek-12345".into())
        );
    }

    #[test]
    fn custom_transform_request_sets_model() {
        let a = custom::CustomAdapter;
        let mut body = json!({
            "model": "auto",
            "messages": [{"role": "user", "content": "hello"}]
        });
        a.transform_request(&mut body, "deepseek-chat");
        assert_eq!(body["model"], "deepseek-chat");
    }

    #[test]
    fn custom_transform_response_passthrough() {
        let a = custom::CustomAdapter;
        let mut body = json!({
            "id": "chatcmpl-custom-123",
            "choices": [{"message": {"content": "Hi from deepseek"}}]
        });
        let original = body.clone();
        a.transform_response(&mut body);
        assert_eq!(body, original);
    }

    #[test]
    fn custom_needs_sse_transform() {
        let a = custom::CustomAdapter;
        assert!(!a.needs_sse_transform());
    }

    #[test]
    fn custom_extract_sse_usage() {
        let a = custom::CustomAdapter;
        let line = r#"{"choices":[],"usage":{"prompt_tokens":20,"completion_tokens":15}}"#;
        assert_eq!(a.extract_sse_usage(line), (20, 15));
    }

    #[test]
    fn custom_parse_models_response() {
        let a = custom::CustomAdapter;
        let body = json!({
            "data": [
                {"id": "deepseek-chat", "owned_by": "deepseek"}
            ]
        });
        let models = a.parse_models_response(&body);
        assert_eq!(models.len(), 1);
        assert_eq!(models[0], ("deepseek-chat".into(), Some("deepseek".into())));
    }

    // ================================================================
    //  join_url shared utility
    // ================================================================

    #[test]
    fn join_url_basic() {
        assert_eq!(
            join_url("https://api.openai.com", "v1/chat"),
            "https://api.openai.com/v1/chat"
        );
    }

    #[test]
    fn join_url_dedup_v1() {
        assert_eq!(
            join_url("https://api.openai.com/v1", "v1/chat"),
            "https://api.openai.com/v1/chat"
        );
    }

    #[test]
    fn join_url_dedup_v1beta() {
        assert_eq!(
            join_url(
                "https://generativelanguage.googleapis.com/v1beta",
                "v1beta/models"
            ),
            "https://generativelanguage.googleapis.com/v1beta/models"
        );
    }

    #[test]
    fn join_url_dedup_trailing_slash() {
        assert_eq!(
            join_url("https://api.openai.com/", "v1/chat"),
            "https://api.openai.com/v1/chat"
        );
    }

    // ================================================================
    //  get_adapter factory
    // ================================================================

    #[test]
    fn get_adapter_openai() {
        let a = get_adapter("openai");
        assert!(!a.needs_sse_transform());
        assert_eq!(
            a.build_chat_url("https://api.openai.com", "gpt-4"),
            "https://api.openai.com/v1/chat/completions"
        );
    }

    #[test]
    fn get_adapter_claude() {
        let a = get_adapter("claude");
        assert!(a.needs_sse_transform());
        assert_eq!(
            a.build_chat_url("https://api.anthropic.com", "claude-3"),
            "https://api.anthropic.com/v1/messages"
        );
    }

    #[test]
    fn get_adapter_anthropic_alias() {
        let a = get_adapter("anthropic");
        assert!(a.needs_sse_transform());
    }

    #[test]
    fn get_adapter_custom() {
        let a = get_adapter("custom");
        assert!(!a.needs_sse_transform());
        assert_eq!(
            a.build_chat_url("https://api.deepseek.com/v1", "deepseek-chat"),
            "https://api.deepseek.com/v1/chat/completions"
        );
    }

    #[test]
    fn get_adapter_unknown_fallback() {
        let a = get_adapter("some_unknown_type");
        assert!(!a.needs_sse_transform());
        assert_eq!(
            a.build_chat_url("https://example.com", "model"),
            "https://example.com/v1/chat/completions"
        );
    }

    // ================================================================
    //  Azure Adapter
    // ================================================================

    #[test]
    fn azure_chat_url_basic() {
        let a = azure::AzureAdapter;
        assert_eq!(
            a.build_chat_url("https://myresource.openai.azure.com", "gpt-4o"),
            "https://myresource.openai.azure.com/openai/deployments/gpt-4o/chat/completions?api-version=2024-02-01"
        );
    }

    #[test]
    fn azure_chat_url_trailing_slash() {
        let a = azure::AzureAdapter;
        assert_eq!(
            a.build_chat_url("https://myresource.openai.azure.com/", "gpt-4o"),
            "https://myresource.openai.azure.com/openai/deployments/gpt-4o/chat/completions?api-version=2024-02-01"
        );
    }

    #[test]
    fn azure_models_url() {
        let a = azure::AzureAdapter;
        assert_eq!(
            a.build_models_url("https://myresource.openai.azure.com", "abc123"),
            "https://myresource.openai.azure.com/openai/deployments?api-version=2024-02-01"
        );
    }

    #[test]
    fn azure_auth_headers() {
        let a = azure::AzureAdapter;
        let headers = a.build_auth_headers("azure-key-12345");
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0], ("api-key".into(), "azure-key-12345".into()));
    }

    #[test]
    fn azure_uses_query_auth() {
        let a = azure::AzureAdapter;
        assert!(!a.uses_query_auth());
    }

    #[test]
    fn azure_transform_request_removes_model() {
        let a = azure::AzureAdapter;
        let mut body = json!({
            "model": "gpt-4o",
            "messages": [{"role": "user", "content": "hello"}]
        });
        a.transform_request(&mut body, "gpt-4o-deployment");
        // Azure ignores model in body — we remove it to avoid 400 errors
        assert!(body.get("model").is_none());
        // messages preserved
        assert_eq!(body["messages"][0]["content"], "hello");
    }

    #[test]
    fn azure_transform_response_passthrough() {
        let a = azure::AzureAdapter;
        let mut body = json!({
            "id": "chatcmpl-azure-123",
            "object": "chat.completion",
            "choices": [{"message": {"role": "assistant", "content": "Hi from Azure"}}],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5}
        });
        let original = body.clone();
        a.transform_response(&mut body);
        assert_eq!(body, original);
    }

    #[test]
    fn azure_needs_sse_transform() {
        let a = azure::AzureAdapter;
        assert!(!a.needs_sse_transform());
    }

    #[test]
    fn azure_extract_sse_usage() {
        let a = azure::AzureAdapter;
        let line = r#"{"choices":[],"usage":{"prompt_tokens":50,"completion_tokens":30}}"#;
        assert_eq!(a.extract_sse_usage(line), (50, 30));
    }

    #[test]
    fn azure_parse_models_response() {
        let a = azure::AzureAdapter;
        let body = json!({
            "data": [
                {"id": "gpt-4o-deployment", "model": "gpt-4o", "status": "succeeded"},
                {"id": "gpt-35-turbo-deployment"}
            ]
        });
        let models = a.parse_models_response(&body);
        assert_eq!(models.len(), 2);
        assert_eq!(models[0], ("gpt-4o-deployment".into(), Some("gpt-4o".into())));
        assert_eq!(models[1], ("gpt-35-turbo-deployment".into(), None));
    }

    #[test]
    fn azure_apply_auth() {
        let a = azure::AzureAdapter;
        let client = reqwest::Client::new();
        let builder = client.post("https://example.com");
        // apply_auth adds the api-key header
        let _ = a.apply_auth(builder, "test-key");
        // We can't easily inspect headers without sending, just verify no panic
    }

    // ================================================================
    //  Gemini Adapter
    // ================================================================

    #[test]
    fn gemini_chat_url_basic() {
        let a = gemini::GeminiAdapter;
        assert_eq!(
            a.build_chat_url("https://generativelanguage.googleapis.com", "gemini-2.0-flash"),
            "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"
        );
    }

    #[test]
    fn gemini_chat_url_dedup_v1beta() {
        let a = gemini::GeminiAdapter;
        assert_eq!(
            a.build_chat_url("https://generativelanguage.googleapis.com/v1beta", "gemini-2.0-flash"),
            "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"
        );
    }

    #[test]
    fn gemini_models_url() {
        let a = gemini::GeminiAdapter;
        assert_eq!(
            a.build_models_url("https://generativelanguage.googleapis.com", "AIza-test123"),
            "https://generativelanguage.googleapis.com/v1beta/openai/models?key=AIza-test123"
        );
    }

    #[test]
    fn gemini_auth_headers_empty() {
        let a = gemini::GeminiAdapter;
        let headers = a.build_auth_headers("AIza-test");
        assert!(headers.is_empty());
    }

    #[test]
    fn gemini_uses_query_auth() {
        let a = gemini::GeminiAdapter;
        assert!(a.uses_query_auth());
    }

    #[test]
    fn gemini_transform_request_sets_model() {
        let a = gemini::GeminiAdapter;
        let mut body = json!({
            "model": "auto",
            "messages": [{"role": "user", "content": "hello"}]
        });
        a.transform_request(&mut body, "gemini-2.0-flash");
        assert_eq!(body["model"], "gemini-2.0-flash");
    }

    #[test]
    fn gemini_transform_response_passthrough() {
        let a = gemini::GeminiAdapter;
        let mut body = json!({
            "id": "chatcmpl-gemini-123",
            "choices": [{"message": {"content": "Hi from Gemini"}}]
        });
        let original = body.clone();
        a.transform_response(&mut body);
        assert_eq!(body, original);
    }

    #[test]
    fn gemini_needs_sse_transform() {
        let a = gemini::GeminiAdapter;
        assert!(!a.needs_sse_transform());
    }

    #[test]
    fn gemini_extract_sse_usage() {
        let a = gemini::GeminiAdapter;
        let line = r#"{"choices":[],"usage":{"prompt_tokens":100,"completion_tokens":200}}"#;
        assert_eq!(a.extract_sse_usage(line), (100, 200));
    }

    #[test]
    fn gemini_parse_models_response_openai_compatible() {
        let a = gemini::GeminiAdapter;
        // Google's OpenAI-compatible endpoint returns standard OpenAI format
        let body = json!({
            "data": [
                {"id": "gemini-2.0-flash", "owned_by": "google"},
                {"id": "gemini-2.5-pro-preview", "owned_by": "google"}
            ]
        });
        let models = a.parse_models_response(&body);
        assert_eq!(models.len(), 2);
        assert_eq!(models[0], ("gemini-2.0-flash".into(), Some("google".into())));
        assert_eq!(models[1], ("gemini-2.5-pro-preview".into(), Some("google".into())));
    }

    #[test]
    fn get_adapter_gemini() {
        let a = get_adapter("gemini");
        assert!(a.uses_query_auth());
        assert!(!a.needs_sse_transform());
        assert_eq!(
            a.build_chat_url("https://generativelanguage.googleapis.com", "gemini-2.0-flash"),
            "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"
        );
    }

    #[test]
    fn get_adapter_azure() {
        let a = get_adapter("azure");
        assert!(!a.uses_query_auth());
        assert!(!a.needs_sse_transform());
        assert_eq!(
            a.build_chat_url("https://myresource.openai.azure.com", "gpt-4o"),
            "https://myresource.openai.azure.com/openai/deployments/gpt-4o/chat/completions?api-version=2024-02-01"
        );
    }

    // ================================================================
    //  Gemini native format conversion (standalone functions)
    // ================================================================

    #[test]
    fn gemini_native_chat_url() {
        assert_eq!(
            gemini::build_gemini_native_chat_url(
                "https://generativelanguage.googleapis.com",
                "gemini-2.0-flash"
            ),
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent"
        );
    }

    #[test]
    fn gemini_native_stream_url() {
        assert_eq!(
            gemini::build_gemini_native_stream_url(
                "https://generativelanguage.googleapis.com",
                "gemini-2.0-flash"
            ),
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:streamGenerateContent?alt=sse"
        );
    }

    #[test]
    fn gemini_native_models_url() {
        assert_eq!(
            gemini::build_gemini_native_models_url(
                "https://generativelanguage.googleapis.com",
                "AIza-test"
            ),
            "https://generativelanguage.googleapis.com/v1beta/models?key=AIza-test"
        );
    }

    #[test]
    fn gemini_native_parse_models() {
        let body = json!({
            "models": [
                {"name": "models/gemini-2.0-flash", "displayName": "Gemini 2.0 Flash"},
                {"name": "models/gemini-2.5-pro", "displayName": "Gemini 2.5 Pro"}
            ]
        });
        let models = gemini::parse_gemini_native_models(&body);
        assert_eq!(models.len(), 2);
        assert_eq!(models[0], ("gemini-2.0-flash".into(), Some("Gemini 2.0 Flash".into())));
        assert_eq!(models[1], ("gemini-2.5-pro".into(), Some("Gemini 2.5 Pro".into())));
    }

    #[test]
    fn gemini_native_transform_request_basic() {
        let mut body = json!({
            "model": "auto",
            "messages": [
                {"role": "system", "content": "You are helpful."},
                {"role": "user", "content": "hello"}
            ],
            "temperature": 0.7,
            "max_tokens": 1024
        });
        gemini::transform_request_to_gemini(&mut body, "gemini-2.0-flash");

        assert_eq!(body["model"], "gemini-2.0-flash");
        assert!(body.get("systemInstruction").is_some());
        assert_eq!(body["systemInstruction"]["parts"][0]["text"], "You are helpful.");
        // Messages → contents
        let contents = body["contents"].as_array().unwrap();
        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0]["role"], "user");
        assert_eq!(contents[0]["parts"][0]["text"], "hello");
        // generationConfig
        assert_eq!(body["generationConfig"]["temperature"], 0.7);
        assert_eq!(body["generationConfig"]["maxOutputTokens"], 1024);
    }

    #[test]
    fn gemini_native_transform_request_with_stream() {
        let mut body = json!({
            "model": "auto",
            "messages": [{"role": "user", "content": "hi"}],
            "stream": true
        });
        gemini::transform_request_to_gemini(&mut body, "gemini-2.0-flash");
        assert_eq!(body["stream"], true);
    }

    #[test]
    fn gemini_native_transform_request_with_stop() {
        let mut body = json!({
            "model": "auto",
            "messages": [{"role": "user", "content": "hi"}],
            "stop": ["\n"]
        });
        gemini::transform_request_to_gemini(&mut body, "gemini-2.0-flash");
        assert_eq!(body["generationConfig"]["stopSequences"], json!(["\n"]));
    }

    #[test]
    fn gemini_native_transform_response_basic() {
        let mut body = json!({
            "candidates": [{
                "content": {
                    "parts": [{"text": "Hello!"}],
                    "role": "model"
                },
                "finishReason": "STOP"
            }],
            "usageMetadata": {
                "promptTokenCount": 25,
                "candidatesTokenCount": 10,
                "totalTokenCount": 35
            },
            "model": "gemini-2.0-flash"
        });
        gemini::transform_response_from_gemini(&mut body);

        assert_eq!(body["object"], "chat.completion");
        assert_eq!(body["model"], "gemini-2.0-flash");
        assert_eq!(body["choices"][0]["message"]["content"], "Hello!");
        assert_eq!(body["choices"][0]["finish_reason"], "stop");
        assert_eq!(body["usage"]["prompt_tokens"], 25);
        assert_eq!(body["usage"]["completion_tokens"], 10);
        assert_eq!(body["usage"]["total_tokens"], 35);
    }

    #[test]
    fn gemini_native_transform_response_max_tokens() {
        let mut body = json!({
            "candidates": [{
                "content": {"parts": [{"text": "..."}], "role": "model"},
                "finishReason": "MAX_TOKENS"
            }],
            "usageMetadata": {"promptTokenCount": 50, "candidatesTokenCount": 8192}
        });
        gemini::transform_response_from_gemini(&mut body);
        assert_eq!(body["choices"][0]["finish_reason"], "length");
    }

    #[test]
    fn gemini_native_transform_response_safety() {
        let mut body = json!({
            "candidates": [{
                "content": {"parts": [{"text": ""}], "role": "model"},
                "finishReason": "SAFETY"
            }]
        });
        gemini::transform_response_from_gemini(&mut body);
        assert_eq!(body["choices"][0]["finish_reason"], "content_filter");
    }

    #[test]
    fn gemini_native_sse_transform_text_chunk() {
        let line = r#"{"candidates":[{"content":{"parts":[{"text":"Hello"}],"role":"model"}}],"usageMetadata":{"promptTokenCount":20}}"#;
        let result = gemini::transform_gemini_sse_line(line).unwrap();
        let v: serde_json::Value = parse_json(&result);
        assert_eq!(v["object"], "chat.completion.chunk");
        assert_eq!(v["choices"][0]["delta"]["content"], "Hello");
        assert_eq!(v["choices"][0]["finish_reason"], Value::Null);
    }

    #[test]
    fn gemini_native_sse_transform_with_finish_reason() {
        let line = r#"{"candidates":[{"content":{"parts":[{"text":"!"}],"role":"model"},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":20,"candidatesTokenCount":5,"totalTokenCount":25}}"#;
        let result = gemini::transform_gemini_sse_line(line).unwrap();
        let v: serde_json::Value = parse_json(&result);
        assert_eq!(v["choices"][0]["finish_reason"], "stop");
        assert_eq!(v["usage"]["completion_tokens"], 5);
    }

    #[test]
    fn gemini_native_sse_transform_empty_text_no_chunk() {
        // Gemini sends empty chunks between content — we still emit them
        let line = r#"{"candidates":[{"content":{"parts":[{"text":""}],"role":"model"}}]}"#;
        let result = gemini::transform_gemini_sse_line(line).unwrap();
        let v: serde_json::Value = parse_json(&result);
        // Empty text → no content in delta
        assert!(v["choices"][0]["delta"].as_object().unwrap().is_empty());
    }
}
