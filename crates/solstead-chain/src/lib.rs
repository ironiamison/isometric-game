pub mod config;
pub mod indexer;
pub mod pda;
pub mod withdraw;

pub use config::ChainConfig;
pub use indexer::{DepositObservation, scan_vault_deposits};
pub use pda::{derive_vault, derive_vault_token_account};
pub use withdraw::{
    WithdrawTicket, build_ed25519_verify_instruction, build_withdraw_transaction,
    sign_withdraw_ticket, withdraw_message,
};

/// SPL token decimals used for Solstead devnet mint.
pub const TOKEN_DECIMALS: u8 = 6;

pub fn ui_to_base_units(amount_ui: f64) -> u64 {
    (amount_ui * 10f64.powi(TOKEN_DECIMALS as i32)).round() as u64
}

pub fn base_units_to_ui(amount: u64) -> f64 {
    amount as f64 / 10f64.powi(TOKEN_DECIMALS as i32)
}
