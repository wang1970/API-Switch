use super::circuit_breaker::{parse_status_codes, CircuitBreaker};
use super::handlers::ProxyError;
use super::protocol::get_adapter;
use super::server::ProxyState;
use crate::database::{AccessKey, ApiEntry};
use axum::body::Body;
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use bytes::Bytes;
use futures::Stream;
use serde_json::Value;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Arc;
use std::task::Poll;
use std::time::Instant;

/// Forward error with upstream status code (0 = connection failure).
type ForwardError = (String, u16);

struct ForwardResult {
    response: axum::response::Response,
    prompt_tokens: i64,
    completion_tokens: i64,
    first_token_ms: i64,
    status_code: i32,
}

/// Parse newline-separated keywords into lowercase Vec.
fn parse_keywords(input: &str) -> Vec<String> {
    input
        .lines()
        .map(|line| line.trim().to_lowercase())
        .filter(|line| !line.is_empty())
        .collect()
}

/// Forward a request to the resolved entries with retry/failover.
pub async fn forward_with_retry(
    state: &ProxyState,
    entries: &[ApiEntry],
    body: &Value,
    _original_headers: &HeaderMap,
    requested_model: &str,
    access_key: Option<&AccessKey>,
    is_stream: bool,
) -> Result<axum::response::Response, ProxyError> {
    // Read circuit config once
    let settings = state.db.get_settings().ok();
    let disable_codes = settings
        .as_ref()
        .map(|s| parse_status_codes(&s.circuit_disable_codes))
        .unwrap_or_default();
    let retry_codes = settings
        .as_ref()
        .map(|s| parse_status_codes(&s.circuit_retry_codes))
        .unwrap_or_default();
    let disable_keywords = settings
        .as_ref()
        .map(|s| parse_keywords(&s.disable_keywords))
        .unwrap_or_default();

    let mut last_error: Option<(String, u16)> = None;

    for entry in entries {
        let start = Instant::now();

        // Check circuit breaker
        {
            let breakers = state.circuit_breakers.read().await;
            if let Some(cb) = breakers.get(&entry.id) {
                if !cb.is_available() {
                    continue;
                }
            }
        }

        match forward_single(state, entry, body, requested_model, access_key, is_stream).await {
            Ok(result) => {
                let elapsed = start.elapsed();
                record_circuit_success(state, &entry.id).await;

                if !is_stream {
                    let latency_ms = elapsed.as_millis() as i64;
                    log_usage(
                        &state.db, access_key, entry, requested_model,
                        is_stream, result.prompt_tokens, result.completion_tokens,
                        result.first_token_ms, latency_ms, result.status_code, true, None,
                    );
                }
                return Ok(result.response);
            }
            Err((e, status)) => {
                let elapsed = start.elapsed();
                let latency_ms = elapsed.as_millis() as i64;
                let log_status = if status > 0 { status as i32 } else { 502 };

                // Write usage log
                log_usage(
                    &state.db, access_key, entry, requested_model,
                    is_stream, 0, 0, 0, latency_ms, log_status, false, Some(&e),
                );

                // Auto-disable channel: if error message contains any keyword
                let e_lower = e.to_lowercase();
                if disable_keywords.iter().any(|kw| e_lower.contains(kw)) {
                    log::warn!(
                        "Disable keyword matched for entry {} (channel {}), disabling channel",
                        entry.id, entry.channel_id
                    );
                    let _ = state.db.disable_channel(&entry.channel_id);
                    return Err(ProxyError::Internal(e));
                }

                // 504 and 524 are never retried (always return immediately)
                if status == 504 || status == 524 {
                    return Err(ProxyError::Internal(e));
                }

                // Auto-disable: if status is in disable_codes, immediately fail
                if disable_codes.contains(&status) {
                    return Err(ProxyError::Internal(e));
                }

                // Circuit breaker: trip on 5xx and connection failures
                if status >= 500 || status == 0 {
                    record_circuit_failure(state, &entry.id).await;
                }

                // Check retry: if status NOT in retry_codes, stop retrying
                if !retry_codes.contains(&status) {
                    return Err(ProxyError::Internal(e));
                }

                last_error = Some((e, status));
                continue;
            }
        }
    }

    Err(last_error
        .map(|(e, _)| ProxyError::Internal(e))
        .unwrap_or(ProxyError::AllProvidersFailed))
}

