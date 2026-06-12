use axum::http::{HeaderMap, HeaderValue};
use std::collections::HashSet;
use std::env;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

const DEFAULT_PRODUCTION_ORIGIN: &str = "https://aeven.xyz";

#[derive(Clone, Debug)]
pub(super) struct ServerConfig {
    pub bind_addr: SocketAddr,
    pub database_url: String,
    pub allowed_origins: Vec<HeaderValue>,
    pub admin_api_token: Option<Arc<str>>,
    pub session_signing_secret: Arc<[u8]>,
    pub auth_session_ttl: Duration,
    trusted_proxies: Arc<HashSet<IpAddr>>,
}

impl ServerConfig {
    pub fn from_env() -> Result<Self, String> {
        let environment = env::var("AEVEN_ENV").unwrap_or_else(|_| {
            if cfg!(debug_assertions) {
                "development".to_string()
            } else {
                "production".to_string()
            }
        });
        let is_production = environment.eq_ignore_ascii_case("production");

        let bind_addr = env::var("AEVEN_BIND_ADDR")
            .unwrap_or_else(|_| "0.0.0.0:2567".to_string())
            .parse()
            .map_err(|error| format!("invalid AEVEN_BIND_ADDR: {error}"))?;
        let database_url = env::var("AEVEN_DATABASE_URL")
            .unwrap_or_else(|_| "sqlite:game.db?mode=rwc".to_string());

        let allowed_origins_raw = env::var("AEVEN_ALLOWED_ORIGINS").unwrap_or_else(|_| {
            if is_production {
                DEFAULT_PRODUCTION_ORIGIN.to_string()
            } else {
                "http://localhost:5173,http://127.0.0.1:5173".to_string()
            }
        });
        let allowed_origins = parse_origins(&allowed_origins_raw)?;

        let admin_api_token = env::var("AEVEN_ADMIN_API_TOKEN")
            .ok()
            .filter(|token| !token.trim().is_empty())
            .map(|token| Arc::<str>::from(token.trim()));
        if admin_api_token
            .as_deref()
            .is_some_and(|token| token.len() < 32)
        {
            return Err("AEVEN_ADMIN_API_TOKEN must be at least 32 characters".to_string());
        }

        let session_signing_secret = match env::var("AEVEN_SESSION_SIGNING_SECRET") {
            Ok(secret) if secret.len() >= 32 => Arc::from(secret.into_bytes()),
            Ok(_) => {
                return Err("AEVEN_SESSION_SIGNING_SECRET must be at least 32 bytes".to_string());
            }
            Err(_) if is_production => {
                return Err("AEVEN_SESSION_SIGNING_SECRET is required in production".to_string());
            }
            Err(_) => {
                use rand::RngCore;
                let mut secret = vec![0u8; 32];
                rand::thread_rng().fill_bytes(&mut secret);
                Arc::from(secret)
            }
        };

        let auth_session_ttl_hours = env::var("AEVEN_AUTH_SESSION_TTL_HOURS")
            .unwrap_or_else(|_| "24".to_string())
            .parse::<u64>()
            .map_err(|error| format!("invalid AEVEN_AUTH_SESSION_TTL_HOURS: {error}"))?;
        if !(1..=720).contains(&auth_session_ttl_hours) {
            return Err("AEVEN_AUTH_SESSION_TTL_HOURS must be between 1 and 720".to_string());
        }

        let trusted_proxies = env::var("AEVEN_TRUSTED_PROXIES")
            .unwrap_or_default()
            .split(',')
            .filter_map(|value| {
                let value = value.trim();
                (!value.is_empty()).then_some(value)
            })
            .map(|value| {
                value
                    .parse::<IpAddr>()
                    .map_err(|error| format!("invalid trusted proxy IP '{value}': {error}"))
            })
            .collect::<Result<HashSet<_>, _>>()?;

        Ok(Self {
            bind_addr,
            database_url,
            allowed_origins,
            admin_api_token,
            session_signing_secret,
            auth_session_ttl: Duration::from_secs(auth_session_ttl_hours * 60 * 60),
            trusted_proxies: Arc::new(trusted_proxies),
        })
    }

