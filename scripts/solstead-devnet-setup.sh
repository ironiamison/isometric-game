#!/usr/bin/env bash
# Deploy Solstead escrow + devnet SPL mint, write rust-server/.env.chain
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

echo "==> Solana devnet"
solana config set --url devnet
solana airdrop 2 || true

KEYPAIR=$(solana config get | awk '/Keypair Path/ {print $3}')
AUTHORITY_PUBKEY="$(solana-keygen pubkey "$KEYPAIR")"

echo "==> Build Anchor program"
anchor build

PROGRAM_ID="$(anchor keys list | awk '/solstead_escrow/ {print $2}')"
echo "Program ID: $PROGRAM_ID"

echo "==> Deploy program"
solana program deploy --url devnet --keypair "$KEYPAIR" \
  --program-id target/deploy/solstead_escrow-keypair.json \
  target/deploy/solstead_escrow.so

echo "==> Create SOLST mint (6 decimals)"
MINT_OUTPUT="$(spl-token create-token --decimals 6 --url devnet --fee-payer "$KEYPAIR" 2>&1)"
MINT="$(echo "$MINT_OUTPUT" | awk '/Creating token/ {print $3}')"
if [[ -z "$MINT" ]]; then
  MINT="$(echo "$MINT_OUTPUT" | rg -o '[1-9A-HJ-NP-Za-km-z]{32,44}' | head -1)"
fi
echo "Mint: $MINT"

spl-token create-account "$MINT" --url devnet --fee-payer "$KEYPAIR" >/dev/null
spl-token mint "$MINT" 1000000 "$AUTHORITY_PUBKEY" --url devnet --fee-payer "$KEYPAIR" >/dev/null

echo "==> Initialize vault"
npm install --prefix "$ROOT/scripts" --silent
INIT_OUT="$(node "$ROOT/scripts/chain-devnet-init.mjs" "$PROGRAM_ID" "$MINT" "$KEYPAIR")"
echo "$INIT_OUT"
VAULT_TOKEN="$(echo "$INIT_OUT" | awk '/Vault token account:/ {print $4}')"

SECRET_JSON="$(cat "$KEYPAIR" | tr -d '\n')"

cat > "$ROOT/rust-server/.env.chain" <<EOF
# Source before starting the game server:
#   set -a && source rust-server/.env.chain && set +a
SOLSTEAD_CHAIN_ENABLED=1
SOLSTEAD_SOLANA_RPC_URL=https://api.devnet.solana.com
SOLSTEAD_PROGRAM_ID=$PROGRAM_ID
SOLSTEAD_MINT_ADDRESS=$MINT
SOLSTEAD_CHAIN_AUTHORITY_SECRET=$SECRET_JSON
EOF

cat > "$ROOT/programs/devnet-config.json" <<EOF
{
  "cluster": "devnet",
  "programId": "$PROGRAM_ID",
  "mint": "$MINT",
  "authority": "$AUTHORITY_PUBKEY",
  "vaultTokenAccount": "$VAULT_TOKEN",
  "tokenSymbol": "SOLST",
  "tokenDecimals": 6
}
EOF

echo ""
echo "Done. Devnet config written to:"
echo "  programs/devnet-config.json"
echo "  rust-server/.env.chain"
echo ""
echo "Start server with chain enabled:"
echo "  cd rust-server && set -a && source .env.chain && set +a && cargo run --locked"
