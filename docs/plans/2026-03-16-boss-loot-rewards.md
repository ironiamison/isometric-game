# Boss Loot Rewards System Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Allow players to claim boss loot from the Battle Master NPC after defeating the Desert Wurm, with independent loot rolls per participant, DB persistence, and chat announcements.

**Architecture:** Track damage dealers during the boss fight via a `HashSet` on `BossState`. On boss death, independently roll the loot table for each participant, persist results to a `boss_pending_rewards` DB table, and announce rolls in chat. The Battle Master NPC checks for pending rewards on interaction and lets players claim them via dialogue.

**Tech Stack:** Rust (Axum/Tokio), SQLite (sqlx), MessagePack protocol, TOML data files.

---

### Task 1: Database — Add boss_pending_rewards Table

**Files:**
- Modify: `rust-server/src/db.rs`

**Step 1: Add table creation**

In `db.rs`, find the migration section (near `koth_pending_rewards` table creation around line 651). Add a new migration for `boss_pending_rewards`:

```sql
CREATE TABLE IF NOT EXISTS boss_pending_rewards (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    player_id TEXT NOT NULL,
    item_id TEXT NOT NULL,
    quantity INTEGER NOT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
)
```

**Step 2: Add DB methods**

Add three methods mirroring the KOTH pattern (around line 706):

```rust
pub async fn add_boss_pending_reward(
    &self,
    player_id: &str,
    item_id: &str,
    quantity: u32,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO boss_pending_rewards (player_id, item_id, quantity) VALUES (?, ?, ?)")
        .bind(player_id)
        .bind(item_id)
        .bind(quantity as i64)
        .execute(&self.pool)
        .await?;
    Ok(())
}

pub async fn get_boss_pending_rewards(
    &self,
    player_id: &str,
) -> Result<Vec<(String, u32)>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, i64)>(
        "SELECT item_id, quantity FROM boss_pending_rewards WHERE player_id = ? ORDER BY created_at"
    )
    .bind(player_id)
    .fetch_all(&self.pool)
    .await?;
    Ok(rows.into_iter().map(|(id, qty)| (id, qty as u32)).collect())
}

pub async fn claim_boss_pending_rewards(
    &self,
    player_id: &str,
) -> Result<Vec<(String, u32)>, sqlx::Error> {
    let rewards = self.get_boss_pending_rewards(player_id).await?;
    sqlx::query("DELETE FROM boss_pending_rewards WHERE player_id = ?")
        .bind(player_id)
        .execute(&self.pool)
        .await?;
    Ok(rewards)
}
```

**Step 3: Verify server compiles**

Run: `cargo check` in `rust-server/`

**Step 4: Commit**

```
feat: add boss_pending_rewards DB table and methods
```

---

### Task 2: Boss State — Track Damage Dealers

**Files:**
- Modify: `rust-server/src/boss.rs` (BossState struct, ~line 165)
- Modify: `rust-server/src/game.rs` (NPC combat damage, ~line 3953)

**Step 1: Add damage_dealers to BossState**

In `rust-server/src/boss.rs`, add to the `BossState` struct:
```rust
pub damage_dealers: std::collections::HashSet<String>,
```

Initialize it in the constructor:
```rust
damage_dealers: std::collections::HashSet::new(),
```

**Step 2: Track damage in combat**

In `rust-server/src/game.rs`, find where instance NPC damage is applied (~line 3953, after `npc.take_damage()`). After damage is dealt to a boss NPC, record the attacker:

```rust
// Track boss damage dealers
if damage > 0 {
    if let Some(ref inst_id) = attacker_instance {
        let mut boss_states = self.boss_states.write().await;
        if let Some(boss) = boss_states.get_mut(inst_id) {
            boss.damage_dealers.insert(player_id.to_string());
        }
    }
}
```

