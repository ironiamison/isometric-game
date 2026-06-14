//! Shared HTTP agent for the native client.
//!
//! Every blocking HTTP request (login, character CRUD, matchmaking, health
//! checks) goes through [`agent`]. These calls run on the render thread, so
//! without a deadline a stalled connection freezes the whole window — on
//! Windows the OS then paints it "(Not Responding)" until the socket finally
//! gives up. `ureq`'s default agent has *no* timeouts, which is exactly how a
//! player on a flaky uplink ends up with a hard-locked client instead of a
//! recoverable "couldn't connect" error. The agent below caps connection setup
//! and the overall request so a bad link fails fast.
//!
//! Debug builds also honour `AEVEN_NET_DELAY_MS` to reproduce the field report
//! locally; the matching WebSocket-drop knob lives in `network::client`.

use std::sync::OnceLock;
use std::time::Duration;

/// Max time to establish the TCP/TLS connection.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// Hard ceiling for the whole request (DNS + connect + send + response).
const OVERALL_TIMEOUT: Duration = Duration::from_secs(10);

static AGENT: OnceLock<ureq::Agent> = OnceLock::new();

/// The shared, timeout-configured [`ureq::Agent`]. Cheap to clone — the agent
/// is internally reference-counted and pools connections.
///
/// In debug builds this first applies `AEVEN_NET_DELAY_MS` latency injection
/// (see [`inject_latency`]) so the main-thread freeze can be reproduced.
pub fn agent() -> ureq::Agent {
    #[cfg(debug_assertions)]
    inject_latency();

    AGENT
        .get_or_init(|| {
            ureq::AgentBuilder::new()
                .timeout_connect(CONNECT_TIMEOUT)
                .timeout(OVERALL_TIMEOUT)
                .build()
        })
        .clone()
}

/// Sleep before a request to emulate a slow/janky uplink. Debug-only.
///
/// `AEVEN_NET_DELAY_MS=8000` reproduces the reported freeze: the matchmaking
/// POST runs on the render thread, so the window locks for the full delay.
///
/// Note this is a *client-side* stall, so it is not interrupted by the agent's
/// socket deadline above — it demonstrates the blocking-call problem itself. To
/// exercise the real connect/overall deadline against a stalled socket, build
/// the client pointed at an unreachable host, e.g.
/// `AEVEN_COMPILED_SERVER_URL=http://10.255.255.1:81 AEVEN_COMPILED_WS_URL=ws://10.255.255.1:81 cargo run`
/// — the connect aborts after `CONNECT_TIMEOUT` instead of hanging forever.
#[cfg(debug_assertions)]
fn inject_latency() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static WARNED: AtomicBool = AtomicBool::new(false);

    if let Some(ms) = std::env::var("AEVEN_NET_DELAY_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|ms| *ms > 0)
    {
        if !WARNED.swap(true, Ordering::Relaxed) {
            log::warn!(
                "[net-fault] AEVEN_NET_DELAY_MS={ms} — injecting {ms}ms latency into every HTTP request"
            );
        }
        std::thread::sleep(Duration::from_millis(ms));
    }
}
