use std::env;

const LOCAL_HTTP_URL: &str = "http://localhost:2567";
const LOCAL_WS_URL: &str = "ws://localhost:2567";
const PRODUCTION_HTTP_URL: &str = "https://aeven.xyz";
const PRODUCTION_WS_URL: &str = "wss://aeven.xyz";

fn main() {
    println!("cargo:rerun-if-env-changed=AEVEN_SERVER_URL");
    println!("cargo:rerun-if-env-changed=AEVEN_WS_URL");
    println!("cargo:rerun-if-env-changed=AEVEN_ALLOW_INSECURE_ENDPOINTS");

    let profile = env::var("PROFILE").expect("Cargo must set PROFILE");
    let is_release = profile == "release" || profile == "release-wasm";
    let server_url = env::var("AEVEN_SERVER_URL").unwrap_or_else(|_| {
        if is_release {
            PRODUCTION_HTTP_URL.to_string()
        } else {
            LOCAL_HTTP_URL.to_string()
        }
    });
    let ws_url = env::var("AEVEN_WS_URL").unwrap_or_else(|_| {
        if is_release {
            PRODUCTION_WS_URL.to_string()
        } else {
            LOCAL_WS_URL.to_string()
        }
    });

    validate_url("AEVEN_SERVER_URL", &server_url, &["http://", "https://"]);
    validate_url("AEVEN_WS_URL", &ws_url, &["ws://", "wss://"]);

    let allow_insecure = env::var("AEVEN_ALLOW_INSECURE_ENDPOINTS").as_deref() == Ok("1");
    if is_release && !allow_insecure {
        if server_url.starts_with("http://") || ws_url.starts_with("ws://") {
            panic!(
                "release clients require HTTPS/WSS endpoints; set \
                 AEVEN_ALLOW_INSECURE_ENDPOINTS=1 only for an intentional private build"
            );
        }
        if is_local_endpoint(&server_url) || is_local_endpoint(&ws_url) {
            panic!("release clients must not target localhost or loopback endpoints");
        }
    }

    println!("cargo:rustc-env=AEVEN_COMPILED_SERVER_URL={server_url}");
    println!("cargo:rustc-env=AEVEN_COMPILED_WS_URL={ws_url}");
}

fn validate_url(name: &str, value: &str, allowed_schemes: &[&str]) {
    if value.ends_with('/') {
        panic!("{name} must not end with '/'");
    }
    if !allowed_schemes
        .iter()
        .any(|scheme| value.starts_with(scheme))
    {
        panic!("{name} has an invalid scheme: {value}");
    }
}

fn is_local_endpoint(value: &str) -> bool {
    let authority = value
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(value)
        .split('/')
        .next()
        .unwrap_or_default();
    let host = if let Some(ipv6) = authority.strip_prefix('[') {
        ipv6.split_once(']').map(|(host, _)| host).unwrap_or(ipv6)
    } else {
        authority.split(':').next().unwrap_or_default()
    };

    matches!(host, "localhost" | "127.0.0.1" | "::1" | "0.0.0.0")
}
