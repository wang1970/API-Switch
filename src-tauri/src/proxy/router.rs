use super::circuit_breaker::CircuitBreaker;
use crate::database::ApiEntry;
use std::collections::HashMap;
use tokio::sync::RwLock;

/// Resolve which entries to try for a given model request.
/// Returns an ordered list of entries to attempt (failover in order).
///
/// Rules:
/// - `auto`: use enabled entries only (auto-select pool).
/// - exact model match: try matched entries (all, including disabled) first,
///   then fall back to enabled entries as auto-fallback to prevent disconnection.
/// - wrong model name: fall back to enabled entries (AUTO behavior).
///
/// `enabled_entries`: entries with enabled=1 (AUTO pool).
/// `all_entries`: all entries including disabled (exact match pool).
pub async fn resolve(
    model: &str,
    enabled_entries: &[ApiEntry],
    all_entries: &[ApiEntry],
    circuit_breakers: &RwLock<HashMap<String, CircuitBreaker>>,
) -> Vec<ApiEntry> {
    let breakers = circuit_breakers.read().await;

    // Helper: filter out circuit-open entries
    let filter_available = |entries: &[ApiEntry]| -> Vec<ApiEntry> {
        entries
            .iter()
            .filter(|e| {
                if let Some(cb) = breakers.get(&e.id) {
                    cb.is_available()
                } else {
                    true
                }
            })
            .cloned()
            .collect()
    };

    if model == "auto" {
        // AUTO: only enabled + available entries
        return filter_available(enabled_entries);
    }

    // Exact model match from ALL entries (including disabled)
    let all_available = filter_available(all_entries);
    let matched: Vec<ApiEntry> = all_available
        .iter()
        .filter(|e| e.model == model)
        .cloned()
        .collect();

    if matched.is_empty() {
        // Wrong model name → fallback to AUTO (enabled entries)
        return filter_available(enabled_entries);
    }

    // Exact match found: try matched entries first,
    // then append enabled entries as auto-fallback to prevent disconnection.
    let enabled_available = filter_available(enabled_entries);
    let mut result = matched;
    for entry in &enabled_available {
        if !result.iter().any(|e| e.id == entry.id) {
            result.push(entry.clone());
        }
    }
    result
}
