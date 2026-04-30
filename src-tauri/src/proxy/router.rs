use super::circuit_breaker::CircuitBreaker;
use crate::database::ApiEntry;
use std::collections::HashMap;
use tokio::sync::RwLock;

/// Parse response_ms field (stored as milliseconds string, e.g. "1234") to i64.
/// Returns None if missing or unparseable.
fn parse_response_ms(entry: &ApiEntry) -> Option<i64> {
    entry
        .response_ms
        .as_deref()
        .and_then(|s| s.parse::<i64>().ok())
}

/// Sort entries by response time ascending; entries without measurement go last.
fn sort_by_latency(entries: &mut [ApiEntry]) {
    entries.sort_by_key(|e| parse_response_ms(e).unwrap_or(i64::MAX));
}

/// Sort entries by sort_index ascending (user's custom order).
fn sort_by_index(entries: &mut [ApiEntry]) {
    entries.sort_by_key(|e| e.sort_index);
}

/// Sort entries by release date descending; entries without release date go last.
fn sort_by_release_date(entries: &mut [ApiEntry]) {
    entries.sort_by(|a, b| {
        let date_cmp = b
            .release_date
            .as_deref()
            .unwrap_or("")
            .cmp(a.release_date.as_deref().unwrap_or(""));
        if date_cmp == std::cmp::Ordering::Equal {
            a.sort_index.cmp(&b.sort_index)
        } else {
            date_cmp
        }
    });
}

/// Resolve which entries to try for a given model request.
/// Returns an ordered list of entries to attempt (failover in order).
///
/// Rules:
/// - `auto`: use enabled entries only (auto-select pool), sorted by `sort_mode`.
/// - exact model match: try matched enabled entries first (sorted by `sort_mode`),
///   then fall back to enabled entries as auto-fallback to prevent disconnection.
/// - wrong model name: fall back to enabled entries (AUTO behavior).
pub async fn resolve(
    model: &str,
    enabled_entries: &[ApiEntry],
    circuit_breakers: &RwLock<HashMap<String, CircuitBreaker>>,
    sort_mode: &str,
) -> Vec<ApiEntry> {
    let breakers = circuit_breakers.read().await;

    // Helper: filter out circuit-open entries, then sort by sort_mode
    let filter_available = |entries: &[ApiEntry]| -> Vec<ApiEntry> {
        let mut available: Vec<ApiEntry> = entries
            .iter()
            .filter(|e| {
                if let Some(cb) = breakers.get(&e.id) {
                    cb.is_available()
                } else {
                    true
                }
            })
            .cloned()
            .collect();
        apply_sort_mode(&mut available, sort_mode);
        available
    };

    if model.is_empty() || model.eq_ignore_ascii_case("auto") {
        // AUTO: only enabled + available entries, sorted by sort_mode
        return filter_available(enabled_entries);
    }

    // Exact model match from ENABLED entries only.
    let mut enabled_available = filter_available(enabled_entries);
    let mut matched: Vec<ApiEntry> = enabled_available
        .iter()
        .filter(|e| e.model == model)
        .cloned()
        .collect();

    if matched.is_empty() {
        // Wrong model name → fallback to AUTO (enabled entries)
        return enabled_available;
    }

    // Exact match found: try matched entries first,
    // then append remaining enabled entries as auto-fallback.
    apply_sort_mode(&mut matched, sort_mode);
    let mut result = matched;
    for entry in &enabled_available {
        if !result.iter().any(|e| e.id == entry.id) {
            result.push(entry.clone());
        }
    }
    result
}

/// Apply sort mode to entries: "custom" → sort_index, "fastest" → latency, "latest" → release_date.
pub(crate) fn apply_sort_mode(entries: &mut [ApiEntry], sort_mode: &str) {
    match sort_mode {
        "fastest" => sort_by_latency(entries),
        "latest" => sort_by_release_date(entries),
        _ => sort_by_index(entries),
    }
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
            response_ms: None,
            owned_by: None,
            provider_logo: None,
            release_date: None,
            model_meta_zh: None,
            model_meta_en: None,
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

        let resolved = resolve("auto", &enabled, &breakers, "custom").await;

        assert_eq!(resolved.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), vec!["first", "second", "third"]);
    }

    #[tokio::test]
    async fn exact_model_matches_enabled_entries_only() {
        let breakers = RwLock::new(HashMap::new());
        let enabled = vec![
            entry("match", "gpt-4o", true, 0),
            entry("fallback", "claude-3", true, 1),
        ];

        let resolved = resolve("gpt-4o", &enabled, &breakers, "custom").await;

        assert_eq!(resolved.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), vec!["match", "fallback"]);
    }

    #[tokio::test]
    async fn exact_model_without_enabled_match_falls_back_to_auto_pool() {
        let breakers = RwLock::new(HashMap::new());
        let enabled = vec![entry("fallback", "claude-3", true, 1)];

        let resolved = resolve("gpt-4o", &enabled, &breakers, "custom").await;

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

        let resolved = resolve("gpt-4o", &enabled, &breakers, "custom").await;

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].id, "fallback");
    }

    #[tokio::test]
    async fn latest_sort_uses_release_date_descending() {
        let breakers = RwLock::new(HashMap::new());
        let mut older = entry("older", "old-model", true, 0);
        older.release_date = Some("2023-01".to_string());
        let mut newer = entry("newer", "new-model", true, 1);
        newer.release_date = Some("2024-08".to_string());
        let missing = entry("missing", "unknown-model", true, 2);
        let enabled = vec![older, missing, newer];

        let resolved = resolve("auto", &enabled, &breakers, "latest").await;

        assert_eq!(resolved.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), vec!["newer", "older", "missing"]);
    }
}
