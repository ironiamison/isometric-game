# Construction Skill — Implementation Plan

## Context

Adding a Construction skill that lets players purchase an instanced house, expand it with rooms, and furnish it Sims-style with a catalog/placement UI. Furniture provides instant effects (restore HP/prayer, open crafting, storage). XP comes from a build-and-remove cycle. Materials come from existing skills (woodcutting, mining, smithing) or gold for basic items.

---

## Phase 1: Foundation — Skill, Data Definitions, Persistence

### 1.1 Add Construction to Skills Enum
- **Server**: `rust-server/src/skills.rs` — add `Construction` to `SkillType` enum, `as_str()`, `from_str()`, `all()`, `Skills` struct (`#[serde(default)]`), `Skills::new()`, `get()/get_mut()`, `total_level()`
- **Client**: `client/src/game/skills.rs` — mirror all additions
- **Protocol**: `rust-server/src/protocol.rs` — add `construction_level`/`construction_xp` to `SkillsSync`
- **Client handler**: parse new skill fields in message handler

### 1.2 Furniture Definitions (TOML)
- **New dir**: `rust-server/data/construction/`
- **New file**: `rust-server/data/construction/furniture.toml`
- Each furniture entry: `id`, `display_name`, `sprite`, `room_type`, `level_required`, `xp`, `size` (tiles), `rotation` (bool), `cost` (gold or materials list), optional `interaction` (restore_prayer, restore_hp, open_crafting, storage)
- **New files**: `rust-server/src/construction/furniture_def.rs`, `furniture_registry.rs` — follows `ItemDefinition`/`CraftingRegistry` patterns

### 1.3 Room Definitions (TOML)
- **New file**: `rust-server/data/construction/rooms.toml`
- Each room: `id`, `display_name`, `width`/`height` (8x8 default), `cost` (gold), `level_required`, `doorways` (edge, position, which room types can connect)
- **New files**: `rust-server/src/construction/room_def.rs`, `room_registry.rs`

**Starter rooms:**
| Room | Level | Cost | Purpose |
|------|-------|------|---------|
| Parlour | 1 | 5,000g | Starting room, decorative |
| Kitchen | 5 | 10,000g | Cooking range, larder |
| Bedroom | 20 | 10,000g | Bed (restore HP), wardrobe |
| Workshop | 15 | 15,000g | Workbench for crafting |
| Chapel | 45 | 50,000g | Altar (restore prayer) |
| Garden | 10 | 10,000g | Outdoor decorations |

### 1.4 New Material Items
- Add to `rust-server/data/items/materials.toml`: `oak_plank`, `nails`, `limestone_block`, `cloth`
- Add recipes to new `rust-server/data/recipes/construction_materials.toml`: oak_plank (from oak_log at sawmill), nails (from iron_bar at anvil), limestone_block (from limestone at stonecutter)

### 1.5 House Persistence (SQLite)
- **Modify**: `rust-server/src/db.rs` — new table `player_houses` with `character_id PRIMARY KEY` + `house_data_json TEXT`
- Functions: `load_character_house()`, `save_character_house()` — follows `slayer_state` pattern
- **New file**: `rust-server/src/construction/house_data.rs`

```
HouseData { purchased, rooms: Vec<PlacedRoom>, furniture: Vec<PlacedFurniture> }
PlacedRoom { room_id, grid_x, grid_y }
PlacedFurniture { furniture_id, room_index, local_x, local_y, rotation (0-3) }
```

---

## Phase 2: Server-Side Construction Logic

### 2.1 House Instance Generation
- **New file**: `rust-server/src/construction/instance_gen.rs`
- Takes `HouseData` + registries → generates map dynamically: ground layer, walls, collision grid, exit portal in parlour
- Rooms compose on a grid (each 8x8 tiles, shared walls, doorway openings at matching positions)
- Map ID format: `"house_{player_id}"` using `InstanceManager::get_or_create_private()`
- Collision grid updated in-place when furniture placed/removed (Instance already has `collision: RwLock<Vec<bool>>`)

### 2.2 Protocol Messages
- **Modify**: `rust-server/src/protocol.rs`

**ClientMessage additions:**
- `EnterBuildMode`, `ExitBuildMode`
- `PlaceFurniture { furniture_id, room_index, local_x, local_y, rotation }`
- `RemoveFurniture { furniture_index }`
- `AddRoom { room_id, doorway_room_index, doorway_index }`
- `PurchaseHouse`
- `EnterHouse`
- `InteractFurniture { furniture_index }`

**ServerMessage additions:**
- `HouseSync { house_data_json, map_width, map_height, ground_layer, collision, furniture_catalog_json, room_catalog_json }`
- `BuildModeChanged { active }`
- `FurniturePlaced { furniture_index, furniture_id, room_index, local_x, local_y, rotation }`
- `FurnitureRemoved { furniture_index }`
- `RoomAdded { room_id, grid_x, grid_y }` → triggers full HouseSync

### 2.3 Construction Handler
- **New file**: `rust-server/src/game/construction.rs`
- `handle_place_furniture()` — validate level, check materials/gold, deduct cost, award Construction XP, add to HouseData, update collision, broadcast, persist
- `handle_remove_furniture()` — remove from HouseData, update collision, broadcast (no materials returned — RS-style XP sink)
- `handle_add_room()` — validate level + gold, regenerate entire instance map (dimensions change), send fresh HouseSync
- `handle_purchase_house()` — create initial HouseData with parlour, deduct gold
- `handle_enter_house()` — load from DB, generate instance, enter player
- `handle_interact_furniture()` — dispatch based on interaction type (restore prayer, restore HP, open crafting, etc.)

