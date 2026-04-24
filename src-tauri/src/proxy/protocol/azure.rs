use super::ProtocolAdapter;
/// Azure OpenAI protocol adapter.
///
/// Azure Chat Completions API is OpenAI-protocol compatible — the request/response
/// body format is identical to OpenAI. The differences are:
///
/// - **URL**: `https://<resource>.openai.azure.com/openai/deployments/<deployment>/chat/completions?api-version=2024-02-01`
/// - **Auth**: `api-key` header (not `Bearer` token)
/// - **Model**: The `model` field in the request body is *ignored* by Azure;
///   the deployment name is in the URL path.
/// - **Response**: Uses OpenAI-compatible format natively. SSE streaming is also
///   OpenAI-compatible (no transformation needed).
///
/// NOTE: `api-version` is configurable. We use `2024-02-01` as the default
/// because it is widely supported. Newer versions add features but this one
/// covers chat completions + tools + streaming.
use serde_json::Value;

/// Default API version for Azure OpenAI.
const AZURE_API_VERSION: &str = "2024-02-01";

pub struct AzureAdapter;

impl ProtocolAdapter for AzureAdapter {
    fn build_chat_url(&self, base_url: &str, model: &str) -> String {
        // Azure format:
        //   {base_url}/openai/deployments/{deployment}/chat/completions?api-version=...
        // `model` from the API entry is used as the deployment name.
        let base = base_url.trim_end_matches('/');
        format!(
            "{}/openai/deployments/{}/chat/completions?api-version={}",
            base, model, AZURE_API_VERSION
        )
    }

    fn build_models_url(&self, base_url: &str, _api_key: &str) -> String {
        let base = base_url.trim_end_matches('/');
        format!(
            "{}/openai/deployments?api-version={}",
            base, AZURE_API_VERSION
        )
    }

    fn uses_query_auth(&self) -> bool {
        false
    }

    fn build_auth_headers(&self, api_key: &str) -> Vec<(String, String)> {
        vec![("api-key".to_string(), api_key.to_string())]
    }

    fn apply_auth(
        &self,
        builder: reqwest::RequestBuilder,
        api_key: &str,
    ) -> reqwest::RequestBuilder {
        builder.header("api-key", api_key)
    }

    fn transform_request(&self, body: &mut Value, _actual_model: &str) {
        // Azure ignores the `model` field in the request body — the deployment
        // name is already in the URL. We still set it so logging / compatibility
        // tools can see what was requested, but it has no effect on routing.
        // No other transformation needed — Azure uses OpenAI format natively.
        // Remove `model` from body to avoid Azure 400 errors for unknown model.
        if let Some(obj) = body.as_object_mut() {
            obj.remove("model");
        }
    }

    fn transform_response(&self, _body: &mut Value) {
        // Azure response is already in OpenAI format. No transformation needed.
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
        // Azure returns: { data: [{ id: "deployment-name", model: "gpt-4o", ... }] }
        // The `id` is the deployment name, `model` is the underlying model.
        // We return deployment name as the id, model as the display name.
        body.get("data")
            .and_then(|d| d.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| {
                        let id = m.get("id")?.as_str()?.to_string();
                        // Use "model" field as display name if available
                        let owned_by = m
                            .get("model")
                            .and_then(|v| v.as_str())
                            .map(String::from)
                            .or_else(|| {
                                m.get("owned_by").and_then(|v| v.as_str()).map(String::from)
                            });
                        Some((id, owned_by))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}
