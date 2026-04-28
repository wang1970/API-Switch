use super::circuit_breaker::CircuitBreaker;
use super::handlers::ProxyError;
use super::protocol::get_adapter;
use super::server::ProxyState;
use crate::database::{AccessKey, ApiEntry, Database};
use crate::{build_tray_menu, TRAY_ID};
use tauri::Emitter;
use axum::body::Body;
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use bytes::Bytes;
use futures::Stream;
use serde_json::Value;
use std::future::Future;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Arc;
use std::task::Poll;
use std::time::{Duration, Instant};
use tokio::time::sleep;

const STREAMING_IDLE_TIMEOUT: Duration = Duration::from_secs(300);
const STREAMING_PING_INTERVAL: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy)]
enum StreamEndReason {
    Done,
    UpstreamError,
    Timeout,
    Dropped,
}

impl StreamEndReason {
    fn as_str(self) -> &'static str {
        match self {
            StreamEndReason::Done => "done",
            StreamEndReason::UpstreamError => "upstream_error",
            StreamEndReason::Timeout => "timeout",
            StreamEndReason::Dropped => "dropped",
        }
    }
}

#[derive(Debug, Clone)]
struct AttemptInfo {
    entry_id: String,
    channel_name: String,
    model: String,
    status_code: i32,
    success: bool,
    error: Option<String>,
}

fn attempt_path_json(attempts: &[AttemptInfo]) -> String {
    serde_json::to_string(
        &attempts
            .iter()
            .map(|a| {
                serde_json::json!({
                    "entry_id": a.entry_id,
                    "channel": a.channel_name,
                    "model": a.model,
                    "status_code": a.status_code,
                    "success": a.success,
                    "error": a.error,
                })
            })
            .collect::<Vec<_>>()
    )
    .unwrap_or_else(|_| "[]".to_string())
}

fn push_attempt(
    attempts: &mut Vec<AttemptInfo>,
    entry: &ApiEntry,
    status_code: i32,
    success: bool,
    error: Option<String>,
) {
    attempts.push(AttemptInfo {
        entry_id: entry.id.clone(),
        channel_name: entry.channel_name.clone().unwrap_or_else(|| "unknown".to_string()),
        model: entry.model.clone(),
        status_code,
        success,
        error,
    });
}

fn attempt_path_with_current(
    prior_attempts: &[AttemptInfo],
    entry: &ApiEntry,
    status_code: i32,
    success: bool,
    error: Option<String>,
) -> String {
    let mut attempts = prior_attempts.to_vec();
    push_attempt(&mut attempts, entry, status_code, success, error);
    attempt_path_json(&attempts)
}

/// Forward error with upstream status code (0 = connection failure).
type ForwardError = (String, u16);

struct ForwardResult {
    response: axum::response::Response,
    prompt_tokens: i64,
    completion_tokens: i64,
    first_token_ms: i64,
    status_code: i32,
}

/// StreamLogGuard: safety net for writing usage log when stream is dropped
/// without reaching Poll::Ready(None) (e.g. client disconnect).
/// Primary log writing happens in Poll::Ready(None) — this guard is fallback only.
struct StreamLogGuard {
    logged: Arc<AtomicBool>,
    db: Arc<Database>,
    app_handle: tauri::AppHandle,
    access_key: Option<AccessKey>,
    entry: ApiEntry,
    requested_model: String,
    prompt_tokens: Arc<AtomicI64>,
    completion_tokens: Arc<AtomicI64>,
    first_token_ms: Arc<AtomicI64>,
    status_code: i32,
    start: Instant,
    prior_attempts: Vec<AttemptInfo>,
}

impl Drop for StreamLogGuard {
    fn drop(&mut self) {
        if !self.logged.swap(true, Ordering::SeqCst) {
            let attempt_path = attempt_path_with_current(
                &self.prior_attempts,
                &self.entry,
                self.status_code,
                false,
                Some("stream dropped before normal completion".to_string()),
            );
            log_usage(
                &self.db, &self.app_handle, self.access_key.as_ref(), &self.entry, &self.requested_model,
                true, self.prompt_tokens.load(Ordering::SeqCst),
                self.completion_tokens.load(Ordering::SeqCst),
                self.first_token_ms.load(Ordering::SeqCst),
                self.start.elapsed().as_millis() as i64,
                self.status_code, false, Some("stream dropped before normal completion"),
                Some(attempt_path.as_str()), Some(StreamEndReason::Dropped),
            );
        }
    }
}