### 2.4 Player/GameRoom Extensions
- **Modify**: `rust-server/src/game.rs`
  - `Player` struct: add `in_build_mode: bool`, `house_data: Option<HouseData>`
  - `GameRoom` struct: add `furniture_registry: Arc<FurnitureRegistry>`, `room_registry: Arc<RoomRegistry>`

### 2.5 Estate Agent NPC
- Add Estate Agent NPC definition in entity data (existing NPC/dialogue system)
- Place in town near a house portal
- Dialogue options: purchase house, enter house, add rooms
- House portal object in overworld for quick entry

### 2.6 Message Dispatch
- **Modify**: `rust-server/src/main.rs` — add match arms for all new ClientMessage variants in the dispatch block

### 2.7 Save Integration
- **Modify**: player save flow in `main.rs` — call `save_character_house()` on disconnect and auto-save, alongside slayer state

---

## Phase 3: Client-Side UI and Rendering

### 3.1 Build Mode UI Panel (Sims-style)
- **New file**: `client/src/render/ui/construction.rs`
- Side panel (like crafting panel) with:
  - **Category tabs**: Chairs, Tables, Decorations, Storage, Functional, Rooms
  - **Scrollable list**: furniture icons, names, level requirements, material costs
  - **Material availability**: green/red tint based on inventory
  - **"Place" button** → enters placement mode
  - **"Remove" toggle** → click furniture to remove
  - **"Add Room" section** → shows available rooms at doorway connections

### 3.2 Ghost Preview System
- **New file**: `client/src/render/ghost_preview.rs`
- Semi-transparent furniture sprite follows cursor, snapped to tile grid via `screen_to_world()`
- Green tint = valid placement, Red tint = blocked/occupied/wrong room
- Click to confirm → sends `PlaceFurniture` to server
- R key or button to cycle rotation (0→1→2→3)

### 3.3 Furniture Rendering
- Placed furniture renders as objects in the instance's object layer
- Standard isometric sprites (64x32 base), depth-sorted by y-coordinate (existing system handles this)
- 4 rotation variants per sprite (separate frames or rotation offset)

### 3.4 Client State Extensions
- Add to GameState: `build_mode_active`, `house_data`, `furniture_catalog`, `room_catalog`, `selected_furniture_id`, `placement_rotation`, `build_mode_remove`

### 3.5 Message Handler
- **Modify**: `client/src/network/message_handler.rs` — handle `HouseSync`, `BuildModeChanged`, `FurniturePlaced`, `FurnitureRemoved`, `RoomAdded`

---

## Phase 4: Integration and Polish

### 4.1 Module Structure
- **New**: `rust-server/src/construction/mod.rs` with submodules: `furniture_def`, `furniture_registry`, `room_def`, `room_registry`, `house_data`, `instance_gen`
- Register in `rust-server/src/main.rs`

### 4.2 Registry Loading
- Load furniture + room registries at startup alongside item/crafting registries

### 4.3 Interaction Dispatch
- `RestorePrayer` → reuse prayer restore logic from `game/prayer.rs`
- `RestoreHp` → direct HP heal, cap at max
- `OpenCrafting` → send crafting UI data with specified station/categories (reuses crafting system)
- `Storage` → similar to bank/chest panel

---

## Verification Plan

1. **Skill added correctly**: Check skills panel shows Construction at level 1 for existing characters
2. **Purchase flow**: Talk to Estate Agent → buy house → gold deducted → HouseData created in DB
3. **Enter house**: Use portal → private instance loads → parlour renders correctly with walls/floor
4. **Build mode**: Toggle build mode → catalog panel appears → furniture list populated from TOML
5. **Place furniture**: Select chair → ghost preview follows cursor → click to place → materials deducted → XP granted → furniture renders in house
6. **Remove furniture**: Toggle remove mode → click placed furniture → removed → space freed
7. **Add room**: Select room from catalog → attach to doorway → instance regenerates with new room
8. **Interactions**: Place altar → exit build mode → interact → prayer restored
9. **Persistence**: Place furniture → log out → log back in → enter house → furniture still there
10. **Rotation**: Place furniture → press R → cycles through 4 rotations → sprite updates

## Key Files to Modify
- `rust-server/src/skills.rs`
- `rust-server/src/protocol.rs`
- `rust-server/src/db.rs`
- `rust-server/src/game.rs`
- `rust-server/src/main.rs`
- `client/src/game/skills.rs`
- `client/src/network/message_handler.rs`
- `rust-server/data/items/materials.toml`

## New Files to Create
- `rust-server/src/construction/mod.rs`
- `rust-server/src/construction/furniture_def.rs`
- `rust-server/src/construction/furniture_registry.rs`
- `rust-server/src/construction/room_def.rs`
- `rust-server/src/construction/room_registry.rs`
- `rust-server/src/construction/house_data.rs`
- `rust-server/src/construction/instance_gen.rs`
- `rust-server/src/game/construction.rs`
- `rust-server/data/construction/furniture.toml`
- `rust-server/data/construction/rooms.toml`
- `rust-server/data/recipes/construction_materials.toml`
- `client/src/render/ui/construction.rs`
- `client/src/render/ghost_preview.rs`
