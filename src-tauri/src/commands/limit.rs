use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LimitCredentialStatus {
    Valid,
    Expired,
    NotFound,
    ParseError,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LimitTier {
    pub name: String,
    pub utilization: f64,
    pub resets_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LimitQueryResult {
    pub provider: String,
    pub credential_status: LimitCredentialStatus,
    pub credential_message: Option<String>,
    pub success: bool,
    pub tiers: Vec<LimitTier>,
    pub error: Option<String>,
    pub queried_at: Option<i64>,
    /// Raw upstream response for extensibility/debugging. The normalized `tiers` above are the
    /// stable cross-provider data shape; provider-specific fields can be read from `raw`.
    pub raw: Option<serde_json::Value>,
}

enum CodingPlanProvider {
    Kimi,
    Zhipu,
    MiniMaxCn,
    MiniMaxGlobal,
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn millis_to_iso8601(ms: i64) -> Option<String> {
    let secs = ms / 1000;
    let nsecs = ((ms % 1000) * 1_000_000) as u32;
    chrono::DateTime::from_timestamp(secs, nsecs).map(|dt| dt.to_rfc3339())
}

fn extract_reset_time(value: &serde_json::Value) -> Option<String> {
    if let Some(s) = value.as_str() {
        return Some(s.to_string());
    }
    if let Some(n) = value.as_i64() {
        let ms = if n < 1_000_000_000_000 { n * 1000 } else { n };
        return millis_to_iso8601(ms);
    }
    None
}

fn parse_f64(value: &serde_json::Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
}

fn detect_coding_plan_provider(base_url: &str) -> Option<CodingPlanProvider> {
    let url = base_url.to_lowercase();
    if url.contains("api.kimi.com/coding") {
        Some(CodingPlanProvider::Kimi)
    } else if url.contains("open.bigmodel.cn")
        || url.contains("bigmodel.cn")
        || url.contains("api.z.ai")
    {
        Some(CodingPlanProvider::Zhipu)
    } else if url.contains("api.minimaxi.com") {
        Some(CodingPlanProvider::MiniMaxCn)
    } else if url.contains("api.minimax.io") {
        Some(CodingPlanProvider::MiniMaxGlobal)
    } else {
        None
    }
}

fn not_found(provider: &str) -> LimitQueryResult {
    LimitQueryResult {
        provider: provider.to_string(),
        credential_status: LimitCredentialStatus::NotFound,
        credential_message: None,
        success: false,
        tiers: vec![],
        error: None,
        queried_at: None,
        raw: None,
    }
}

fn error(provider: &str, message: String) -> LimitQueryResult {
    LimitQueryResult {
        provider: provider.to_string(),
        credential_status: LimitCredentialStatus::Valid,
        credential_message: None,
        success: false,
        tiers: vec![],
        error: Some(message),
        queried_at: Some(now_millis()),
        raw: None,
    }
}

fn auth_error(provider: &str, status: reqwest::StatusCode) -> LimitQueryResult {
    LimitQueryResult {
        provider: provider.to_string(),
        credential_status: LimitCredentialStatus::Expired,
        credential_message: Some("Invalid API key".to_string()),
        success: false,
        tiers: vec![],
        error: Some(format!("Authentication failed (HTTP {status})")),
        queried_at: Some(now_millis()),
        raw: None,
    }
}

fn success(
    provider: &str,
    tiers: Vec<LimitTier>,
    credential_message: Option<String>,
    raw: serde_json::Value,
) -> LimitQueryResult {
    LimitQueryResult {
        provider: provider.to_string(),
        credential_status: LimitCredentialStatus::Valid,
        credential_message,
        success: true,
        tiers,
        error: None,
        queried_at: Some(now_millis()),
        raw: Some(raw),
    }
}

async fn query_kimi(client: &reqwest::Client, api_key: &str) -> LimitQueryResult {
    let provider = "kimi";
    let resp = client
        .get("https://api.kimi.com/coding/v1/usages")
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Accept", "application/json")
        .send()
        .await;

    let resp = match resp {
        Ok(r) => r,
        Err(e) => return error(provider, format!("Network error: {e}")),
    };

    let status = resp.status();
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return auth_error(provider, status);
    }
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return error(provider, format!("API error (HTTP {status}): {body}"));
    }

    let body: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => return error(provider, format!("Failed to parse response: {e}")),
    };

    let mut tiers = Vec::new();
    if let Some(limits) = body.get("limits").and_then(|v| v.as_array()) {
        for limit_item in limits {
            if let Some(detail) = limit_item.get("detail") {
                let limit = detail.get("limit").and_then(parse_f64).unwrap_or(1.0);
                let remaining = detail.get("remaining").and_then(parse_f64).unwrap_or(0.0);
                let resets_at = detail.get("resetTime").and_then(extract_reset_time);
                let used = (limit - remaining).max(0.0);
                let utilization = if limit > 0.0 { (used / limit) * 100.0 } else { 0.0 };
                tiers.push(LimitTier { name: "five_hour".to_string(), utilization, resets_at });
            }
        }
    }

    if let Some(usage) = body.get("usage") {
        let limit = usage.get("limit").and_then(parse_f64).unwrap_or(1.0);
        let remaining = usage.get("remaining").and_then(parse_f64).unwrap_or(0.0);
        let resets_at = usage.get("resetTime").and_then(extract_reset_time);
        let used = (limit - remaining).max(0.0);
        let utilization = if limit > 0.0 { (used / limit) * 100.0 } else { 0.0 };
        tiers.push(LimitTier { name: "weekly_limit".to_string(), utilization, resets_at });
    }

    success(provider, tiers, None, body)
}

