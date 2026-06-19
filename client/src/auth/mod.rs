pub mod credentials;
pub mod session_storage;
pub mod types;

#[cfg(target_arch = "wasm32")]
pub mod wallet_wasm;

#[cfg(not(target_arch = "wasm32"))]
mod client;

#[cfg(target_arch = "wasm32")]
mod wasm_client;

#[cfg(not(target_arch = "wasm32"))]
pub use client::AuthClient;

#[cfg(target_arch = "wasm32")]
pub use wasm_client::{AuthClient, AuthResult};

#[cfg(target_arch = "wasm32")]
pub use wallet_wasm::{is_wallet_available, poll_wallet_sign, start_wallet_sign, WalletSignPoll, WalletSignResult};

pub use types::{AuthError, AuthSession, CharacterInfo, WalletChallenge};

#[cfg(target_arch = "wasm32")]
pub use session_storage::take_pending_auth_session;
