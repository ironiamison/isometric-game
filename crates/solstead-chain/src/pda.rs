use solana_sdk::pubkey::Pubkey;
use spl_associated_token_account::get_associated_token_address;

pub fn derive_vault(program_id: &Pubkey, mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"vault", mint.as_ref()], program_id)
}

pub fn derive_vault_token_account(program_id: &Pubkey, mint: &Pubkey) -> Pubkey {
    let (vault, _bump) = derive_vault(program_id, mint);
    get_associated_token_address(&vault, mint)
}
