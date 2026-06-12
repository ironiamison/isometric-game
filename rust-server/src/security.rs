use super::*;

/// Rate limiter entry: (request_count, window_start_time)
type RateLimitEntry = (u32, std::time::Instant);

// ============================================================================
// Signed Session Tokens (Security Hardening)
// ============================================================================

type HmacSha256 = Hmac<Sha256>;

/// Session token validity duration
const SESSION_TOKEN_EXPIRY_SECS: u64 = 300; // 5 minutes

/// Signed session token generator/validator
#[derive(Clone)]
pub(super) struct SessionTokenSigner {
    secret: Arc<[u8]>,
}

impl SessionTokenSigner {
    pub(super) fn new(secret: Arc<[u8]>) -> Self {
        Self { secret }
    }

    /// Create a signed session token
    /// Format: base64(session_id:room_id:expiry_ts:signature)
    pub(super) fn create_token(&self, session_id: &str, room_id: &str) -> String {
        use base64::Engine;

        let expiry = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + SESSION_TOKEN_EXPIRY_SECS;

        let payload = format!("{}:{}:{}", session_id, room_id, expiry);

        let mut mac = HmacSha256::new_from_slice(&self.secret).expect("validated HMAC secret");
        mac.update(payload.as_bytes());
        let signature = mac.finalize().into_bytes();

        let token_data = format!(
            "{}:{}",
            payload,
            base64::engine::general_purpose::STANDARD.encode(signature)
        );
        base64::engine::general_purpose::URL_SAFE.encode(token_data)
    }

    /// Validate a signed session token
    /// Returns Some((session_id, room_id)) if valid, None if invalid/expired
    pub(super) fn validate_token(&self, token: &str) -> Option<(String, String)> {
        use base64::Engine;

        // Decode base64
        let token_data = base64::engine::general_purpose::URL_SAFE
            .decode(token)
            .ok()?;
        let token_str = String::from_utf8(token_data).ok()?;

        // Parse: session_id:room_id:expiry:signature
        let parts: Vec<&str> = token_str.splitn(4, ':').collect();
        if parts.len() != 4 {
            return None;
        }

        let session_id = parts[0];
        let room_id = parts[1];
        let expiry: u64 = parts[2].parse().ok()?;
        let signature_b64 = parts[3];

        // Check expiry
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        if now > expiry {
            warn!("Session token expired: {} > {}", now, expiry);
            return None;
        }

        // Verify signature
        let payload = format!("{}:{}:{}", session_id, room_id, expiry);
        let expected_sig = base64::engine::general_purpose::STANDARD
            .decode(signature_b64)
            .ok()?;

        let mut mac = HmacSha256::new_from_slice(&self.secret).expect("validated HMAC secret");
        mac.update(payload.as_bytes());

        if mac.verify_slice(&expected_sig).is_err() {
            warn!("Session token signature invalid");
            return None;
        }

        Some((session_id.to_string(), room_id.to_string()))
    }
}

/// Simple IP-based rate limiter
#[derive(Clone)]
pub(super) struct RateLimiter {
    /// IP -> (request_count, window_start)
    entries: Arc<DashMap<String, RateLimitEntry>>,
    /// Max requests per window
    max_requests: u32,
    /// Window duration
    window_duration: Duration,
}

impl RateLimiter {
    pub(super) fn new(max_requests: u32, window_secs: u64) -> Self {
        Self {
            entries: Arc::new(DashMap::new()),
            max_requests,
            window_duration: Duration::from_secs(window_secs),
        }
    }

    /// Check if request is allowed. Returns true if allowed, false if rate limited.
    pub(super) fn check(&self, ip: &str) -> bool {
        let now = std::time::Instant::now();

        let mut entry = self.entries.entry(ip.to_string()).or_insert((0, now));
        let (count, window_start) = entry.value_mut();

        // Reset window if expired
        if now.duration_since(*window_start) > self.window_duration {
            *count = 0;
            *window_start = now;
        }

        // Check limit
        if *count >= self.max_requests {
            return false;
        }

        *count += 1;
        true
    }

    /// Record a failed login attempt (for stricter limiting on failures)
    pub(super) fn record_failure(&self, ip: &str) {
        let now = std::time::Instant::now();
        let mut entry = self.entries.entry(ip.to_string()).or_insert((0, now));
        let (count, _) = entry.value_mut();
        // Add extra penalty for failures
        *count = (*count).saturating_add(2);
    }

    pub(super) fn prune_expired(&self) {
        let now = std::time::Instant::now();
        self.entries
            .retain(|_, (_, started)| now.duration_since(*started) <= self.window_duration);
    }
}
