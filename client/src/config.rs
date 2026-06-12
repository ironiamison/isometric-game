/// HTTP endpoint compiled into this client artifact.
pub const SERVER_URL: &str = env!("AEVEN_COMPILED_SERVER_URL");

/// WebSocket endpoint compiled into this client artifact.
pub const WS_URL: &str = env!("AEVEN_COMPILED_WS_URL");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_endpoints_have_expected_schemes() {
        assert!(SERVER_URL.starts_with("http://") || SERVER_URL.starts_with("https://"));
        assert!(WS_URL.starts_with("ws://") || WS_URL.starts_with("wss://"));
        assert!(!SERVER_URL.ends_with('/'));
        assert!(!WS_URL.ends_with('/'));
    }
}
