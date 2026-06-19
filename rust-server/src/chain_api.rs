use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use solstead_chain::{
    base_units_to_ui, sign_withdraw_ticket, ui_to_base_units, ChainConfig, TOKEN_DECIMALS,
};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::characters::extract_auth;
use crate::AppState;

#[derive(Serialize)]
struct ChainConfigResponse {
    enabled: bool,
    cluster: String,
    program_id: Option<String>,
    mint: Option<String>,
    vault_token_account: Option<String>,
    token_symbol: String,
    token_decimals: u8,
}

#[derive(Serialize)]
struct ChainBalanceResponse {
    success: bool,
    balance: Option<f64>,
    balance_base_units: Option<i64>,
    wallet_pubkey: Option<String>,
    error: Option<String>,
}

#[derive(Serialize)]
struct ChainHistoryResponse {
    success: bool,
    transactions: Option<Vec<ChainTxDto>>,
    error: Option<String>,
}

#[derive(Serialize)]
struct ChainTxDto {
    tx_signature: String,
    direction: String,
    amount: f64,
    status: String,
    created_at: String,
}

#[derive(Serialize)]
struct WithdrawResponse {
    success: bool,
    tx_signature: Option<String>,
    amount: Option<f64>,
    error: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct WithdrawRequest {
    amount: f64,
}

pub(super) async fn chain_config(State(state): State<AppState>) -> impl IntoResponse {
    let Some(chain) = state.chain_config.as_ref() else {
        return Json(ChainConfigResponse {
            enabled: false,
            cluster: "devnet".to_string(),
            program_id: None,
            mint: None,
            vault_token_account: None,
            token_symbol: "SOLST".to_string(),
            token_decimals: TOKEN_DECIMALS,
        });
    };

    Json(ChainConfigResponse {
        enabled: true,
        cluster: "devnet".to_string(),
        program_id: Some(chain.program_id.to_string()),
        mint: Some(chain.mint.to_string()),
        vault_token_account: Some(chain.vault_token_account().to_string()),
        token_symbol: "SOLST".to_string(),
        token_decimals: TOKEN_DECIMALS,
    })
}

pub(super) async fn chain_balance(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let (account_id, _) = match extract_auth(&headers, &state.auth_sessions) {
        Some(auth) => auth,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(ChainBalanceResponse {
                    success: false,
                    balance: None,
                    balance_base_units: None,
                    wallet_pubkey: None,
                    error: Some("Not authenticated".to_string()),
                }),
            );
        }
    };

    let balance = match state.db.get_chain_balance(account_id).await {
        Ok(balance) => balance,
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ChainBalanceResponse {
                    success: false,
                    balance: None,
                    balance_base_units: None,
                    wallet_pubkey: None,
                    error: Some(format!("Database error: {error}")),
                }),
            );
        }
    };

    let wallet_pubkey = state
        .db
        .get_wallet_pubkey_for_account(account_id)
        .await
        .ok()
        .flatten();

    (
        StatusCode::OK,
        Json(ChainBalanceResponse {
            success: true,
            balance: Some(base_units_to_ui(balance as u64)),
            balance_base_units: Some(balance),
            wallet_pubkey,
            error: None,
        }),
    )
}

pub(super) async fn chain_history(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let (account_id, _) = match extract_auth(&headers, &state.auth_sessions) {
        Some(auth) => auth,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(ChainHistoryResponse {
                    success: false,
                    transactions: None,
                    error: Some("Not authenticated".to_string()),
                }),
            );
        }
    };

    match state.db.list_chain_transactions(account_id, 20).await {
        Ok(rows) => {
            let transactions = rows
                .into_iter()
                .map(|row| ChainTxDto {
                    tx_signature: row.tx_signature,
                    direction: row.direction,
                    amount: base_units_to_ui(row.amount as u64),
                    status: row.status,
                    created_at: row.created_at,
                })
                .collect();
            (
                StatusCode::OK,
                Json(ChainHistoryResponse {
                    success: true,
                    transactions: Some(transactions),
                    error: None,
                }),
            )
        }
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ChainHistoryResponse {
                success: false,
                transactions: None,
                error: Some(format!("Database error: {error}")),
            }),
        ),
    }
}