Find the right location — it should be right after the `take_damage` call in the instance NPC combat path. Make sure `player_id` is available in scope (it should be, since it's the attacker).

**Step 3: Verify server compiles**

Run: `cargo check` in `rust-server/`

**Step 4: Commit**

```
feat: track boss damage dealers for loot distribution
```

---

### Task 3: Boss Death — Roll and Persist Loot

**Files:**
- Modify: `rust-server/src/game/boss_tick.rs` (BossDied handler, ~line 431)
- Modify: `rust-server/src/boss.rs` (BossEvent enum, if needed)

**Step 1: Roll loot for each damage dealer on boss death**

In `rust-server/src/game/boss_tick.rs`, in the `BossEvent::BossDied` handler (around line 431), after the existing death handling logic, add loot rolling:

```rust
// Roll loot independently for each damage dealer
let damage_dealers: Vec<String> = {
    let boss_states = self.boss_states.read().await;
    boss_states.get(&instance_id)
        .map(|b| b.damage_dealers.iter().cloned().collect())
        .unwrap_or_default()
};

if !damage_dealers.is_empty() {
    // Get boss prototype for loot table
    let prototype = self.entity_registry.get("desert_wurm");

    if let Some(proto) = prototype {
        let mut rng = rand::thread_rng();
        let mut loot_announcements: Vec<String> = Vec::new();

        for pid in &damage_dealers {
            let mut player_loot: Vec<(String, u32)> = Vec::new();

            // Roll gold
            let gold = rng.gen_range(proto.rewards.gold_min..=proto.rewards.gold_max);
            if gold > 0 {
                if let Err(e) = self.db.add_boss_pending_reward(&pid, "gold", gold as u32).await {
                    tracing::error!("Failed to store boss gold reward for {}: {}", pid, e);
                }
                player_loot.push(("gold".to_string(), gold as u32));
            }

            // Roll loot table
            for entry in &proto.loot {
                if rng.gen::<f32>() < entry.drop_chance {
                    let quantity = rng.gen_range(entry.quantity_min..=entry.quantity_max);
                    if quantity > 0 {
                        if let Err(e) = self.db.add_boss_pending_reward(&pid, &entry.item_id, quantity as u32).await {
                            tracing::error!("Failed to store boss loot for {}: {}", pid, e);
                        }
                        player_loot.push((entry.item_id.clone(), quantity as u32));
                    }
                }
            }

            // Award combat XP
            let xp = proto.rewards.exp_base;
            // XP granting would go here if needed

            // Build announcement string
            let player_name = {
                let players = self.players.read().await;
                players.get(pid).map(|p| p.name.clone()).unwrap_or_else(|| pid.clone())
            };
            let loot_str: Vec<String> = player_loot.iter().map(|(id, qty)| {
                if id == "gold" {
                    format!("{} gold", qty)
                } else {
                    let display = self.item_registry.get(id)
                        .map(|d| d.display_name.clone())
                        .unwrap_or_else(|| id.clone());
                    format!("{}x {}", qty, display)
                }
            }).collect();
            if !loot_str.is_empty() {
                loot_announcements.push(format!("{} received: {}", player_name, loot_str.join(", ")));
            }
        }

        // Announce loot rolls to instance
        if !loot_announcements.is_empty() {
            let announcement = loot_announcements.join("\n");
            self.send_to_instance(
                &instance_id,
                ServerMessage::Announcement {
                    text: format!("Loot Rolls:\n{}", announcement),
                },
            ).await;
        }
    }
}
```

You'll need `use rand::Rng;` at the top of the file if not already imported. Check existing imports.

**Step 2: Verify server compiles**

Run: `cargo check` in `rust-server/`

**Step 3: Commit**

```
feat: roll and persist boss loot on death with chat announcements
```

---

### Task 4: Battle Master — Add boss_rewards Behavior Flag

**Files:**
- Modify: `rust-server/src/entity/prototype.rs` (EntityBehaviors struct)
- Modify: `rust-server/data/entities/monsters/desert_boss.toml` (wurm_battle_master)

**Step 1: Add boss_rewards flag to EntityBehaviors**

In `rust-server/src/entity/prototype.rs`, find the `EntityBehaviors` struct (~line 306). Add:
```rust
pub boss_rewards: bool,
```

Make sure it has `#[serde(default)]` or is included in the default initialization if EntityBehaviors derives Default.

Also check `RawEntityPrototype` — the behaviors may be parsed from a sub-table. The field should be added wherever behaviors are deserialized.

**Step 2: Set the flag on wurm_battle_master**

In `rust-server/data/entities/monsters/desert_boss.toml`, add to `[wurm_battle_master.behaviors]`:
```toml
boss_rewards = true
```

**Step 3: Verify server compiles**

Run: `cargo check` in `rust-server/`

**Step 4: Commit**

```
feat: add boss_rewards behavior flag for Battle Master NPC
```

---

### Task 5: Battle Master — Interaction and Dialogue

**Files:**
- Modify: `rust-server/src/game.rs` (handle_interact_object ~line 4960, handle_dialogue_choice ~line 5063)

**Step 1: Add interaction routing**

In `rust-server/src/game.rs`, in `handle_interact_object()`, find the KOTH rewards routing block (~line 4960). Add a similar block right after it for boss rewards:

```rust
let is_boss_rewards = self
    .entity_registry
    .get(&entity_type)
    .map(|p| p.behaviors.boss_rewards)
    .unwrap_or(false);

if is_boss_rewards {
    self.show_boss_rewards_dialogue(player_id, &npc_id).await;
    return;
}
```

**Step 2: Add show_boss_rewards_dialogue method**

Add a new method on `GameRoom` (can go near the KOTH rewards methods, or in boss_tick.rs):

```rust
async fn show_boss_rewards_dialogue(&self, player_id: &str, npc_id: &str) {
    let rewards = match self.db.get_boss_pending_rewards(player_id).await {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("Failed to get boss rewards for {}: {}", player_id, e);
            return;
        }
    };

    if rewards.is_empty() {
        // No rewards pending
        self.send_to_player(
            player_id,
            ServerMessage::ShowDialogue {
                quest_id: format!("boss_rewards:{}", npc_id),
                npc_id: npc_id.to_string(),
                speaker: "Battle Master".to_string(),
                text: "Hail, hunter! Defeat the Desert Wurm and I'll distribute the spoils.".to_string(),
                choices: vec![DialogueChoice {
                    id: "close".to_string(),
                    text: "Farewell".to_string(),
                }],
            },
        )
        .await;
    } else {
        // Aggregate rewards for display
        let mut aggregated: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
        for (item_id, qty) in &rewards {
            *aggregated.entry(item_id.clone()).or_insert(0) += qty;
        }

        let reward_lines: Vec<String> = aggregated.iter().map(|(id, qty)| {
            if id == "gold" {
                format!("  {} gold", qty)
            } else {
                let display = self.item_registry.get(id)
                    .map(|d| d.display_name.clone())
                    .unwrap_or_else(|| id.clone());
                format!("  {}x {}", qty, display)
            }
        }).collect();

        let text = format!(
            "Well fought, hunter! I've collected your share of the spoils:\n\n{}\n\nWould you like to claim them?",
            reward_lines.join("\n")
        );

        self.send_to_player(
            player_id,
            ServerMessage::ShowDialogue {
                quest_id: format!("boss_rewards:{}", npc_id),
                npc_id: npc_id.to_string(),
                speaker: "Battle Master".to_string(),
                text,
                choices: vec![
                    DialogueChoice {
                        id: "claim".to_string(),
                        text: "Claim Rewards".to_string(),
                    },
                    DialogueChoice {
                        id: "close".to_string(),
                        text: "Not Yet".to_string(),
                    },
                ],
            },
        )
        .await;
    }
}
```

**Step 3: Add dialogue choice handler**

In `handle_dialogue_choice()` (~line 5063), add a handler for `"boss_rewards:"` prefix, right near the KOTH handler:

```rust
if quest_id.starts_with("boss_rewards:") {
    if choice_id == "claim" {
        self.send_to_player(player_id, ServerMessage::DialogueClosed).await;
        self.claim_boss_rewards(player_id).await;
    } else {
        self.send_to_player(player_id, ServerMessage::DialogueClosed).await;
    }
    return;
}
```

**Step 4: Add claim_boss_rewards method**

```rust
async fn claim_boss_rewards(&self, player_id: &str) {
    let rewards = match self.db.claim_boss_pending_rewards(player_id).await {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("Failed to claim boss rewards for {}: {}", player_id, e);
            return;
        }
    };

    if rewards.is_empty() {
        return;
    }

    let mut total_gold = 0u32;
    let mut item_count = 0u32;

    for (item_id, quantity) in &rewards {
        if item_id == "gold" {
            total_gold += quantity;
        } else {
            self.grant_item_to_player(player_id, item_id, *quantity).await;
            item_count += 1;
        }
    }

    if total_gold > 0 {
        let mut players = self.players.write().await;
        if let Some(player) = players.get_mut(player_id) {
            player.inventory.gold += total_gold as i32;
        }
    }

    self.send_system_message(
        player_id,
        &format!("You claimed your boss rewards! ({} items, {} gold)", item_count, total_gold),
    ).await;
}
```

**Step 5: Verify server compiles**

Run: `cargo check` in `rust-server/`

**Step 6: Commit**

```
feat: add Battle Master reward claiming dialogue and flow
```

---

### Task 6: Integration — Verify Full Flow

**Step 1: Build server**

```bash
cd rust-server && cargo build
```

**Step 2: Manual test checklist**

- [ ] Start server, enter boss arena, fight the boss
- [ ] Deal damage to boss with at least one player
- [ ] Kill the boss
- [ ] Loot roll announcement appears in chat showing what each player received
- [ ] After teleport to overworld, talk to Battle Master at (-258, -127)
- [ ] Dialogue shows pending rewards with item names and quantities
- [ ] Click "Claim Rewards" — items appear in inventory, gold is added
- [ ] Talk to Battle Master again — shows "no rewards" dialogue
- [ ] Kill boss again, rewards accumulate
- [ ] Restart server, talk to Battle Master — rewards still pending (persisted)

**Step 3: Commit any fixes**

```
fix: polish boss reward claiming flow
```
