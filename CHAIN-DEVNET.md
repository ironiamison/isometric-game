# Solstead chain economy — devnet

Hybrid SPL economy: gameplay stays on the Rust server; **deposits and withdrawals** settle on Solana devnet.

## Architecture

| Layer | Role |
|-------|------|
| `programs/solstead-escrow` | Anchor vault — SPL deposit + authority-signed withdraw |
| `crates/solstead-chain` | RPC indexer, withdraw tx builder |
| `rust-server` | `chain_balance` in SQLite, REST API, background indexer |
| `/play/` | Phantom deposit UI; server-relayed withdraw |

## One-time devnet setup

Requires: Solana CLI, Anchor 0.31, Node 20+, `spl-token`.

```bash
cd aeven
npm install --prefix scripts @solana/web3.js@1 @solana/spl-token@0.4
chmod +x scripts/solstead-devnet-setup.sh
./scripts/solstead-devnet-setup.sh
```

This will:

1. Deploy `solstead-escrow` to devnet
2. Create a **SOLST** SPL mint (6 decimals)
3. Mint 1,000,000 test tokens to your default wallet
4. Initialize the vault PDA
5. Write `rust-server/.env.chain` and `programs/devnet-config.json`

## Run locally with chain enabled

```bash
cd rust-server
set -a && source .env.chain && set +a
AEVEN_ENV=development cargo run --locked
```

In another terminal, serve the site or open `/play/` against the game server.

## Player flow

1. **Connect wallet** on the title screen (links `wallet_pubkey` to account)
2. **Deposit** — sign SPL transfer into vault via Phantom; indexer credits `chain_balance` within ~30s
3. **Withdraw** — server debits balance and submits on-chain withdraw (tokens arrive in Phantom)

Guest accounts cannot deposit/withdraw (no linked wallet).

## API

| Endpoint | Auth | Description |
|----------|------|-------------|
| `GET /api/chain/config` | — | Program id, mint, vault, decimals |
| `GET /api/chain/balance` | Bearer | UI + base-unit balance |
| `GET /api/chain/history` | Bearer | Recent deposits/withdrawals |
| `POST /api/chain/withdraw` | Bearer | `{ "amount": 1.0 }` |

## Environment variables

| Variable | Required | Description |
|----------|----------|-------------|
| `SOLSTEAD_CHAIN_ENABLED` | yes | `1` to enable |
| `SOLSTEAD_SOLANA_RPC_URL` | yes | Default `https://api.devnet.solana.com` |
| `SOLSTEAD_PROGRAM_ID` | yes | Deployed program |
| `SOLSTEAD_MINT_ADDRESS` | yes | SOLST mint |
| `SOLSTEAD_CHAIN_AUTHORITY_SECRET` | yes | JSON byte array or base58 keypair (must match vault authority) |

## Faucet test tokens

After setup, your deploy wallet holds minted SOLST. Send to players via:

```bash
spl-token transfer <MINT> 100 <PLAYER_WALLET> --fund-recipient
```

## Next steps (Phase 2)

- SPL-priced player stalls / Grand Exchange
- On-chain market settlement with fee sink
- Mainnet deploy + production mint
