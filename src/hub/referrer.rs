//! Referrer addresses for ride transactions are set only by Hub API configuration.

/// Returns the configured referrer address when non-empty.
pub fn configured_referrer(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
