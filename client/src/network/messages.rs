pub use aeven_protocol::ClientMessage;

pub fn enter_portal(portal_id: &str) -> ClientMessage {
    ClientMessage::EnterPortal {
        portal_id: portal_id.to_string(),
    }
}