pub(super) async fn chain_withdraw(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<WithdrawRequest>,
) -> impl IntoResponse {
    let chain = match state.chain_config.as_ref() {
        Some(chain) => chain.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(WithdrawResponse {
                    success: false,
                    tx_signature: None,
                    amount: None,
                    error: Some("Chain economy is not enabled on this server".to_string()),
                }),
            );
        }
    };

    let (account_id, _) = match extract_auth(&headers, &state.auth_sessions) {
        Some(auth) => auth,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(WithdrawResponse {
                    success: false,
                    tx_signature: None,
                    amount: None,
                    error: Some("Not authenticated".to_string()),
                }),
            );
        }
    };

    if req.amount <= 0.0 {
        return (
            StatusCode::BAD_REQUEST,
            Json(WithdrawResponse {
                success: false,
                tx_signature: None,
                amount: None,
                error: Some("Amount must be greater than zero".to_string()),
            }),
        );
    }

    let wallet_pubkey = match state.db.get_wallet_pubkey_for_account(account_id).await {
        Ok(Some(pk)) => pk,
        Ok(None) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(WithdrawResponse {
                    success: false,
                    tx_signature: None,
                    amount: None,
                    error: Some("Withdraw requires a wallet-linked account".to_string()),
                }),
            );
        }
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(WithdrawResponse {
                    success: false,
                    tx_signature: None,
                    amount: None,
                    error: Some(format!("Database error: {error}")),
                }),
            );
        }
    };

    let amount_base = ui_to_base_units(req.amount) as i64;
    if amount_base <= 0 {
        return (
            StatusCode::BAD_REQUEST,
            Json(WithdrawResponse {
                success: false,
                tx_signature: None,
                amount: None,
                error: Some("Amount is too small".to_string()),
            }),
        );
    }

    if !state
        .db
        .reserve_chain_withdraw(account_id, amount_base)
        .await
        .unwrap_or(false)
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(WithdrawResponse {
                success: false,
                tx_signature: None,
                amount: None,
                error: Some("Insufficient chain balance".to_string()),
            }),
        );
    }

    let recipient = match Pubkey::from_str(&wallet_pubkey) {
        Ok(pk) => pk,
        Err(error) => {
            let _ = state
                .db
                .finalize_chain_withdraw(account_id, "invalid-recipient", amount_base, false)
                .await;
            return (
                StatusCode::BAD_REQUEST,
                Json(WithdrawResponse {
                    success: false,
                    tx_signature: None,
                    amount: None,
                    error: Some(format!("Invalid wallet pubkey: {error}")),
                }),
            );
        }
    };

    let expires_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64 + 300)
        .unwrap_or(0);
    let ticket = sign_withdraw_ticket(
        &chain.authority_keypair,
        &recipient,
        amount_base as u64,
        expires_at,
    );

    let rpc = RpcClient::new(chain.rpc_url.clone());
    let blockhash = match rpc.get_latest_blockhash() {
        Ok(hash) => hash,
        Err(error) => {
            let _ = state
                .db
                .finalize_chain_withdraw(account_id, "blockhash-error", amount_base, false)
                .await;
            return (
                StatusCode::BAD_GATEWAY,
                Json(WithdrawResponse {
                    success: false,
                    tx_signature: None,
                    amount: None,
                    error: Some(format!("Solana RPC error: {error}")),
                }),
            );
        }
    };

    let tx = solstead_chain::build_withdraw_transaction(
        &chain,
        &ticket,
        &chain.authority_keypair,
        blockhash,
    );

    match rpc.send_and_confirm_transaction(&tx) {
        Ok(signature) => {
            let sig = signature.to_string();
            let _ = state
                .db
                .finalize_chain_withdraw(account_id, &sig, amount_base, true)
                .await;
            (
                StatusCode::OK,
                Json(WithdrawResponse {
                    success: true,
                    tx_signature: Some(sig),
                    amount: Some(req.amount),
                    error: None,
                }),
            )
        }
        Err(error) => {
            let _ = state
                .db
                .finalize_chain_withdraw(account_id, "withdraw-failed", amount_base, false)
                .await;
            (
                StatusCode::BAD_GATEWAY,
                Json(WithdrawResponse {
                    success: false,
                    tx_signature: None,
                    amount: None,
                    error: Some(format!("Withdraw transaction failed: {error}")),
                }),
            )
        }
    }
}

pub(super) fn load_chain_config() -> Option<Arc<ChainConfig>> {
    ChainConfig::from_env()
        .ok()
        .flatten()
        .map(Arc::new)
}

pub(super) async fn run_chain_indexer(state: AppState) {
    let Some(chain) = state.chain_config.clone() else {
        return;
    };

    let vault = chain.vault_token_account();
    tracing::info!(
        "Chain indexer started (devnet) — vault token account {}",
        vault
    );

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(15)).await;

        let deposits = match solstead_chain::scan_vault_deposits(&chain.rpc_url, &vault, None, 25)
        {
            Ok(deposits) => deposits,
            Err(error) => {
                tracing::warn!("Chain indexer scan failed: {error}");
                continue;
            }
        };

        for deposit in deposits {
            if state.db.chain_tx_exists(&deposit.signature).await.unwrap_or(true) {
                continue;
            }

            let Some(account_id) = state
                .db
                .get_account_id_by_wallet(&deposit.depositor_wallet)
                .await
                .ok()
                .flatten()
            else {
                tracing::debug!(
                    "Ignoring deposit {} — wallet {} not linked",
                    deposit.signature,
                    deposit.depositor_wallet
                );
                continue;
            };

            match state
                .db
                .credit_chain_deposit(account_id, &deposit.signature, deposit.amount as i64)
                .await
            {
                Ok(true) => tracing::info!(
                    "Credited deposit {} (+{} base units) to account {}",
                    deposit.signature,
                    deposit.amount,
                    account_id
                ),
                Ok(false) => {}
                Err(error) => tracing::warn!(
                    "Failed to credit deposit {}: {error}",
                    deposit.signature
                ),
            }
        }
    }
}
