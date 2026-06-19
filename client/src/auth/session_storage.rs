#[cfg(target_arch = "wasm32")]
use super::types::{AuthSession, CharacterInfo};
#[cfg(not(target_arch = "wasm32"))]
use super::types::AuthSession;

#[cfg(target_arch = "wasm32")]
use sapp_jsutils::JsObject;

const SESSION_KEY: &str = "solstead_auth_session";

#[cfg(target_arch = "wasm32")]
extern "C" {
    fn storage_get(key: JsObject) -> JsObject;
}

/// Read the browser title-screen auth session (kept in localStorage for site pages).
#[cfg(target_arch = "wasm32")]
pub fn take_pending_auth_session() -> Option<AuthSession> {
    unsafe {
        let obj = storage_get(JsObject::string(SESSION_KEY));
        if obj.is_nil() {
            return None;
        }
        let mut json = String::new();
        obj.to_string(&mut json);
        if json.is_empty() {
            return None;
        }

        #[derive(serde::Deserialize)]
        struct StoredSession {
            token: String,
            username: String,
            characters: Vec<CharacterInfo>,
        }

        serde_json::from_str::<StoredSession>(&json)
            .ok()
            .map(|s| AuthSession {
                token: s.token,
                username: s.username,
                characters: s.characters,
            })
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn take_pending_auth_session() -> Option<AuthSession> {
    None
}