/// Forward a request to the resolved entries with retry/failover.
///
/// Personal-version cooldown strategy:
/// 1. Any upstream failure is considered abnormal for this model entry.
/// 2. Failed entries are cooled down for `circuit_recovery_secs` seconds and skipped by routing.
/// 3. Unrecoverable status codes can disable an entry automatically.
pub async fn forward_with_retry(
    state: &ProxyState,
    entries: &[ApiEntry],
    body: &Value,
    _original_headers: &HeaderMap,
    requested_model: &str,
    access_key: Option<&AccessKey>,
    is_stream: bool,
) -> Result<axum::response::Response, ProxyError> {
    let mut last_error: Option<(String, u16)> = None;
    let mut attempts: Vec<AttemptInfo> = Vec::new();

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

        match forward_single(state, entry, body, requested_model, access_key, is_stream, attempts.clone()).await {
            Ok(result) => {
                let elapsed = start.elapsed();

                if !is_stream {
                    record_circuit_success(state, &entry.id).await;
                    push_attempt(&mut attempts, entry, result.status_code, true, None);
                    let attempt_path = attempt_path_json(&attempts);
                    let latency_ms = elapsed.as_millis() as i64;
                    log_usage(
                        &state.db, &state.app_handle, access_key, entry, requested_model,
                        is_stream, result.prompt_tokens, result.completion_tokens,
                        result.first_token_ms, latency_ms, result.status_code, true, None,
                        Some(attempt_path.as_str()), None,
                    );
                }
                return Ok(result.response);
            }
            Err((e, status)) => {
                let elapsed = start.elapsed();
                let latency_ms = elapsed.as_millis() as i64;
                let log_status = if status > 0 { status as i32 } else { 502 };
                let settings = state.db.get_settings().ok();
                push_attempt(&mut attempts, entry, log_status, false, Some(e.clone()));
                let attempt_path = attempt_path_json(&attempts);

                // Step 1: Always write usage log for every failed attempt
                log_usage(
                    &state.db, &state.app_handle, access_key, entry, requested_model,
                    is_stream, 0, 0, 0, latency_ms, log_status, false, Some(&e),
                    Some(attempt_path.as_str()), None,
                );

                // Step 2: disable unrecoverable status codes, otherwise cool down.
                if status > 0
                    && settings
                        .as_ref()
                        .map(|s| should_disable_entry_for_status(&s.circuit_disable_codes, status))
                        .unwrap_or(false)
                {
                    disable_entry(state, entry).await;
                } else {
                    cool_down_entry(state, entry).await;
                }

                last_error = Some((e, status));
                continue;
            }
        }
    }

    Err(last_error
        .map(|(msg, status)| {
            if status > 0 {
                ProxyError::Upstream { status, message: msg }
            } else {
                ProxyError::Internal(msg)
            }
        })
        .unwrap_or(ProxyError::AllProvidersFailed))
}

