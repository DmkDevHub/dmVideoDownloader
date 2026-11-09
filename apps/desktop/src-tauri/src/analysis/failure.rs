// Only fixed error codes cross the UI boundary. Diagnostics can contain signed URLs.
pub fn classify(stderr: &[u8]) -> &'static str {
    let text = String::from_utf8_lossy(stderr);
    let mut errors = text
        .lines()
        .filter(|line| line.starts_with("ERROR:"))
        .peekable();
    if errors.peek().is_none() {
        return "ANALYSIS_FAILED";
    }
    let signals: Vec<_> = errors
        .map(|line| {
            line.split_whitespace()
                .filter(|part| !part.contains("://"))
                .collect::<Vec<_>>()
                .join(" ")
                .to_ascii_lowercase()
        })
        .collect();
    let has = |markers: &[&str]| {
        signals
            .iter()
            .any(|line| markers.iter().any(|marker| line.contains(marker)))
    };
    if has(&[
        "drm protected",
        "drm-protected",
        "drm protection",
        "protected by drm",
    ]) {
        return "DRM_PROTECTED";
    }
    if has(&[
        "http error 401",
        "401 client error",
        "login required",
        "requires authentication",
        "sign in to access",
        "log in to access",
    ]) {
        return "AUTH_REQUIRED";
    }
    if has(&[
        "http error 410",
        "410 client error",
        "url has expired",
        "link has expired",
        "expired signature",
        "signature has expired",
    ]) {
        return "EXPIRED_OR_GONE";
    }
    if has(&["http error 403", "403 client error", "http error 451"]) {
        return "ACCESS_DENIED";
    }
    if has(&["http error 404", "404 client error"]) {
        return "NOT_FOUND";
    }
    if has(&["timed out", "timeout error", "timeoutexception"]) {
        return "ANALYSIS_TIMEOUT";
    }
    if has(&[
        "connection aborted",
        "connection reset",
        "connection refused",
        "unable to resolve",
        "certificate verify failed",
        "http error 502",
        "http error 503",
        "http error 504",
        "transporterror",
    ]) {
        return "NETWORK_ERROR";
    }
    if signals
        .iter()
        .any(|line| line.starts_with("error: unsupported url:"))
    {
        return "UNSUPPORTED_URL";
    }
    if has(&["no video formats found", "no video could be found"]) {
        return "NO_FORMATS";
    }
    "ANALYSIS_FAILED"
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn maps_only_evidence_and_keeps_ambiguous_denials_separate() {
        for (message, expected) in [
            ("ERROR: [generic] HTTP Error 401: Unauthorized", "AUTH_REQUIRED"),
            ("ERROR: This video is DRM protected. No video formats found", "DRM_PROTECTED"),
            ("ERROR: HTTP Error 410: Gone", "EXPIRED_OR_GONE"),
            ("ERROR: HTTP Error 403: Forbidden https://example.org/url%20has%20expired?token=fixture", "ACCESS_DENIED"),
            ("ERROR: HTTP Error 404: Not Found", "NOT_FOUND"),
            ("ERROR: Unsupported URL: https://example.org/expired-signature", "UNSUPPORTED_URL"),
            ("ERROR: TransportError: connection reset", "NETWORK_ERROR"),
            ("ERROR: The read operation timed out", "ANALYSIS_TIMEOUT"),
            ("ERROR: No video formats found", "NO_FORMATS"),
            ("WARNING: login required", "ANALYSIS_FAILED"),
            ("ERROR: unfamiliar response", "ANALYSIS_FAILED"),
        ] {
            assert_eq!(classify(message.as_bytes()), expected);
        }
    }
}
