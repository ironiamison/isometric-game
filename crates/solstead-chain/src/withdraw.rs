use sha2::{Digest, Sha256};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_program,
    sysvar,
    transaction::Transaction,
};
use spl_associated_token_account::get_associated_token_address;
use std::str::FromStr;

use crate::pda::derive_vault;

pub const WITHDRAW_PREFIX: &str = "solstead:withdraw:";

#[derive(Debug, Clone)]
pub struct WithdrawTicket {
    pub recipient: Pubkey,
    pub amount: u64,
    pub expires_at: i64,
    pub message: String,
    pub signature: [u8; 64],
}

pub fn withdraw_message(recipient: &Pubkey, amount: u64, expires_at: i64) -> String {
    format!("solstead:withdraw:{recipient}:{amount}:{expires_at}")
}

pub fn sign_withdraw_ticket(
    authority: &Keypair,
    recipient: &Pubkey,
    amount: u64,
    expires_at: i64,
) -> WithdrawTicket {
    let message = withdraw_message(recipient, amount, expires_at);
    let signature = authority.sign_message(message.as_bytes());
    WithdrawTicket {
        recipient: *recipient,
        amount,
        expires_at,
        message,
        signature: signature.into(),
    }
}

fn anchor_discriminator(name: &str) -> [u8; 8] {
    let preimage = format!("global:{name}");
    let hash = Sha256::digest(preimage.as_bytes());
    hash[..8].try_into().expect("discriminator is 8 bytes")
}

pub fn build_ed25519_verify_instruction(
    authority: &Pubkey,
    message: &str,
    signature: &[u8; 64],
) -> Instruction {
    let mut data = Vec::with_capacity(112 + message.len());
    data.extend_from_slice(&16u16.to_le_bytes());
    data.extend_from_slice(&u16::MAX.to_le_bytes());
    data.extend_from_slice(&80u16.to_le_bytes());
    data.extend_from_slice(&u16::MAX.to_le_bytes());
    data.extend_from_slice(&112u16.to_le_bytes());
    data.extend_from_slice(&(message.len() as u16).to_le_bytes());
    data.extend_from_slice(&u16::MAX.to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes());
    data.extend_from_slice(signature);
    data.extend_from_slice(&authority.to_bytes());
    data.extend_from_slice(message.as_bytes());

    Instruction {
        program_id: Pubkey::from_str("Ed25519SigVerify111111111111111111111111111").unwrap(),
        accounts: vec![],
        data,
    }
}

pub fn build_withdraw_transaction(
    config: &crate::config::ChainConfig,
    ticket: &WithdrawTicket,
    payer: &Keypair,
    blockhash: solana_sdk::hash::Hash,
) -> Transaction {
    let program_id = config.program_id;
    let mint = config.mint;
    let (vault, _vault_bump) = derive_vault(&program_id, &mint);
    let vault_token_account =
        spl_associated_token_account::get_associated_token_address(&vault, &mint);
    let recipient_token_account = get_associated_token_address(&ticket.recipient, &mint);

    let ed25519_ix = build_ed25519_verify_instruction(
        &config.authority_pubkey(),
        &ticket.message,
        &ticket.signature,
    );

    let mut withdraw_data = Vec::with_capacity(26);
    withdraw_data.extend_from_slice(&anchor_discriminator("withdraw"));
    withdraw_data.extend_from_slice(&ticket.amount.to_le_bytes());
    withdraw_data.extend_from_slice(&ticket.expires_at.to_le_bytes());
    withdraw_data.extend_from_slice(&0u16.to_le_bytes());

    let withdraw_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new_readonly(ticket.recipient, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new_readonly(vault, false),
            AccountMeta::new(vault_token_account, false),
            AccountMeta::new(recipient_token_account, false),
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(sysvar::instructions::ID, false),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(spl_associated_token_account::ID, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: withdraw_data,
    };

    Transaction::new_signed_with_payer(
        &[ed25519_ix, withdraw_ix],
        Some(&payer.pubkey()),
        &[payer],
        blockhash,
    )
}
