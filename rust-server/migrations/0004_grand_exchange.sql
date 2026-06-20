-- Grand Exchange: global SOLST-priced order book.
-- Prices and settlement use accounts.chain_balance (SOLST base units).
-- Items are escrowed off the seller's character inventory at placement and
-- delivered into a buyer's collect box on match.

CREATE TABLE IF NOT EXISTS grand_exchange_offers (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id    INTEGER NOT NULL,
    character_id  INTEGER NOT NULL,
    side          TEXT NOT NULL CHECK (side IN ('buy', 'sell')),
    item_id       TEXT NOT NULL,
    price         INTEGER NOT NULL,            -- SOLST base units per unit
    quantity      INTEGER NOT NULL,            -- original order size
    remaining     INTEGER NOT NULL,            -- units still resting on the book
    collect_items INTEGER NOT NULL DEFAULT 0,  -- units waiting in the owner's collect box
    status        TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'done')),
    created_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE
);

-- Matching scans by item + side + price, so index those.
CREATE INDEX IF NOT EXISTS idx_ge_match
    ON grand_exchange_offers(item_id, side, status, price);

CREATE INDEX IF NOT EXISTS idx_ge_account
    ON grand_exchange_offers(account_id);
