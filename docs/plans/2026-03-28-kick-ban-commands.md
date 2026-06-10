# Kick & Ban Commands Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add `/kick` and `/ban` admin commands with account-level + IP-level bans.

**Architecture:** New `bans` DB table, ban checks on login + matchmaking, kick via dropping the player's mpsc sender (triggers existing disconnect cleanup). Player struct gains `account_id` and `ip_address` fields.

**Tech Stack:** Rust, SQLite (sqlx), Tokio mpsc channels, chrono for timestamps.

---

### Task 1: Add `bans` table migration

**Files:**
- Modify: `rust-server/src/db.rs` (in `migrate()` function, after existing table creations)

**Step 1: Add bans table creation to migrate()**

Add after the last `CREATE TABLE` block in `migrate()`:

```rust
// Create bans table for moderation
sqlx::query(
    r#"
    CREATE TABLE IF NOT EXISTS bans (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        account_id INTEGER NOT NULL,
        ip_address TEXT,
        banned_by TEXT NOT NULL,
        reason TEXT,
        banned_at TEXT NOT NULL,
        expires_at TEXT NOT NULL,
        FOREIGN KEY (account_id) REFERENCES accounts(id)
    )
    "#,
)
.execute(pool)
.await?;
```

**Step 2: Add DB query methods to `impl Database`**

Add these methods:

```rust
/// Check if an account has an active ban. Returns (reason, expires_at) if banned.
pub async fn check_ban_by_account(&self, account_id: i64) -> Option<(Option<String>, String)> {
    sqlx::query_as::<_, (Option<String>, String)>(
        "SELECT reason, expires_at FROM bans WHERE account_id = ? AND expires_at > datetime('now') ORDER BY expires_at DESC LIMIT 1"
    )
    .bind(account_id)
    .fetch_optional(&self.pool)
    .await
    .ok()
    .flatten()
}

/// Check if an IP has an active ban. Returns (reason, expires_at) if banned.
pub async fn check_ban_by_ip(&self, ip: &str) -> Option<(Option<String>, String)> {
    sqlx::query_as::<_, (Option<String>, String)>(
        "SELECT reason, expires_at FROM bans WHERE ip_address = ? AND expires_at > datetime('now') ORDER BY expires_at DESC LIMIT 1"
    )
    .bind(ip)
    .fetch_optional(&self.pool)
    .await
    .ok()
    .flatten()
}

/// Insert a new ban record.
pub async fn insert_ban(
    &self,
    account_id: i64,
    ip_address: Option<&str>,
    banned_by: &str,
    reason: Option<&str>,
    hours: f64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO bans (account_id, ip_address, banned_by, reason, banned_at, expires_at) VALUES (?, ?, ?, ?, datetime('now'), datetime('now', '+' || ? || ' hours'))"
    )
    .bind(account_id)
    .bind(ip_address)
    .bind(banned_by)
    .bind(reason)
    .bind(hours)
    .execute(&self.pool)
    .await?;
    Ok(())
}

/// Look up account_id by character name (for offline bans).
pub async fn get_account_id_by_character_name(&self, name: &str) -> Option<i64> {
    sqlx::query_scalar::<_, i64>(
        "SELECT account_id FROM characters WHERE name = ? COLLATE NOCASE LIMIT 1"
    )
    .bind(name)
    .fetch_optional(&self.pool)
    .await
    .ok()
    .flatten()
}
```

**Step 3: Verify it compiles**

Run: `cd rust-server && cargo check 2>&1 | head -20`

**Step 4: Commit**

```
feat: add bans table and DB query methods
```

---

### Task 2: Add `account_id` and `ip_address` to Player struct

**Files:**
- Modify: `rust-server/src/game.rs` — Player struct + `reserve_player_with_data`
- Modify: `rust-server/src/main.rs` — pass account_id and IP when creating player

**Step 1: Add fields to Player struct**

In `game.rs`, add to the Player struct (after `is_god_mode`):

```rust
pub account_id: i64,
pub ip_address: Option<String>,
```

**Step 2: Update `reserve_player_with_data` to accept and store new fields**

Add `account_id: i64` and `ip_address: Option<String>` parameters. Set them on the Player when constructing it.

**Step 3: Update the call site in `matchmake_join_or_create`**

In `main.rs`, pass `account_id` and `client_ip` (already available as `addr.ip().to_string()`) to `reserve_player_with_data`.

**Step 4: Verify it compiles**

Run: `cd rust-server && cargo check 2>&1 | head -20`

**Step 5: Commit**

```
feat: add account_id and ip_address to Player struct
```

---

### Task 3: Add `/kick` command

**Files:**
- Modify: `rust-server/src/game/chat.rs`

**Step 1: Add "kick" to ADMIN_COMMANDS list**

```rust
const ADMIN_COMMANDS: &[&str] = &[
    "/give", "/setlevel", "/teleport", "/tpto", "/spawn",
    "/heal", "/kill", "/god", "/announce", "/arena", "/boss",
    "/kick", "/ban",
];
```

**Step 2: Add `/kick` match arm**

Add before the `_ =>` default arm:

```rust
"/kick" => {
    if parts.len() < 2 {
        self.send_system_message(player_id, "Usage: /kick <player_name>")
            .await;
        return;
    }
    let target_name = parts[1];

    let target_id = {
        let players = self.players.read().await;
        players
            .values()
            .find(|p| p.name.eq_ignore_ascii_case(target_name))
            .map(|p| p.id.clone())
    };

    match target_id {
        Some(tid) => {
            let admin_name = {
                let players = self.players.read().await;
                players.get(player_id).map(|p| p.name.clone()).unwrap_or_default()
            };
            tracing::info!("Admin {} kicked player {}", admin_name, target_name);
            self.send_system_message(&tid, "You have been kicked by an admin.")
                .await;
            // Drop the player's sender — closes their mpsc channel,
            // which causes the send task to exit and triggers normal disconnect cleanup.
            self.unregister_player_sender(&tid).await;
            self.send_system_message(player_id, &format!("Kicked {}", target_name))
                .await;
        }
        None => {
            self.send_system_message(player_id, "Player not found or not online.")
                .await;
        }
    }
}
```

