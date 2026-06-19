use solana_client::rpc_client::RpcClient;
use solana_client::rpc_config::RpcTransactionConfig;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signature;
use solana_transaction_status::{
    EncodedConfirmedTransactionWithStatusMeta, UiTransactionEncoding,
    option_serializer::OptionSerializer,
};
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepositObservation {
    pub signature: String,
    pub depositor_wallet: String,
    pub amount: u64,
}

pub fn scan_vault_deposits(
    rpc_url: &str,
    vault_token_account: &Pubkey,
    before: Option<&str>,
    limit: usize,
) -> Result<Vec<DepositObservation>, String> {
    let client = RpcClient::new(rpc_url.to_string());
    let before_sig = before.and_then(|s| Signature::from_str(s).ok());

    let signatures = client
        .get_signatures_for_address_with_config(
            vault_token_account,
            solana_client::rpc_client::GetConfirmedSignaturesForAddress2Config {
                before: before_sig,
                limit: Some(limit),
                ..Default::default()
            },
        )
        .map_err(|e| format!("get_signatures_for_address failed: {e}"))?;

    let mut deposits = Vec::new();
    for status in signatures {
        if status.err.is_some() {
            continue;
        }
        let sig = status.signature;
        let Some(tx) = fetch_transaction(&client, &sig)? else {
            continue;
        };
        if let Some(mut observation) = parse_deposit(&tx, vault_token_account) {
            observation.signature = sig;
            deposits.push(observation);
        }
    }

    Ok(deposits)
}

fn fetch_transaction(
    client: &RpcClient,
    signature: &str,
) -> Result<Option<EncodedConfirmedTransactionWithStatusMeta>, String> {
    let sig = Signature::from_str(signature)
        .map_err(|e| format!("invalid signature {signature}: {e}"))?;
    client
        .get_transaction_with_config(
            &sig,
            RpcTransactionConfig {
                encoding: Some(UiTransactionEncoding::JsonParsed),
                max_supported_transaction_version: Some(0),
                ..Default::default()
            },
        )
        .map(|tx| Some(tx))
        .map_err(|e| format!("get_transaction {signature} failed: {e}"))
}

fn optional_balances(
    value: &OptionSerializer<Vec<solana_transaction_status::UiTransactionTokenBalance>>,
) -> Option<&Vec<solana_transaction_status::UiTransactionTokenBalance>> {
    match value {
        OptionSerializer::Some(balances) => Some(balances),
        OptionSerializer::None | OptionSerializer::Skip => None,
    }
}

fn parse_deposit(
    tx: &EncodedConfirmedTransactionWithStatusMeta,
    vault_token_account: &Pubkey,
) -> Option<DepositObservation> {
    let meta = tx.transaction.meta.as_ref()?;
    let pre = optional_balances(&meta.pre_token_balances)?;
    let post = optional_balances(&meta.post_token_balances)?;

    let vault_str = vault_token_account.to_string();

    let post_vault = post.iter().find(|b| account_index_matches_vault(b, &vault_str, tx))?;

    let pre_amount = pre
        .iter()
        .find(|b| b.account_index == post_vault.account_index)
        .and_then(|b| b.ui_token_amount.amount.parse::<u64>().ok())
        .unwrap_or(0);
    let post_amount = post_vault
        .ui_token_amount
        .amount
        .parse::<u64>()
        .ok()?;
    if post_amount <= pre_amount {
        return None;
    }
    let amount = post_amount - pre_amount;

    let depositor_wallet = find_depositor_wallet(pre, post, post_vault.account_index)?;

    Some(DepositObservation {
        depositor_wallet,
        amount,
        signature: String::new(),
    })
}

fn account_index_matches_vault(
    balance: &solana_transaction_status::UiTransactionTokenBalance,
    vault_str: &str,
    tx: &EncodedConfirmedTransactionWithStatusMeta,
) -> bool {
    let Some(decoded) = tx.transaction.transaction.decode() else {
        return false;
    };
    let account_keys = decoded.message.static_account_keys();
    account_keys
        .get(balance.account_index as usize)
        .is_some_and(|key| key.to_string() == *vault_str)
}

fn find_depositor_wallet(
    pre: &[solana_transaction_status::UiTransactionTokenBalance],
    post: &[solana_transaction_status::UiTransactionTokenBalance],
    vault_index: u8,
) -> Option<String> {
    for post_balance in post {
        if post_balance.account_index == vault_index {
            continue;
        }
        let pre_amount = pre
            .iter()
            .find(|b| b.account_index == post_balance.account_index)
            .and_then(|b| b.ui_token_amount.amount.parse::<u64>().ok())
            .unwrap_or(0);
        let post_amount = post_balance
            .ui_token_amount
            .amount
            .parse::<u64>()
            .ok()?;
        if pre_amount > post_amount {
            return match &post_balance.owner {
                OptionSerializer::Some(owner) => Some(owner.clone()),
                OptionSerializer::None | OptionSerializer::Skip => None,
            };
        }
    }
    None
}
