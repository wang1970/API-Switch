use super::circuit_breaker::CircuitBreaker;
use crate::database::ApiEntry;
use std::collections::HashMap;
use tokio::sync::RwLock;

/// Resolve which entries to try for a given model request.
/// Returns an ordered list of entries to attempt (failover in order).
///
/// Rules:
/// - `auto`: use enabled entries only (auto-select pool).
/// - exact model match: try matched enabled entries first,
///   then fall back to enabled entries as auto-fallback to prevent disconnection.
/// - wrong model name: fall back to enabled entries (AUTO behavior).
///
/// This intentionally follows NEW-API's enabled-channel pool behavior: disabled
/// entries must never be selected by the formal proxy route. Testing disabled
/// entries is handled by the test-chat command/path only.
pub async fn resolve(
    model: &str,
    enabled_entries: &[ApiEntry],
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

    // Exact model match from ENABLED entries only. This mirrors NEW-API's
    // channel cache, which excludes disabled channels from formal routing.
    let enabled_available = filter_available(enabled_entries);
    let matched: Vec<ApiEntry> = enabled_available
        .iter()
        .filter(|e| e.model == model)
        .cloned()
        .collect();

    if matched.is_empty() {
        // Wrong model name → fallback to AUTO (enabled entries)
        return enabled_available;
    }

    // Exact match found: try matched entries first,
    // then append enabled entries as auto-fallback to prevent disconnection.
    let mut result = matched;
    for entry in &enabled_available {
        if !result.iter().any(|e| e.id == entry.id) {
            result.push(entry.clone());
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, model: &str, enabled: bool, sort_index: i32) -> ApiEntry {
        ApiEntry {
            id: id.to_string(),
            channel_id: format!("channel-{id}"),
            model: model.to_string(),
            display_name: model.to_string(),
            sort_index,
            enabled,
            cooldown_until: None,
            circuit_state: "closed".to_string(),
            created_at: 0,
            updated_at: 0,
            channel_name: Some(format!("channel-{id}")),
            channel_api_type: Some("openai".to_string()),
            owned_by: None,
        }
    }

    #[tokio::test]
    async fn auto_uses_enabled_entries_in_order() {
        let breakers = RwLock::new(HashMap::new());
        let enabled = vec![
            entry("first", "gpt-4o", true, 0),
            entry("second", "claude-3", true, 1),
            entry("third", "gemini-pro", true, 2),
        ];

        let resolved = resolve("auto", &enabled, &breakers).await;

        assert_eq!(
            resolved.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(),
            vec!["first", "second", "third"]
        );
    }

    #[tokio::test]
    async fn exact_model_matches_enabled_entries_only() {
        let breakers = RwLock::new(HashMap::new());
        let enabled = vec![
            entry("match", "gpt-4o", true, 0),
            entry("fallback", "claude-3", true, 1),
        ];

        let resolved = resolve("gpt-4o", &enabled, &breakers).await;

        assert_eq!(
            resolved.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(),
            vec!["match", "fallback"]
        );
    }

    #[tokio::test]
    async fn exact_model_without_enabled_match_falls_back_to_auto_pool() {
        let breakers = RwLock::new(HashMap::new());
        let enabled = vec![entry("fallback", "claude-3", true, 1)];

        let resolved = resolve("gpt-4o", &enabled, &breakers).await;

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].id, "fallback");
    }

    #[tokio::test]
    async fn circuit_open_entries_are_skipped() {
        let breakers = RwLock::new(HashMap::new());
        let enabled = vec![
            entry("open", "gpt-4o", true, 0),
            entry("fallback", "claude-3", true, 1),
        ];
        {
            let mut guard = breakers.write().await;
            let cb = CircuitBreaker::new(60);
            cb.record_failure(1);
            guard.insert("open".to_string(), cb);
        }

        let resolved = resolve("gpt-4o", &enabled, &breakers).await;

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].id, "fallback");
    }
}
