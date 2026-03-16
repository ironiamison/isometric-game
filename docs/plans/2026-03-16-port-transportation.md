# Port Transportation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add port master NPCs that offer a travel menu with priced destinations, teleporting the player with a fade effect.

**Architecture:** New `port_master` behavior flag + `PortConfig` on prototypes. Server shows destinations via existing `ShowDialogue`, handles choice by deducting gold and teleporting. Client triggers existing `MapTransition` fade for the travel effect.

**Tech Stack:** Rust server (prototype system, dialogue system), Rust client (Macroquad, existing transition overlay)

---

### Task 1: Add PortConfig and port_master behavior to prototype system

**Files:**
- Modify: `rust-server/src/entity/prototype.rs`
- Modify: `rust-server/src/entity/registry.rs`

**Step 1: Add PortDestination and PortConfig structs to prototype.rs**

After the `DialogueConfig` struct (~line 202), add:

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct PortDestination {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub cost: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PortConfig {
    #[serde(default)]
    pub destinations: Vec<PortDestination>,
}
```

**Step 2: Add `port_master` flag to RawEntityBehaviors**

In `RawEntityBehaviors` (~line 76), add after `koth_rewards`:

```rust
    #[serde(default)]
    pub port_master: bool,
```

**Step 3: Add `port_master` flag to EntityBehaviors**

In `EntityBehaviors` (~line 288), add after `koth_rewards`:

```rust
    pub port_master: bool,
```

In `EntityBehaviors::default()` (~line 312), add after `koth_rewards: false,`:

```rust
    port_master: false,
```

In `impl From<&RawEntityBehaviors> for EntityBehaviors` (~line 338), add after `koth_rewards: raw.koth_rewards,`:

```rust
    port_master: raw.port_master,
```

**Step 4: Add `port` field to RawEntityPrototype and EntityPrototype**

In `RawEntityPrototype` (~line 206), add after `speech`:

```rust
    pub port: Option<PortConfig>,
```

In `EntityPrototype` (~line 366), add after `speech`:

```rust
    pub port: Option<PortConfig>,
```

**Step 5: Add `port_master` to `is_npc()` check**

In `EntityPrototype::is_npc()` (~line 391), add `|| self.behaviors.port_master` to the chain.

**Step 6: Wire up port config in registry.rs resolve**

In `registry.rs` resolve function (~line 290), add after the `speech` field:

```rust
            port: raw
                .port
                .clone()
                .or_else(|| parent.and_then(|p| p.port.clone())),
```

**Step 7: Verify it compiles**

Run: `cd rust-server && cargo check 2>&1 | head -20`
Expected: No new errors

**Step 8: Commit**

```bash
git add rust-server/src/entity/prototype.rs rust-server/src/entity/registry.rs
git commit -m "feat: add PortConfig and port_master behavior to prototype system"
```

---

### Task 2: Add port_master flag to NPC server struct and NpcUpdate

**Files:**
- Modify: `rust-server/src/npc.rs`

**Step 1: Add `is_port_master` to PrototypeStats**

In `PrototypeStats` (~line 42), add after `is_friendly`:

```rust
    pub is_port_master: bool,
```

**Step 2: Set it in `Npc::from_prototype()`**

In `from_prototype()` (~line 132), in the `PrototypeStats` constructor, add after `is_friendly: prototype.behaviors.friendly,`:

```rust
    is_port_master: prototype.behaviors.port_master,
```

**Step 3: Add helper method**

After `is_friendly()` (~line 265), add:

```rust
    pub fn is_port_master(&self) -> bool {
        self.stats.is_port_master
    }
```

**Step 4: Add `is_port_master` to NpcUpdate struct**

In `NpcUpdate` (~line 778), add after `is_friendly`:

```rust
    /// Whether this NPC is a port master (travel services)
    pub is_port_master: bool,
```

**Step 5: Set it in `From<&Npc> for NpcUpdate`**

In the `From` impl (~line 822), add after `is_friendly: npc.is_friendly(),`:

```rust
    is_port_master: npc.is_port_master(),
```

**Step 6: Add to is_hostile() exclusion**

In `is_hostile()` (~line 274), add `&& !self.stats.is_port_master` to the condition so port masters aren't treated as hostile.

**Step 7: Verify it compiles**

Run: `cd rust-server && cargo check 2>&1 | head -20`
Expected: No new errors (some warnings about unused field are fine)

**Step 8: Commit**

```bash
git add rust-server/src/npc.rs
git commit -m "feat: add port_master flag to NPC struct and NpcUpdate"
```

---

### Task 3: Handle port master interaction on the server

**Files:**
- Modify: `rust-server/src/game.rs`

**Step 1: Add port master check in `handle_npc_interact()`**

In `handle_npc_interact()`, after the KOTH rewards check block (~line 4972) and before the merchant shop check, add:

```rust
        // Port master interaction - show travel destinations
        let is_port_master = self
            .entity_registry
            .get(&entity_type)
            .map(|p| p.behaviors.port_master)
            .unwrap_or(false);

        if is_port_master {
            self.show_port_master_dialogue(player_id, &npc_id, &entity_type)
                .await;
            return;
        }
```

**Step 2: Add `show_port_master_dialogue()` method**

Add this method to the GameRoom impl (near the other `show_*_dialogue` methods):

```rust
    async fn show_port_master_dialogue(&self, player_id: &str, npc_id: &str, entity_type: &str) {
        let prototype = match self.entity_registry.get(entity_type) {
            Some(p) => p,
            None => return,
        };

        let port_config = match &prototype.port {
            Some(c) => c,
            None => return,
        };

        let player_gold = {
            let players = self.players.read().await;
            match players.get(player_id) {
                Some(p) => p.inventory.gold,
                None => return,
            }
        };

        let speaker = prototype.display_name.clone();
        let greeting = prototype
            .dialogue
            .greeting
            .clone()
            .unwrap_or_else(|| "Where would you like to travel?".to_string());

        let mut choices: Vec<crate::protocol::DialogueChoice> = port_config
            .destinations
            .iter()
            .enumerate()
            .map(|(i, dest)| {
                let affordable = player_gold >= dest.cost;
                let label = if affordable {
                    format!("{} - {}g", dest.name, dest.cost)
                } else {
                    format!("{} - {}g (not enough gold)", dest.name, dest.cost)
                };
                crate::protocol::DialogueChoice {
                    id: format!("port_dest_{}", i),
                    text: label,
                }
            })
            .collect();

        choices.push(crate::protocol::DialogueChoice {
            id: "close".to_string(),
            text: "Nevermind".to_string(),
        });

        self.send_to_player(
            player_id,
            ServerMessage::ShowDialogue {
                quest_id: format!("port:{}", npc_id),
                npc_id: npc_id.to_string(),
                speaker,
                text: greeting,
                choices,
            },
        )
        .await;
    }
```

**Step 3: Handle port dialogue choice in `handle_dialogue_choice()`**

In `handle_dialogue_choice()` (~line 4986), after the waystone block (~line 5073) and before the final `handle_quest_dialogue_choice`, add:

```rust
        // Handle port master travel choices (format: "port:{npc_id}")
        if let Some(npc_id) = quest_id.strip_prefix("port:") {
            if let Some(dest_str) = choice_id.strip_prefix("port_dest_") {
                if let Ok(dest_index) = dest_str.parse::<usize>() {
                    self.handle_port_travel(player_id, npc_id, dest_index).await;
                }
            } else {
                self.send_to_player(player_id, ServerMessage::DialogueClosed)
                    .await;
            }
            return;
        }
```

**Step 4: Add `handle_port_travel()` method**

```rust
    async fn handle_port_travel(&self, player_id: &str, npc_id: &str, dest_index: usize) {
        // Close dialogue first
        self.send_to_player(player_id, ServerMessage::DialogueClosed)
            .await;

        // Look up the NPC's prototype to get the port config
        let (entity_type, npc_x, npc_y) = {
            let npcs = self.npcs.read().await;
            match npcs.get(npc_id) {
                Some(npc) => (npc.prototype_id.clone(), npc.x, npc.y),
                None => return,
            }
        };

        let prototype = match self.entity_registry.get(&entity_type) {
            Some(p) => p,
            None => return,
        };

        let port_config = match &prototype.port {
            Some(c) => c,
            None => return,
        };

        let destination = match port_config.destinations.get(dest_index) {
            Some(d) => d,
            None => return,
        };

        // Verify player is still near the NPC
        let mut players = self.players.write().await;
        let player = match players.get_mut(player_id) {
            Some(p) if p.active && !p.is_dead => p,
            _ => return,
        };

        let dx = (player.x - npc_x) as f32;
        let dy = (player.y - npc_y) as f32;
        if (dx * dx + dy * dy).sqrt() > 5.0 {
            return;
        }

        // Check gold
        if player.inventory.gold < destination.cost {
            drop(players);
            self.send_system_message(player_id, "You don't have enough gold for that trip.")
                .await;
            return;
        }

        // Deduct gold and teleport
        player.inventory.gold -= destination.cost;
        player.x = destination.x;
        player.y = destination.y;
        // Reset movement state
        player.vel_x = 0;
        player.vel_y = 0;
        player.is_moving = false;

        let new_gold = player.inventory.gold;
        drop(players);

        // Send gold update
        self.send_to_player(
            player_id,
            ServerMessage::GoldUpdate { gold: new_gold },
        )
        .await;

        // Send system message
        self.send_system_message(
            player_id,
            &format!("You travel to {}. (-{}g)", destination.name, destination.cost),
        )
        .await;
    }
```

Note: Check that `ServerMessage::GoldUpdate` exists. If not, look for whatever message the server uses to sync gold after changes (it may be part of InventoryUpdate or StateSync). Adjust accordingly.

**Step 5: Verify it compiles**

Run: `cd rust-server && cargo check 2>&1 | head -30`
Expected: No new errors. Fix any issues with field names or missing variants.

**Step 6: Commit**

```bash
git add rust-server/src/game.rs
git commit -m "feat: handle port master interaction and travel on server"
```

---

### Task 4: Add client-side port master flag and travel fade effect

**Files:**
- Modify: `client/src/network/message_handler.rs`
- Modify: `client/src/game/state.rs` (if client NPC struct needs `is_port_master`)

**Step 1: Check how client NPC struct receives flags**

Search for where `is_port_master` would be parsed from NpcUpdate in the message handler. The client NPC struct in `client/src/game/state.rs` or `client/src/game/npc.rs` should have an `is_port_master` field added, and the message handler should parse it from the NPC update data.

Add `is_port_master: bool` to the client-side Npc struct, and parse it where other flags like `is_merchant`, `is_banker` are parsed from the server NpcUpdate.

**Step 2: Trigger fade on port travel**

When the client receives a `ShowDialogue` with quest_id starting with `"port:"`, it already renders as a normal dialogue — no special handling needed. The player clicks a destination, sends `DialogueChoiceMsg`, and the server teleports them.

The key is: the server teleports the player's position directly. The next `StateSync` will show the player at the new location. The client already handles large position jumps (snap if distance > 4 tiles).

For the fade effect, we want to trigger a brief fade-out/fade-in when the player selects a port destination. The simplest approach:

In the `showDialogue` handler in `message_handler.rs`, when receiving a `dialogueClosed` after a port dialogue choice, **OR** better: detect the position jump in StateSync and trigger a fade.

Actually, the cleanest approach: when the client sends a port travel dialogue choice, immediately start a fade-out. When the StateSync arrives with the new position, transition to fade-in.

In the client's dialogue choice sending code, check if the active dialogue's quest_id starts with `"port:"` and the choice starts with `"port_dest_"`. If so, trigger:

```rust
state.map_transition = MapTransition {
    state: TransitionState::FadingOut,
    progress: 0.0,
    target_map_type: String::new(),
    target_map_id: String::new(),
    target_spawn_x: 0.0,
    target_spawn_y: 0.0,
    instance_id: String::new(),
};
```

Then, when the next StateSync moves the player a large distance (or when `dialogueClosed` is received while a port fade is active), transition to `FadingIn`.

The simplest reliable approach: when `dialogueClosed` is received and `map_transition.state` is `FadingOut` or `Loading`, switch to `FadingIn`. This pairs with starting fade on dialogue choice.

Find where `DialogueChoice` messages are sent from the client (likely in `client/src/input/handler.rs` or the dialogue UI click handler). Add the fade trigger there.

**Step 3: Verify it compiles**

Run: `cd client && cargo check 2>&1 | head -20`

**Step 4: Commit**

```bash
git add client/src/network/message_handler.rs client/src/game/state.rs client/src/input/handler.rs
git commit -m "feat: add port master flag to client and travel fade effect"
```

---

### Task 5: Create port master NPC data file

**Files:**
- Create: `rust-server/data/entities/npcs/port_masters.toml`

**Step 1: Create the TOML file**

```toml
# Port Masters - Travel NPCs at docks/ports

# ============================================================================
# Port Master - Generic port NPC (can be reused at different locations)
# ============================================================================
[port_master]
display_name = "Port Master"
sprite = "lee"
animation_type = "humanoid"
description = "A seasoned sailor who arranges passage between ports."

[port_master.stats]
max_hp = 150
damage = 0
attack_range = 0
aggro_range = 0
chase_range = 0
move_cooldown_ms = 0
attack_cooldown_ms = 0
respawn_time_ms = 0

[port_master.rewards]
exp_base = 0
gold_min = 0
gold_max = 0

[port_master.behaviors]
hostile = false
friendly = true
port_master = true
wander_enabled = false
facing = "down"

[port_master.port]
destinations = [
    { name = "Oakshore Docks", x = 100, y = 200, cost = 50 },
]

[port_master.speech]
radius = 5
interval_min_ms = 25000
interval_max_ms = 45000
messages = [
    "Need passage? I can arrange a voyage.",
    "The seas are calm today. Good for travel.",
    "All aboard! Next ship departs shortly.",
    "Safe travels, adventurer.",
]

[port_master.dialogue]
greeting = "Ahoy! Looking for passage? Here are the routes I can offer."
```

Note: The exact destination coordinates and costs should be adjusted when actually placing port masters on the map. Each port NPC placed on the map should have its own prototype with the correct destinations, OR you can create multiple prototypes (e.g., `port_master_oakshore`, `port_master_village`) each with different destination lists.

**Step 2: Commit**

```bash
git add rust-server/data/entities/npcs/port_masters.toml
git commit -m "feat: add port master NPC data file"
```

---

### Task 6: Wire up NpcUpdate serialization for port_master flag

**Files:**
- Modify: `rust-server/src/protocol.rs` (NpcUpdate serialization)

**Step 1: Find NpcUpdate serialization**

Search for where `is_friendly`, `is_merchant` etc. are serialized in protocol.rs. Add `is_port_master` in the same pattern.

**Step 2: Find NpcUpdate deserialization on client**

Search in `client/src/network/message_handler.rs` for where `is_merchant`, `is_friendly` etc. are parsed from the NPC state sync data. Add `is_port_master` parsing in the same pattern.

**Step 3: Verify both compile**

Run: `cd rust-server && cargo check 2>&1 | head -20`
Run: `cd client && cargo check 2>&1 | head -20`

**Step 4: Commit**

```bash
git add rust-server/src/protocol.rs client/src/network/message_handler.rs
git commit -m "feat: serialize port_master flag in NpcUpdate protocol"
```

---

### Task 7: Test end-to-end

**Step 1: Place a port master NPC on the map**

Add an NPC entry to a chunk JSON file (e.g., `rust-server/maps/world_0/chunk_0_0.json`) or wherever NPCs are spawned, referencing the `port_master` prototype.

**Step 2: Run the server and client**

Run: `cd rust-server && cargo run`
Run: `cd client && cargo run`

**Step 3: Test the flow**

1. Walk to the port master NPC
2. Right-click to interact
3. Verify dialogue shows with destination list and prices
4. Select a destination with enough gold
5. Verify gold is deducted and player teleports with fade effect
6. Verify system message confirms travel
7. Try selecting a destination without enough gold — verify error message

**Step 4: Final commit**

```bash
git add -A
git commit -m "feat: port master NPC transportation system"
```