async fn forward_single(
    state: &ProxyState,
    entry: &ApiEntry,
    body: &Value,
    requested_model: &str,
    access_key: Option<&AccessKey>,
    is_stream: bool,
) -> Result<ForwardResult, ForwardError> {
    let channel = state
        .db
        .get_channel(&entry.channel_id)
        .map_err(|e| (format!("DB error: {e}"), 502))?;

    let adapter = get_adapter(&channel.api_type);
    let url = adapter.build_chat_url(&channel.base_url, &entry.model);

    let mut upstream_body = body.clone();
    adapter.transform_request(&mut upstream_body, &entry.model);

    let client = reqwest::Client::new();
    let mut request = adapter
        .apply_auth(client.post(&url), &channel.api_key)
        .json(&upstream_body);

    if is_stream {
        request = request.header("Accept", "text/event-stream");
    }

    let response = request
        .send()
        .await
        .map_err(|e| (format!("Request failed: {e}"), 0))?;

    let status = response.status().as_u16();

    if !response.status().is_success() {
        let error_body = response.text().await.unwrap_or_default();
        return Err((format!("Upstream error {status}: {error_body}"), status));
    }

    let status_code = status as i32;

    if is_stream {
        let needs_transform = adapter.needs_sse_transform();
        let response = build_streaming_response(
            state, entry, access_key, requested_model,
            response, status_code, needs_transform, adapter,
        );
        Ok(ForwardResult {
            response, prompt_tokens: 0, completion_tokens: 0,
            first_token_ms: 0, status_code,
        })
    } else {
        let mut response_body: Value = response
            .json()
            .await
            .map_err(|e| (format!("Failed to parse response: {e}"), 502))?;

        adapter.transform_response(&mut response_body);
        let (prompt_tokens, completion_tokens) = extract_usage_tokens(&response_body);

        Ok(ForwardResult {
            response: axum::Json(response_body).into_response(),
            prompt_tokens, completion_tokens, first_token_ms: 0, status_code,
        })
    }
}

fn extract_usage_tokens(body: &Value) -> (i64, i64) {
    let usage = body.get("usage");
    let prompt_tokens = usage
        .and_then(|v| v.get("prompt_tokens"))
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let completion_tokens = usage
        .and_then(|v| v.get("completion_tokens"))
        .and_then(Value::as_i64)
        .unwrap_or(0);
    (prompt_tokens, completion_tokens)
}

fn build_streaming_response(
    state: &ProxyState,
    entry: &ApiEntry,
    access_key: Option<&AccessKey>,
    requested_model: &str,
    response: reqwest::Response,
    status_code: i32,
    needs_transform: bool,
    adapter: Box<dyn super::protocol::ProtocolAdapter + Send>,
) -> axum::response::Response {
    let start = Instant::now();
    let db = state.db.clone();
    let entry = entry.clone();
    let access_key = access_key.cloned();
    let requested_model = requested_model.to_string();
    let first_token_ms = Arc::new(AtomicI64::new(0));
    let prompt_tokens = Arc::new(AtomicI64::new(0));
    let completion_tokens = Arc::new(AtomicI64::new(0));
    let seen_first_chunk = Arc::new(AtomicBool::new(false));
    let logged = Arc::new(AtomicBool::new(false));
    let mut sse_buffer = String::new();
    let mut upstream_stream = Box::pin(response.bytes_stream());

    let body_stream = futures::stream::poll_fn(move |cx| -> Poll<Option<Result<Bytes, std::io::Error>>> {
        match upstream_stream.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(chunk))) => {
                if !seen_first_chunk.swap(true, Ordering::Relaxed) {
                    first_token_ms.store(start.elapsed().as_millis() as i64, Ordering::Relaxed);
                }

                if needs_transform {
                    if let Some(transformed) = transform_sse_chunk(
                        &chunk, &mut sse_buffer, &adapter,
                        &prompt_tokens, &completion_tokens,
                    ) {
                        return Poll::Ready(Some(Ok(transformed)));
                    } else {
                        cx.waker().wake_by_ref();
                        return Poll::Pending;
                    }
                } else {
                    append_and_parse_sse(&mut sse_buffer, &chunk, &prompt_tokens, &completion_tokens);
                }

                Poll::Ready(Some(Ok(chunk)))
            }
            Poll::Ready(Some(Err(err))) => {
                if !logged.swap(true, Ordering::Relaxed) {
                    log_usage(
                        &db, access_key.as_ref(), &entry, &requested_model,
                        true, prompt_tokens.load(Ordering::Relaxed),
                        completion_tokens.load(Ordering::Relaxed),
                        first_token_ms.load(Ordering::Relaxed),
                        start.elapsed().as_millis() as i64,
                        502, false, Some(&format!("Stream error: {err}")),
                    );
                }
                Poll::Ready(Some(Err(std::io::Error::new(std::io::ErrorKind::Other, err))))
            }
            Poll::Ready(None) => {
                if !logged.swap(true, Ordering::Relaxed) {
                    log_usage(
                        &db, access_key.as_ref(), &entry, &requested_model,
                        true, prompt_tokens.load(Ordering::Relaxed),
                        completion_tokens.load(Ordering::Relaxed),
                        first_token_ms.load(Ordering::Relaxed),
                        start.elapsed().as_millis() as i64,
                        status_code, true, None,
                    );
                }
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    });

    axum::http::Response::builder()
        .status(axum::http::StatusCode::from_u16(status_code as u16).unwrap_or(axum::http::StatusCode::OK))
        .header("content-type", "text/event-stream")
        .header("cache-control", "no-cache")
        .header("connection", "keep-alive")
        .body(Body::from_stream(body_stream))
        .unwrap()
}

