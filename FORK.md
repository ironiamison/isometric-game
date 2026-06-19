# Fork: Solstead

**Working title:** **Solstead**  
**Base:** [Aeven](https://github.com/andrewrexo/isometric-game) (authoritative isometric MMO)  
**Goal:** Player-driven economy with Solana SPL token settlement — game fun first, chain as ownership/settlement layer.

---

## Why this name

| Criteria | Solstead |
|----------|----------|
| Sounds like a game world | Yes — “your stead” = homestead, town, territory |
| Solana without screaming crypto | “Sol” is subtle; reads as fantasy place name |
| Economy connotation | Stead = land, trade, property, persistence |
| Brandable | Short, memorable, `.gg` / `.io` friendly |

**Alternates if you prefer:**

- **Protia** — ties to `protiable`; more fantasy, less chain-visible
- **Ledgerfall** — economy + fantasy; slightly more abstract

Recommendation: **Solstead** for a public game + SPL economy product.

---

## Local setup (done)

```bash
# Terminal 1 — authoritative server
cd rust-server && AEVEN_ENV=development cargo run --locked

# Terminal 2 — desktop client (needs: brew install lld)
cd client && cargo run --locked --bin new-aeven
```

- Server: `http://localhost:2567` (HTTP + WebSocket)
- Health: `curl http://localhost:2567/health`
- Debug client auto-connects to local server
- Register → create character → open second client to test multiplayer

See [SETUP.md](./SETUP.md) for browser/WASM and production paths.

---

## On-chain economy design (Solana SPL)

Aeven already has the **off-chain game economy** you need as a foundation:

- Gold, inventory, equipment, banks, chests
- Player-to-player **trade**
- **Shops**, stalls, crafting orders
- Server-authoritative (anti-cheat, single source of truth)

**Do not** put combat ticks or movement on-chain. Chain handles **value transfer and ownership**, server handles **gameplay**.

### Architecture (hybrid — recommended)

```
┌─────────────┐     sign tx      ┌──────────────────┐
│   Player    │ ───────────────► │ Solana program   │
│  (Phantom)  │                  │ (Anchor escrow)  │
└──────┬──────┘                  └────────┬─────────┘
       │                                  │
       │ wallet connect                   │ deposit / withdraw events
       ▼                                  ▼
┌─────────────┐     verify + credit  ┌──────────────────┐
│ Site/client │ ◄──────────────────► │ Chain indexer    │
│  UI layer   │                      │ (Rust sidecar)   │
└──────┬──────┘                      └────────┬─────────┘
       │                                      │
       │ WebSocket / REST                     │ confirmed balances
       ▼                                      ▼
┌─────────────────────────────────────────────────────────┐
│ Aeven rust-server (authoritative game + economy)      │
│  • in-game gold sinks/faucets                           │
│  • Grand Exchange / market (existing patterns)          │
│  • `chain_balance` column per account (SPL credits)     │
└─────────────────────────────────────────────────────────┘
```

### Phase 1 — Wallet + deposit/withdraw (MVP)

1. **Connect wallet** on site (`/play/` or launcher) via Phantom / Wallet Standard
2. **Deposit SPL** → program escrow PDA; indexer sees tx → server credits `chain_balance`
3. **Withdraw** → server signs intent → player claims from escrow (rate limits, cooldowns)
4. In-game UI shows both **gold** (sink currency) and **token balance** (tradeable)

### Phase 2 — Market on-chain

- Listings: seller locks item + optional SPL price in escrow OR server-held listing with on-chain settlement at sale
- **Fee sink:** 2–5% of SPL trades → treasury/burn (configurable)
- Price discovery via in-game GE UI; settlement hash stored for audit

### Phase 3 — Ownership (optional)

- Rare/cosmetic items as **Metaplex NFTs** (optional — not required for economy MVP)
- Bulk resources stay off-chain; chain for high-value transfers only

### Economy rules (self-sustaining)

| Mechanism | Off-chain (game) | On-chain (SPL) |
|-----------|------------------|----------------|
| Daily play rewards | Small gold, materials | None or capped drip |
| Skilling / bosses | Items, gold sinks | Withdraw earned balance only |
| Player market | GE order book UI | SPL settlement + fee |
| New player tax | — | Deposit to participate in P2P market |
| Sinks | Repair, travel, GE tax, death | Withdraw fee, listing fee |

**Principle:** Faucets in-game; **withdrawals** are the bridge to SPL. Players earn in-world, cash out through sinks and fees — not token emissions to new wallets.

---

## Implementation map (Aeven touchpoints)

| Area | Path | Chain work |
|------|------|------------|
| Accounts | `rust-server` auth / SQLite | Link `wallet_pubkey` to account |
| Trade | `rust-server/src/game/trade.rs` | Optional SPL component on confirm |
| Shops / stalls | `game/shop.rs`, `game/stall.rs` | SPL-priced listings |
| Site login | `site/` SvelteKit | Wallet connect + link flow |
| Client | `client/` | Deposit/withdraw UI panel |
| New crate | `crates/solstead-chain/` (planned) | Tx verify, escrow client |

---

## Rebrand checklist (when ready)

- [x] Rename display strings: `New Aeven` → **Solstead** (site + login UI)
- [ ] `AEVEN_*` env vars → `SOLSTEAD_*` (or keep internal, rebrand UI only)
- [x] Site meta, play shell title
- [ ] Deploy own domain (see [DEPLOY-SOLSTEAD.md](./DEPLOY-SOLSTEAD.md))
- [ ] SPL mint + Anchor program deploy (devnet → mainnet) — **devnet live:** see [CHAIN-DEVNET.md](./CHAIN-DEVNET.md)

---

## Next build steps

1. Confirm name (**Solstead** vs **Protia**)
2. Rebrand UI strings + site (minimal diff)
3. Add `wallet_pubkey` migration + link API on server
4. Scaffold Anchor escrow program + Rust indexer sidecar
5. Wire deposit/withdraw UI in site `/play/`
