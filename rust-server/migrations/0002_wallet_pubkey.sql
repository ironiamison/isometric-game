ALTER TABLE accounts ADD COLUMN wallet_pubkey TEXT;
CREATE UNIQUE INDEX IF NOT EXISTS idx_accounts_wallet_pubkey ON accounts(wallet_pubkey) WHERE wallet_pubkey IS NOT NULL;
