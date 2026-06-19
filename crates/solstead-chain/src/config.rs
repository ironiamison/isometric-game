use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use std::str::FromStr;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ChainConfig {
    pub rpc_url: String,
    pub program_id: Pubkey,
    pub mint: Pubkey,
    pub authority_keypair: Arc<Keypair>,
    pub enabled: bool,
}

impl ChainConfig {
    pub fn from_env() -> Result<Option<Self>, String> {
        let enabled = std::env::var("SOLSTEAD_CHAIN_ENABLED")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        if !enabled {
            return Ok(None);
        }

        let rpc_url = std::env::var("SOLSTEAD_SOLANA_RPC_URL")
            .unwrap_or_else(|_| "https://api.devnet.solana.com".to_string());

        let program_id = Pubkey::from_str(
            &std::env::var("SOLSTEAD_PROGRAM_ID")
                .map_err(|_| "SOLSTEAD_PROGRAM_ID is required when chain is enabled".to_string())?,
        )
        .map_err(|e| format!("invalid SOLSTEAD_PROGRAM_ID: {e}"))?;

        let mint = Pubkey::from_str(
            &std::env::var("SOLSTEAD_MINT_ADDRESS")
                .map_err(|_| "SOLSTEAD_MINT_ADDRESS is required when chain is enabled".to_string())?,
        )
        .map_err(|e| format!("invalid SOLSTEAD_MINT_ADDRESS: {e}"))?;

        let secret = std::env::var("SOLSTEAD_CHAIN_AUTHORITY_SECRET").map_err(|_| {
            "SOLSTEAD_CHAIN_AUTHORITY_SECRET is required when chain is enabled".to_string()
        })?;
        let authority_keypair = Arc::new(decode_keypair(&secret)?);

        Ok(Some(Self {
            rpc_url,
            program_id,
            mint,
            authority_keypair,
            enabled,
        }))
    }

    pub fn vault_token_account(&self) -> Pubkey {
        crate::pda::derive_vault_token_account(&self.program_id, &self.mint)
    }

    pub fn vault_pda(&self) -> (Pubkey, u8) {
        crate::pda::derive_vault(&self.program_id, &self.mint)
    }

    pub fn authority_pubkey(&self) -> Pubkey {
        self.authority_keypair.pubkey()
    }
}

fn decode_keypair(secret: &str) -> Result<Keypair, String> {
    let trimmed = secret.trim();
    let bytes: Vec<u8> = if trimmed.starts_with('[') {
        serde_json::from_str(trimmed)
            .map_err(|e| format!("invalid SOLSTEAD_CHAIN_AUTHORITY_SECRET JSON: {e}"))?
    } else {
        bs58::decode(trimmed)
            .into_vec()
            .map_err(|e| format!("invalid SOLSTEAD_CHAIN_AUTHORITY_SECRET base58: {e}"))?
    };
    Keypair::try_from(bytes.as_slice())
        .map_err(|e| format!("invalid authority keypair bytes: {e}"))
}
