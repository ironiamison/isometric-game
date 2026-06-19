ALTER TABLE accounts ADD COLUMN chain_balance INTEGER NOT NULL DEFAULT 0;

CREATE TABLE IF NOT EXISTS chain_transactions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL,
    tx_signature TEXT NOT NULL UNIQUE,
    direction TEXT NOT NULL CHECK (direction IN ('deposit', 'withdraw')),
    amount INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'confirmed' CHECK (status IN ('pending', 'confirmed', 'failed')),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_chain_transactions_account_id ON chain_transactions(account_id);