**Step 3: Update `/help` output to include new commands**

Update the admin help string to include `/kick` and `/ban`.

**Step 4: Verify it compiles**

Run: `cd rust-server && cargo check 2>&1 | head -20`

**Step 5: Commit**

```
feat: add /kick admin command
```

---

### Task 4: Add `/ban` command

**Files:**
- Modify: `rust-server/src/game/chat.rs`

**Step 1: Add `/ban` match arm**

Add after the `/kick` arm:

```rust
"/ban" => {
    if parts.len() < 3 {
        self.send_system_message(player_id, "Usage: /ban <player_name> <hours> [reason]")
            .await;
        return;
    }
    let target_name = parts[1];
    let hours: f64 = match parts[2].parse() {
        Ok(h) if h > 0.0 => h,
        _ => {
            self.send_system_message(player_id, "Hours must be a positive number.")
                .await;
            return;
        }
    };
    let reason = if parts.len() > 3 {
        Some(parts[3..].join(" "))
    } else {
        None
    };

    let admin_name = {
        let players = self.players.read().await;
        players.get(player_id).map(|p| p.name.clone()).unwrap_or_default()
    };

    // Check if player is online — get account_id and IP
    let online_info = {
        let players = self.players.read().await;
        players
            .values()
            .find(|p| p.name.eq_ignore_ascii_case(target_name))
            .map(|p| (p.id.clone(), p.account_id, p.ip_address.clone()))
    };

    let db = match &self.db {
        Some(db) => db.clone(),
        None => {
            self.send_system_message(player_id, "Database not available.")
                .await;
            return;
        }
    };

    if let Some((tid, account_id, ip)) = online_info {
        // Online player — ban and kick
        if let Err(e) = db
            .insert_ban(account_id, ip.as_deref(), &admin_name, reason.as_deref(), hours)
            .await
        {
            self.send_system_message(player_id, &format!("Failed to ban: {}", e))
                .await;
            return;
        }

        let ban_msg = match &reason {
            Some(r) => format!("You have been banned for {} hours. Reason: {}", hours, r),
            None => format!("You have been banned for {} hours.", hours),
        };
        self.send_system_message(&tid, &ban_msg).await;
        self.unregister_player_sender(&tid).await;

        tracing::info!("Admin {} banned {} for {} hours (reason: {:?})", admin_name, target_name, hours, reason);
        self.send_system_message(player_id, &format!("Banned {} for {} hours", target_name, hours))
            .await;
    } else {
        // Offline player — look up from DB
        match db.get_account_id_by_character_name(target_name).await {
            Some(account_id) => {
                if let Err(e) = db
                    .insert_ban(account_id, None, &admin_name, reason.as_deref(), hours)
                    .await
                {
                    self.send_system_message(player_id, &format!("Failed to ban: {}", e))
                        .await;
                    return;
                }
                tracing::info!("Admin {} banned offline player {} for {} hours (reason: {:?})", admin_name, target_name, hours, reason);
                self.send_system_message(
                    player_id,
                    &format!("Banned {} (offline) for {} hours", target_name, hours),
                )
                .await;
            }
            None => {
                self.send_system_message(player_id, "Character not found.")
                    .await;
            }
        }
    }
}
```

**Step 2: Verify it compiles**

Run: `cd rust-server && cargo check 2>&1 | head -20`

**Step 3: Commit**

```
feat: add /ban admin command with account + IP bans
```

---

### Task 5: Add ban checks to login and matchmaking

**Files:**
- Modify: `rust-server/src/main.rs` — `login_account` and `matchmake_join_or_create`

**Step 1: Add ban check in `login_account` (after successful credential validation)**

After `Some(account) => {` and before creating the auth token, add:

```rust
// Check for active ban on this account
if let Some((reason, expires_at)) = state.db.check_ban_by_account(account.id).await {
    let msg = match reason {
        Some(r) => format!("Account banned until {}. Reason: {}", expires_at, r),
        None => format!("Account banned until {}.", expires_at),
    };
    return Json(AuthResponse {
        success: false,
        token: None,
        username: None,
        characters: None,
        error: Some(msg),
    });
}
// Check for active ban on this IP
if let Some((reason, expires_at)) = state.db.check_ban_by_ip(&client_ip).await {
    let msg = match reason {
        Some(r) => format!("Connection banned until {}. Reason: {}", expires_at, r),
        None => format!("Connection banned until {}.", expires_at),
    };
    return Json(AuthResponse {
        success: false,
        token: None,
        username: None,
        characters: None,
        error: Some(msg),
    });
}
```

**Step 2: Add ban check in `matchmake_join_or_create` (early, before creating the session)**

After extracting `account_id` from auth, add account + IP ban checks. Return an error response if banned.

**Step 3: Verify it compiles**

Run: `cd rust-server && cargo check 2>&1 | head -20`

**Step 4: Commit**

```
feat: add ban checks to login and matchmaking endpoints
```

---

### Task 6: Final verification

**Step 1: Full compile check**

Run: `cd rust-server && cargo build 2>&1 | tail -5`

**Step 2: Commit any remaining changes**

```
chore: kick and ban system complete
```