    pub fn client_ip(&self, headers: &HeaderMap, peer: SocketAddr) -> IpAddr {
        if !self.trusted_proxies.contains(&peer.ip()) {
            return peer.ip();
        }

        forwarded_ip(headers)
            .or_else(|| x_forwarded_for_ip(headers))
            .unwrap_or_else(|| peer.ip())
    }
}

fn parse_origins(raw: &str) -> Result<Vec<HeaderValue>, String> {
    let origins = raw
        .split(',')
        .map(str::trim)
        .filter(|origin| !origin.is_empty())
        .map(|origin| {
            if origin == "*" {
                return Err("wildcard CORS origins are not allowed".to_string());
            }
            if !(origin.starts_with("https://")
                || origin.starts_with("http://localhost")
                || origin.starts_with("http://127.0.0.1"))
            {
                return Err(format!("insecure or invalid CORS origin: {origin}"));
            }
            HeaderValue::from_str(origin)
                .map_err(|error| format!("invalid CORS origin '{origin}': {error}"))
        })
        .collect::<Result<Vec<_>, _>>()?;

    if origins.is_empty() {
        return Err("AEVEN_ALLOWED_ORIGINS must contain at least one origin".to_string());
    }
    Ok(origins)
}

fn forwarded_ip(headers: &HeaderMap) -> Option<IpAddr> {
    let forwarded = headers.get("forwarded")?.to_str().ok()?;
    let first_hop = forwarded.split(',').next()?;
    let for_value = first_hop.split(';').find_map(|part| {
        let (name, value) = part.trim().split_once('=')?;
        name.eq_ignore_ascii_case("for")
            .then_some(value.trim().trim_matches('"'))
    })?;
    parse_forwarded_address(for_value)
}

fn x_forwarded_for_ip(headers: &HeaderMap) -> Option<IpAddr> {
    headers
        .get("x-forwarded-for")?
        .to_str()
        .ok()?
        .split(',')
        .next()?
        .trim()
        .parse()
        .ok()
}

fn parse_forwarded_address(value: &str) -> Option<IpAddr> {
    if let Ok(ip) = value.parse() {
        return Some(ip);
    }
    if let Ok(socket) = value.parse::<SocketAddr>() {
        return Some(socket.ip());
    }
    value
        .strip_prefix('[')?
        .split_once(']')
        .and_then(|(ip, _)| ip.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config(trusted_proxies: HashSet<IpAddr>) -> ServerConfig {
        ServerConfig {
            bind_addr: "127.0.0.1:2567".parse().unwrap(),
            database_url: "sqlite::memory:".to_string(),
            allowed_origins: vec!["http://localhost:5173".parse().unwrap()],
            admin_api_token: None,
            session_signing_secret: Arc::from(vec![1; 32]),
            auth_session_ttl: Duration::from_secs(3600),
            trusted_proxies: Arc::new(trusted_proxies),
        }
    }

    #[test]
    fn untrusted_peers_cannot_spoof_forwarding_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "203.0.113.7".parse().unwrap());
        let config = test_config(HashSet::new());

        assert_eq!(
            config.client_ip(&headers, "198.51.100.4:1234".parse().unwrap()),
            "198.51.100.4".parse::<IpAddr>().unwrap()
        );
    }

    #[test]
    fn trusted_peer_uses_forwarded_client_ip() {
        let mut headers = HeaderMap::new();
        headers.insert("forwarded", "for=203.0.113.7;proto=https".parse().unwrap());
        let mut proxies = HashSet::new();
        proxies.insert("127.0.0.1".parse().unwrap());
        let config = test_config(proxies);

        assert_eq!(
            config.client_ip(&headers, "127.0.0.1:1234".parse().unwrap()),
            "203.0.113.7".parse::<IpAddr>().unwrap()
        );
    }
}
