/// Shared utilities used across protocol adapters.

/// Join a user-provided `base_url` with an endpoint `path`, deduplicating common
/// version prefixes (e.g. `/v1`, `/v1beta`) that the user may have already included.
pub fn join_url(base_url: &str, path: &str) -> String {
    let base = base_url.trim_end_matches('/');
    let lower = base.to_ascii_lowercase();
    // Strip trailing /v1beta or /v1 so we don't double them
    let base = if lower.ends_with("/v1beta") {
        &base[..base.len() - 7]
    } else if lower.ends_with("/v1") {
        &base[..base.len() - 3]
    } else {
        base
    };
    format!("{}/{}", base, path)
}
