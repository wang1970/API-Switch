use crate::database::AccessKey;
use crate::database::Database;
use crate::error::AppError;
use axum::http::HeaderMap;

/// Extract Access Key from request headers.
/// If access_key_required is enabled, validate the key.
/// Otherwise, just use it for identity tracking.
pub fn extract_access_key(
    headers: &HeaderMap,
    db: &Database,
) -> Result<Option<AccessKey>, AppError> {
    let settings = db.get_settings()?;

    let auth_header = headers.get("authorization").and_then(|v| v.to_str().ok());

    let key_str = auth_header.and_then(|a| {
        let stripped = a
            .strip_prefix("Bearer ")
            .or_else(|| a.strip_prefix("bearer "))
            .unwrap_or(a);
        Some(stripped)
    });

    // Don't treat empty or "auto" as a real key
    let key_str = key_str.filter(|k| !k.is_empty() && k != &"auto");

    let access_key = if let Some(key) = key_str {
        db.find_access_key_by_key(key)?
    } else {
        None
    };

    if settings.access_key_required {
        // Validation enabled: must have a valid enabled key
        match access_key {
            Some(ak) if ak.enabled => Ok(Some(ak)),
            _ => Err(AppError::Validation(
                "Valid Access Key required".to_string(),
            )),
        }
    } else {
        // Validation disabled: just track identity
        Ok(access_key)
    }
}