async fn forward_single(
    state: &ProxyState,
    entry: &ApiEntry,
    body: &Value,
    requested_model: &str,
    access_key: Option<&AccessKey>,
    is_stream: bool,
    prior_attempts: Vec<AttemptInfo>,
) -> Result<ForwardResult, ForwardError> {
    let channel = state
        .db
        .get_channel(&entry.channel_id)
        .map_err(|e| (format!("DB error: {e}"), 502))?;

    let adapter = get_adapter(&channel.api_type);
    let url = adapter.build_chat_url(&channel.base_url, &entry.model);

    let mut upstream_body = body.clone();
    adapter.transform_request(&mut upstream_body, &entry.model);

    let mut request = adapter
        .apply_auth(state.http_client.post(&url), &channel.api_key)
        .json(&upstream_body);

    if is_stream {
        request = request.header("Accept", "text/event-stream");
        request = request.header("Accept-Encoding", "identity");
    }

    // Start timer BEFORE sending request — this measures true TTFB
    let request_start = std::time::Instant::now();

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
            response, status_code, needs_transform, adapter, request_start, prior_attempts,
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
    request_start: std::time::Instant,
    prior_attempts: Vec<AttemptInfo>,
) -> axum::response::Response {
    let start = request_start;
    let db = state.db.clone();
    let app_handle = state.app_handle.clone();
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
    let mut idle_timeout = Box::pin(sleep(STREAMING_IDLE_TIMEOUT));
    let mut ping_interval = Box::pin(sleep(STREAMING_PING_INTERVAL));
    let entry_id = entry.id.clone();
    let circuit_breakers = state.circuit_breakers.clone();
    let settings_db = state.db.clone();
    let entries_app_handle = state.app_handle.clone();
    let success_circuit_breakers = state.circuit_breakers.clone();

    // Guard captured by the move closure → lives as long as the stream body
    let guard = StreamLogGuard {
        logged: logged.clone(),
        db: db.clone(),
        app_handle: app_handle.clone(),
        access_key: access_key.clone(),
        entry: entry.clone(),
        requested_model: requested_model.clone(),
        prompt_tokens: prompt_tokens.clone(),
        completion_tokens: completion_tokens.clone(),
        first_token_ms: first_token_ms.clone(),
        status_code,
        start,
        prior_attempts: prior_attempts.clone(),
    };

    let body_stream = futures::stream::poll_fn(move |cx| -> Poll<Option<Result<Bytes, std::io::Error>>> {
        let _ = &guard; // keep guard alive in the closure's capture list

        if ping_interval.as_mut().poll(cx).is_ready() {
            ping_interval.as_mut().reset(tokio::time::Instant::now() + STREAMING_PING_INTERVAL);
            return Poll::Ready(Some(Ok(Bytes::from_static(b": PING\n\n"))));
        }

        if idle_timeout.as_mut().poll(cx).is_ready() {
            if !logged.swap(true, Ordering::SeqCst) {
                let attempt_path = attempt_path_with_current(
                    &prior_attempts,
                    &entry,
                    504,
                    false,
                    Some("stream idle timeout".to_string()),
                );
                log_usage(
                    &db, &app_handle, access_key.as_ref(), &entry, &requested_model,
                    true, prompt_tokens.load(Ordering::SeqCst),
                    completion_tokens.load(Ordering::SeqCst),
                    first_token_ms.load(Ordering::SeqCst),
                    start.elapsed().as_millis() as i64,
                    504, false, Some("stream idle timeout"),
                    Some(attempt_path.as_str()), Some(StreamEndReason::Timeout),
                );
                spawn_cool_down_entry(circuit_breakers.clone(), settings_db.clone(), entries_app_handle.clone(), entry_id.clone());
            }
            return Poll::Ready(Some(Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "stream idle timeout",
            ))));
        }

        match upstream_stream.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(chunk))) => {
                idle_timeout.as_mut().reset(tokio::time::Instant::now() + STREAMING_IDLE_TIMEOUT);
                if !seen_first_chunk.swap(true, Ordering::SeqCst) {
                    first_token_ms.store(start.elapsed().as_millis() as i64, Ordering::SeqCst);
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
                if !logged.swap(true, Ordering::SeqCst) {
                    let error_message = format!("Stream error: {err}");
                    let attempt_path = attempt_path_with_current(
                        &prior_attempts,
                        &entry,
                        502,
                        false,
                        Some(error_message.clone()),
                    );
                    log_usage(
                        &db, &app_handle, access_key.as_ref(), &entry, &requested_model,
                        true, prompt_tokens.load(Ordering::SeqCst),
                        completion_tokens.load(Ordering::SeqCst),
                        first_token_ms.load(Ordering::SeqCst),
                        start.elapsed().as_millis() as i64,
                        502, false, Some(error_message.as_str()),
                        Some(attempt_path.as_str()), Some(StreamEndReason::UpstreamError),
                    );
                    spawn_cool_down_entry(circuit_breakers.clone(), settings_db.clone(), entries_app_handle.clone(), entry_id.clone());
                }
                Poll::Ready(Some(Err(std::io::Error::new(std::io::ErrorKind::Other, err))))
            }
            Poll::Ready(None) => {
                if !logged.swap(true, Ordering::SeqCst) {
                    let attempt_path = attempt_path_with_current(
                        &prior_attempts,
                        &entry,
                        status_code,
                        true,
                        None,
                    );
                    log_usage(
                        &db, &app_handle, access_key.as_ref(), &entry, &requested_model,
                        true, prompt_tokens.load(Ordering::SeqCst),
                        completion_tokens.load(Ordering::SeqCst),
                        first_token_ms.load(Ordering::SeqCst),
                        start.elapsed().as_millis() as i64,
                        status_code, true, None,
                        Some(attempt_path.as_str()), Some(StreamEndReason::Done),
                    );
                    spawn_record_circuit_success(success_circuit_breakers.clone(), settings_db.clone(), entries_app_handle.clone(), entry_id.clone());
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
        .header("x-accel-buffering", "no")
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
            output.push(b"data: [DONE]\n\n".to_vec());
            continue;
        }

        let (prompt, completion) = adapter.extract_sse_usage(payload);
        if prompt > 0 { prompt_tokens.store(prompt, Ordering::Relaxed); }
        if completion > 0 { completion_tokens.store(completion, Ordering::Relaxed); }

        if let Some(transformed) = adapter.transform_sse_line(payload) {
            output.push(format!("data: {transformed}\n\n").into_bytes());
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

fn refresh_tray(app_handle: &tauri::AppHandle) {
    if let Ok(new_menu) = build_tray_menu(app_handle) {
        if let Some(tray) = app_handle.tray_by_id(TRAY_ID) {
            let _ = tray.set_menu(Some(new_menu));
        }
    }
}

fn status_matches_rule(rule: &str, status: u16) -> bool {
    let rule = rule.trim();
    if rule.is_empty() {
        return false;
    }

    if let Some((start, end)) = rule.split_once('-') {
        let Ok(start) = start.trim().parse::<u16>() else { return false; };
        let Ok(end) = end.trim().parse::<u16>() else { return false; };
        return status >= start && status <= end;
    }

    rule.parse::<u16>() == Ok(status)
}

fn should_disable_entry_for_status(disable_codes: &str, status: u16) -> bool {
    disable_codes
        .split(',')
        .any(|rule| status_matches_rule(rule, status))
}

async fn disable_entry(state: &ProxyState, entry: &ApiEntry) {
    let recovery_secs = state
        .db
        .get_settings()
        .ok()
        .map(|s| s.circuit_recovery_secs)
        .unwrap_or(300)
        .max(1);
    let cooldown_until = chrono::Utc::now().timestamp() + recovery_secs;

    let _ = state.db.toggle_entry(&entry.id, false);
    let _ = state.db.set_entry_cooldown(&entry.id, Some(cooldown_until));
    let _ = state.app_handle.emit("entries-changed", ());
    refresh_tray(&state.app_handle);

    let mut breakers = state.circuit_breakers.write().await;
    breakers.remove(&entry.id);
}

async fn record_circuit_success(state: &ProxyState, entry_id: &str) {
    let _ = state.db.set_entry_cooldown(entry_id, None);
    let _ = state.app_handle.emit("entries-changed", ());
    refresh_tray(&state.app_handle);

    let mut breakers = state.circuit_breakers.write().await;
    let recovery_secs = state.db.get_settings().ok()
        .map(|s| s.circuit_recovery_secs as u64).unwrap_or(300);

    let cb = breakers
        .entry(entry_id.to_string())
        .or_insert_with(|| CircuitBreaker::new(recovery_secs));
    cb.record_success();
}

async fn cool_down_entry(state: &ProxyState, entry: &ApiEntry) {
    let settings = state.db.get_settings().ok();
    let recovery_secs = settings
        .as_ref()
        .map(|s| s.circuit_recovery_secs)
        .unwrap_or(300)
        .max(1);
    let cooldown_until = chrono::Utc::now().timestamp() + recovery_secs;
    let _ = state.db.set_entry_cooldown(&entry.id, Some(cooldown_until));
    let _ = state.app_handle.emit("entries-changed", ());
    refresh_tray(&state.app_handle);

    let mut breakers = state.circuit_breakers.write().await;
    let threshold = settings
        .as_ref()
        .map(|s| s.circuit_failure_threshold as u32)
        .unwrap_or(1)
        .max(1);
    let recovery_secs = settings
        .as_ref()
        .map(|s| s.circuit_recovery_secs as u64)
        .unwrap_or(300);

    let cb = breakers
        .entry(entry.id.clone())
        .or_insert_with(|| CircuitBreaker::new(recovery_secs));
    cb.set_recovery_secs(recovery_secs);
    cb.record_failure(threshold);
}

fn spawn_record_circuit_success(
    circuit_breakers: Arc<tokio::sync::RwLock<std::collections::HashMap<String, CircuitBreaker>>>,
    db: Arc<Database>,
    app_handle: tauri::AppHandle,
    entry_id: String,
) {
    tokio::spawn(async move {
        let recovery_secs = db
            .get_settings()
            .ok()
            .map(|s| s.circuit_recovery_secs as u64)
            .unwrap_or(300);

        let _ = db.set_entry_cooldown(&entry_id, None);
        let _ = app_handle.emit("entries-changed", ());
        refresh_tray(&app_handle);

        let mut breakers = circuit_breakers.write().await;
        let cb = breakers
            .entry(entry_id)
            .or_insert_with(|| CircuitBreaker::new(recovery_secs));
        cb.set_recovery_secs(recovery_secs);
        cb.record_success();
    });
}

fn spawn_cool_down_entry(
    circuit_breakers: Arc<tokio::sync::RwLock<std::collections::HashMap<String, CircuitBreaker>>>,
    db: Arc<Database>,
    app_handle: tauri::AppHandle,
    entry_id: String,
) {
    tokio::spawn(async move {
        let settings = db.get_settings().ok();
        let threshold = settings
            .as_ref()
            .map(|s| s.circuit_failure_threshold as u32)
            .unwrap_or(1)
            .max(1);
        let recovery_secs = settings
            .as_ref()
            .map(|s| s.circuit_recovery_secs as u64)
            .unwrap_or(300);

        let cooldown_until = chrono::Utc::now().timestamp() + recovery_secs as i64;
        let _ = db.set_entry_cooldown(&entry_id, Some(cooldown_until));
        let _ = app_handle.emit("entries-changed", ());
        refresh_tray(&app_handle);

        let mut breakers = circuit_breakers.write().await;
        let cb = breakers
            .entry(entry_id)
            .or_insert_with(|| CircuitBreaker::new(recovery_secs));
        cb.set_recovery_secs(recovery_secs);
        cb.record_failure(threshold);
    });
}

fn log_usage(
    db: &Database,
    app_handle: &tauri::AppHandle,
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
    attempt_path: Option<&str>,
    stream_end_reason: Option<StreamEndReason>,
) {
    let log_type = if success { 2 } else { 5 };
    let content = error_message.unwrap_or("");
    let token_name = access_key.map(|ak| ak.name.as_str()).unwrap_or("auto");
    let use_time = ((latency_ms as f64) / 1000.0).ceil() as i64;
    let other = serde_json::json!({
        "requested_model": requested_model,
        "resolved_model": entry.model,
        "first_token_ms": first_token_ms,
        "status_code": status_code,
        "success": success,
        "attempt_path": attempt_path.and_then(|path| serde_json::from_str::<Value>(path).ok()),
        "stream_end_reason": stream_end_reason.map(StreamEndReason::as_str),
    })
    .to_string();

    let _ = db.insert_usage_log(
        log_type, content,
        access_key.map(|ak| ak.id.as_str()),
        access_key.map(|ak| ak.name.as_str()).unwrap_or("auto"),
        token_name, &entry.id, &entry.channel_id,
        entry.channel_name.as_deref().unwrap_or("unknown"),
        &entry.model, requested_model, 0, is_stream,
        prompt_tokens, completion_tokens, latency_ms,
        first_token_ms, use_time, status_code, success,
        "", "default", &other, error_message, None,
    );

    let _ = app_handle.emit("new-usage-log", ());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proxy::protocol::get_adapter;

    #[test]
    fn transformed_sse_chunks_are_standard_sse_frames() {
        let adapter = get_adapter("claude");
        let chunk = Bytes::from_static(
            b"data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"model\":\"claude-3\"}}\n\
data: [DONE]\n"
        );
        let mut buffer = String::new();
        let prompt_tokens = Arc::new(AtomicI64::new(0));
        let completion_tokens = Arc::new(AtomicI64::new(0));

        let output = transform_sse_chunk(
            &chunk,
            &mut buffer,
            &adapter,
            &prompt_tokens,
            &completion_tokens,
        )
        .expect("transformed output");
        let output = String::from_utf8(output.to_vec()).expect("valid utf8");

        assert!(output.contains("\n\n"));
        assert!(output.ends_with("data: [DONE]\n\n"));
        assert!(!output.contains("data: [DONE]\n\ndata: [DONE]"));
    }
}
