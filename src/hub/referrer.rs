//! Resolve optional referrer addresses for ride transactions.

/// Use the client-supplied referrer when non-empty; otherwise fall back to configured default.
pub fn resolve_referrer(client: Option<String>, default: &str) -> Option<String> {
    if let Some(r) = client {
        let trimmed = r.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    let d = default.trim();
    if d.is_empty() {
        None
    } else {
        Some(d.to_string())
    }
}