async fn query_zhipu(client: &reqwest::Client, api_key: &str) -> LimitQueryResult {
    let provider = "zhipu";
    let resp = client
        .get("https://api.z.ai/api/monitor/usage/quota/limit")
        .header("Authorization", api_key)
        .header("Content-Type", "application/json")
        .header("Accept-Language", "en-US,en")
        .send()
        .await;

    let resp = match resp {
        Ok(r) => r,
        Err(e) => return error(provider, format!("Network error: {e}")),
    };

    let status = resp.status();
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return auth_error(provider, status);
    }
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return error(provider, format!("API error (HTTP {status}): {body}"));
    }

    let body: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => return error(provider, format!("Failed to parse response: {e}")),
    };

    if body.get("success").and_then(|v| v.as_bool()) == Some(false) {
        let msg = body.get("msg").and_then(|v| v.as_str()).unwrap_or("Unknown error");
        return error(provider, format!("API error: {msg}"));
    }

    let data = match body.get("data") {
        Some(d) => d,
        None => return error(provider, "Missing 'data' field in response".to_string()),
    };

    let mut tiers = Vec::new();
    if let Some(limits) = data.get("limits").and_then(|v| v.as_array()) {
        for limit_item in limits {
            let limit_type = limit_item.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if limit_type != "TOKENS_LIMIT" {
                continue;
            }
            let utilization = limit_item
                .get("percentage")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let resets_at = limit_item
                .get("nextResetTime")
                .and_then(|v| v.as_i64())
                .and_then(millis_to_iso8601);
            tiers.push(LimitTier { name: "five_hour".to_string(), utilization, resets_at });
        }
    }

    let level = data.get("level").and_then(|v| v.as_str()).map(str::to_string);
    success(provider, tiers, level, body)
}

async fn query_minimax(client: &reqwest::Client, api_key: &str, is_cn: bool) -> LimitQueryResult {
    let provider = if is_cn { "minimax_cn" } else { "minimax_global" };
    let api_domain = if is_cn { "api.minimaxi.com" } else { "api.minimax.io" };
    let url = format!("https://{api_domain}/v1/api/openplatform/coding_plan/remains");
    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .send()
        .await;

    let resp = match resp {
        Ok(r) => r,
        Err(e) => return error(provider, format!("Network error: {e}")),
    };

    let status = resp.status();
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return auth_error(provider, status);
    }
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return error(provider, format!("API error (HTTP {status}): {body}"));
    }

    let body: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => return error(provider, format!("Failed to parse response: {e}")),
    };

    if let Some(base_resp) = body.get("base_resp") {
        let status_code = base_resp.get("status_code").and_then(|v| v.as_i64()).unwrap_or(-1);
        if status_code != 0 {
            let msg = base_resp
                .get("status_msg")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error");
            return error(provider, format!("API error (code {status_code}): {msg}"));
        }
    }

    let mut tiers = Vec::new();
    if let Some(model_remains) = body.get("model_remains").and_then(|v| v.as_array()) {
        if let Some(item) = model_remains.first() {
            let interval_total = item
                .get("current_interval_total_count")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let interval_remaining = item
                .get("current_interval_usage_count")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let end_time = item.get("end_time").and_then(|v| v.as_i64());
            if interval_total > 0.0 {
                tiers.push(LimitTier {
                    name: "five_hour".to_string(),
                    utilization: ((interval_total - interval_remaining) / interval_total) * 100.0,
                    resets_at: end_time.and_then(millis_to_iso8601),
                });
            }

            let weekly_total = item
                .get("current_weekly_total_count")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let weekly_remaining = item
                .get("current_weekly_usage_count")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let weekly_end = item.get("weekly_end_time").and_then(|v| v.as_i64());
            if weekly_total > 0.0 {
                tiers.push(LimitTier {
                    name: "weekly_limit".to_string(),
                    utilization: ((weekly_total - weekly_remaining) / weekly_total) * 100.0,
                    resets_at: weekly_end.and_then(millis_to_iso8601),
                });
            }
        }
    }

    success(provider, tiers, None, body)
}

pub async fn query_limit_by_url(base_url: &str, api_key: &str) -> Result<LimitQueryResult, AppError> {
    if api_key.trim().is_empty() {
        return Ok(not_found("unknown"));
    }

    let provider = match detect_coding_plan_provider(base_url) {
        Some(p) => p,
        None => return Ok(not_found("unknown")),
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|e| AppError::Network(format!("HTTP client: {e}")))?;

    let result = match provider {
        CodingPlanProvider::Kimi => query_kimi(&client, api_key).await,
        CodingPlanProvider::Zhipu => query_zhipu(&client, api_key).await,
        CodingPlanProvider::MiniMaxCn => query_minimax(&client, api_key, true).await,
        CodingPlanProvider::MiniMaxGlobal => query_minimax(&client, api_key, false).await,
    };

    Ok(result)
}

#[tauri::command]
pub async fn query_limit(base_url: String, api_key: String) -> Result<LimitQueryResult, AppError> {
    query_limit_by_url(&base_url, &api_key).await
}