fn transform_sse_chunk(
    chunk: &Bytes,
    buffer: &mut String,
    adapter: &Box<dyn super::protocol::ProtocolAdapter + Send>,
    prompt_tokens: &Arc<AtomicI64>,
    completion_tokens: &Arc<AtomicI64>,
) -> Option<Bytes> {
    buffer.push_str(&String::from_utf8_lossy(chunk));
    let mut output = Vec::new();

    while let Some(line_end) = buffer.find('\n') {
        let mut line = buffer.drain(..=line_end).collect::<String>();
        if line.ends_with('\n') { line.pop(); }
        if line.ends_with('\r') { line.pop(); }

        let Some(payload) = line.strip_prefix("data: ") else { continue };
        if payload == "[DONE]" {
            output.push(b"data: [DONE]\n".to_vec());
            continue;
        }

        let (prompt, completion) = adapter.extract_sse_usage(payload);
        if prompt > 0 { prompt_tokens.store(prompt, Ordering::Relaxed); }
        if completion > 0 { completion_tokens.store(completion, Ordering::Relaxed); }

        if let Some(transformed) = adapter.transform_sse_line(payload) {
            output.push(format!("data: {transformed}\n").into_bytes());
        }
    }

    if output.is_empty() { None } else { Some(Bytes::from(output.concat())) }
}

fn append_and_parse_sse(
    buffer: &mut String,
    chunk: &Bytes,
    prompt_tokens: &Arc<AtomicI64>,
    completion_tokens: &Arc<AtomicI64>,
) {
    buffer.push_str(&String::from_utf8_lossy(chunk));

    while let Some(line_end) = buffer.find('\n') {
        let mut line = buffer.drain(..=line_end).collect::<String>();
        if line.ends_with('\n') { line.pop(); }
        if line.ends_with('\r') { line.pop(); }

        let Some(payload) = line.strip_prefix("data: ") else { continue };
        if payload == "[DONE]" { continue }

        let Ok(value) = serde_json::from_str::<Value>(payload) else { continue };
        let (prompt, completion) = extract_usage_tokens(&value);
        if prompt > 0 { prompt_tokens.store(prompt, Ordering::Relaxed); }
        if completion > 0 { completion_tokens.store(completion, Ordering::Relaxed); }
    }
}

async fn record_circuit_success(state: &ProxyState, entry_id: &str) {
    let mut breakers = state.circuit_breakers.write().await;
    let recovery_secs = state.db.get_settings().ok()
        .map(|s| s.circuit_recovery_secs as u64).unwrap_or(60);

    let cb = breakers
        .entry(entry_id.to_string())
        .or_insert_with(|| CircuitBreaker::new(recovery_secs));
    cb.record_success();
}

async fn record_circuit_failure(state: &ProxyState, entry_id: &str) {
    let mut breakers = state.circuit_breakers.write().await;
    let settings = state.db.get_settings().ok();
    let threshold = settings
        .as_ref()
        .map(|s| s.circuit_failure_threshold as u32)
        .unwrap_or(4);
    let recovery_secs = settings
        .as_ref()
        .map(|s| s.circuit_recovery_secs as u64)
        .unwrap_or(60);

    let cb = breakers
        .entry(entry_id.to_string())
        .or_insert_with(|| CircuitBreaker::new(recovery_secs));
    cb.set_recovery_secs(recovery_secs);
    cb.record_failure(threshold);
}

fn log_usage(
    db: &crate::database::Database,
    access_key: Option<&AccessKey>,
    entry: &ApiEntry,
    requested_model: &str,
    is_stream: bool,
    prompt_tokens: i64,
    completion_tokens: i64,
    first_token_ms: i64,
    latency_ms: i64,
    status_code: i32,
    success: bool,
    error_message: Option<&str>,
) {
    let log_type = if success { 2 } else { 5 };
    let content = error_message.unwrap_or("");
    let token_name = access_key.map(|ak| ak.name.as_str()).unwrap_or("anonymous");
    let use_time = ((latency_ms as f64) / 1000.0).ceil() as i64;
    let other = format!(
        "{{\"requested_model\":\"{}\",\"resolved_model\":\"{}\",\"first_token_ms\":{},\"status_code\":{},\"success\":{}}}",
        requested_model, entry.model, first_token_ms, status_code, success
    );

    let _ = db.insert_usage_log(
        log_type, content,
        access_key.map(|ak| ak.id.as_str()),
        access_key.map(|ak| ak.name.as_str()).unwrap_or("anonymous"),
        token_name, &entry.id, &entry.channel_id,
        entry.channel_name.as_deref().unwrap_or("unknown"),
        &entry.model, requested_model, 0, is_stream,
        prompt_tokens, completion_tokens, latency_ms,
        first_token_ms, use_time, status_code, success,
        "", "default", &other, error_message, None,
    );
}
