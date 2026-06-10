# Blast Spell (Magic Projectile) Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a new "Blast" spell that fires a visible sprite-based projectile from caster to target, with damage displayed on arrival.

**Architecture:** Add Blast to the static spell list (server). On the client, when a SpellEffect for "blast" arrives, spawn a sprite-based Projectile instead of an on-target effect. The projectile renderer is extended to support sprite animation alongside the existing procedural arrow. Damage number is delayed by the projectile travel time.

**Tech Stack:** Rust (server: spell.rs, game.rs), Rust (client: message_handler.rs, renderer.rs, state.rs), sprite_manifest.json

---

### Task 1: Add Blast spell definition (server)

**Files:**
- Modify: `rust-server/src/spell.rs:24` (SPELLS array)

**Step 1: Add the Blast spell to the SPELLS array**

Insert after the `dark_hand` entry (line 34):

```rust
SpellDef {
    id: "blast",
    name: "Blast",
    spell_type: SpellType::Damage,
    magic_level_req: 1,
    mana_cost: 2,
    cooldown_ms: 1000,
    base_power: 2,
    effect_sprite: "projectile",
},
```

**Step 2: Verify server compiles**

Run: `cd rust-server && cargo check`
Expected: compiles with no new errors

**Step 3: Commit**

```bash
git add rust-server/src/spell.rs
git commit -m "feat: add Blast spell definition (lvl 1, 2 mana, 1s cooldown)"
```

---

### Task 2: Server sends projectile data for Blast spell

**Files:**
- Modify: `rust-server/src/game.rs:7349-7358` (DamageEvent broadcast in `cast_damage_spell_resolved`)

The server already sends `SpellEffect` (line 7337) and `DamageEvent` (line 7350) for all damage spells. For Blast, we need to:
1. Set `projectile: Some("blast".to_string())` on the DamageEvent so the client knows to spawn a projectile
2. The `SpellEffect` will still be sent but the client will skip it for "blast" (handled in Task 5)

**Step 1: Set projectile field for blast spells**

At `rust-server/src/game.rs:7357`, change:

```rust
// Before:
projectile: None,

// After:
projectile: if spell_def.effect_sprite == "projectile" {
    Some("blast".to_string())
} else {
    None
},
```

**Step 2: Verify server compiles**

Run: `cd rust-server && cargo check`
Expected: compiles with no new errors

**Step 3: Commit**

```bash
git add rust-server/src/game.rs
git commit -m "feat: send projectile data in DamageEvent for blast spell"
```

---

### Task 3: Add projectile sprite to effects atlas manifest

**Files:**
- Modify: `client/assets/sprite_manifest.json` (effects_atlas.sprites section)

The `projectile.png` is 256x64 (4 frames, each 64x64). It's a standalone file at `sprites/effects/projectile.png`, not in the effects_atlas. We need to add it to the effects_atlas section of the manifest so the renderer can find it.

**BUT** — the effects_atlas is a single packed atlas PNG. The projectile isn't in that atlas image. Instead, we should load it as a standalone effect texture. Looking at how the renderer handles effects, it uses `spell_effect_textures: SpritesheetStore` which can hold individual textures.

**Alternative approach:** Add the projectile to the manifest as a standalone entry in effects_atlas. The atlas loader should handle sprites that reference different source files — but looking at the code, the atlas stores a single texture with sub-rects. So we need to either:
- (A) Pack projectile.png into the effects_atlas.png, or
- (B) Load it separately

Since the projectile is used in the projectile renderer (not spell_effect renderer), the simplest approach is to load it in the renderer's `render_projectiles` method directly from the spell_effect_textures store if the sprite name matches, OR load it as a standalone texture.

**Simplest path:** Load the projectile texture once during renderer init and store it alongside the existing textures. The renderer already has `spell_effect_textures: SpritesheetStore` — we just need to add the projectile sprite to it as an individual entry.

**Step 1: Add projectile to effects_atlas in manifest**

In `client/assets/sprite_manifest.json`, in the `effects_atlas.sprites` section (after the `self_heal` entry around line 31604), add:

```json
"projectile": {
    "x": 0,
    "y": 0,
    "w": 256,
    "h": 64,
    "file": "sprites/effects/projectile.png"
}
```

Note: This uses a per-sprite `file` override if the atlas loader supports it. If not, we'll load it standalone in the renderer (see Task 4).

**Step 2: Commit**

```bash
git add client/assets/sprite_manifest.json
git commit -m "feat: add projectile sprite to effects manifest"
```

---

### Task 4: Load projectile texture in renderer

**Files:**
- Modify: `client/src/render/renderer.rs` (renderer init + render_projectiles)

We need to understand how `spell_effect_textures` (SpritesheetStore) works. Check the SpritesheetStore type — it likely supports individual texture loading.

**Step 1: Load the projectile texture at init time**

Find where `spell_effect_textures` is populated (around line 1718 area). After loading the effects atlas, also load `projectile.png` as a standalone entry into spell_effect_textures:

```rust
// Load standalone effect sprites not in the atlas
let projectile_path = asset_path("assets/sprites/effects/projectile.png");
if let Ok(tex) = load_texture(&projectile_path).await {
    tex.set_filter(FilterMode::Nearest);
    spell_effect_textures.insert_individual("projectile", tex);
}
```

NOTE: The exact API depends on SpritesheetStore. If it's an enum (Atlas vs Individual), we may need to use a different approach — check the SpritesheetStore type first and adapt. The key requirement is that `self.spell_effect_textures.get("projectile")` returns the texture at render time.

**Step 2: Verify client compiles**

Run: `cd client && cargo check`

**Step 3: Commit**

```bash
git add client/src/render/renderer.rs
git commit -m "feat: load projectile sprite texture at init"
```

---

### Task 5: Render sprite-based projectiles

**Files:**
- Modify: `client/src/render/renderer.rs:5598-5697` (render_projectiles method)

Currently `render_projectiles` draws a procedural arrow for ALL projectiles. We need to add a branch: if `projectile.sprite == "blast"`, draw the animated sprite; otherwise draw the arrow as before.

**Step 1: Add sprite projectile rendering**

At the top of the `for projectile in &state.projectiles` loop (line 5601), add a check:

```rust
for projectile in &state.projectiles {
    let (world_x, world_y, world_z) = projectile.current_pos(current_time);
    let (screen_x, screen_y_raw) =
        world_to_screen_z(world_x, world_y, world_z, &state.camera);

    // Sprite-based projectile (e.g. blast spell)
    if projectile.sprite == "blast" {
        if let Some((texture, atlas_offset)) = self.spell_effect_textures.get("projectile") {
            let (tex_w, tex_h) = self
                .spell_effect_textures
                .get_dimensions("projectile")
                .unwrap_or((texture.width(), texture.height()));
            let frame_count = 4usize;
            let frame_w = tex_w / frame_count as f32;
            let frame_h = tex_h;

            // Animate: cycle through frames based on time
            let elapsed = current_time - projectile.start_time;
            let fps = 10.0;
            let frame_idx = ((elapsed * fps) as usize) % frame_count;

            let (offset_x, offset_y) = atlas_offset.unwrap_or((0.0, 0.0));
            let source_rect = Rect::new(
                offset_x + frame_idx as f32 * frame_w,
                offset_y,
                frame_w,
                frame_h,
            );

            let zoom = state.camera.zoom;
            let draw_w = frame_w * zoom;
            let draw_h = frame_h * zoom;
            let y_offset = -24.0 * zoom; // Match player center height

            draw_texture_ex(
                texture,
                screen_x - draw_w / 2.0,
                screen_y_raw + y_offset - draw_h / 2.0,
                WHITE,
                DrawTextureParams {
                    source: Some(source_rect),
                    dest_size: Some(Vec2::new(draw_w, draw_h)),
                    ..Default::default()
                },
            );
        }
        continue; // Skip arrow rendering for this projectile
    }

    // ... existing arrow rendering code below ...
```

**Step 2: Verify client compiles**

Run: `cd client && cargo check`

**Step 3: Commit**

```bash
git add client/src/render/renderer.rs
git commit -m "feat: render animated sprite for blast projectiles"
```

---

### Task 6: Client spawns projectile for blast spell + delays damage

**Files:**
- Modify: `client/src/network/message_handler.rs` (SpellEffect handler ~line 3860, damageEvent handler ~line 1041)

**Step 1: Handle blast SpellEffect — spawn projectile instead of on-target effect**

In the `"spellEffect"` handler (around line 3880), before pushing to `state.spell_effects`, add:

```rust
// For blast spell: spawn a projectile instead of on-target effect
if spell_id == "blast" {
    if let Some(ref cid) = Some(caster_id.clone()) {
        let source_pos = if let Some(player) = state.players.get(cid) {
            Some((player.x.round(), player.y.round(), player.z))
        } else {
            None
        };

        if let Some((src_x, src_y, src_z)) = source_pos {
            let end_z = state.chunk_manager.get_height(target_x, target_y) as f32;
            state.projectiles.push(crate::game::Projectile {
                sprite: "blast".to_string(),
                start_x: src_x,
                start_y: src_y,
                start_z: src_z,
                end_x: target_x as f32,
                end_y: target_y as f32,
                end_z,
                start_time: macroquad::time::get_time(),
                duration: 0.3, // Slower than arrows so you can see the orb
            });
        }
    }
    // Play cast animation but skip on-target effect
    if let Some(player) = state.players.get_mut(&caster_id) {
        player.play_cast();
    }
    // Don't push to spell_effects — no on-target animation for blast
    continue; // or return/skip the spell_effects.push below
}
```

Note: The exact control flow depends on the handler structure. The goal is: for "blast", push a Projectile and skip the SpellEffect push. For all other spells, keep existing behavior.

**Step 2: Skip DamageEvent projectile spawn for blast (it's handled by SpellEffect)**

In the `"damageEvent"` handler (line 1051), the existing code already spawns a projectile when `projectile` field is set. For blast, the SpellEffect handler already created the projectile, so we need to avoid double-spawning.

The simplest fix: In the damageEvent handler's projectile spawn section (line 1052), skip if projectile type is "blast":

```rust
if let Some(ref projectile_type) = projectile {
    if projectile_type != "blast" {
        // ... existing arrow projectile code ...
    }
}
```

**Step 3: Verify client compiles**

Run: `cd client && cargo check`

**Step 4: Commit**

```bash
git add client/src/network/message_handler.rs
git commit -m "feat: spawn blast projectile from SpellEffect, skip on-target animation"
```

---

### Task 7: Skip blast in spell effect renderer

**Files:**
- Modify: `client/src/render/renderer.rs:5751` (render_spell_effects sprite_name match)

**Step 1: Add blast to the skip list**

In `render_spell_effects` at the `match effect.spell_id.as_str()` block (line 5751), add:

```rust
"blast" => continue, // Projectile-based, no on-target effect
```

**Step 2: Also add to `render_single_spell_effect`**

At line 5843, same match block:

```rust
"blast" => return, // Projectile-based, no on-target effect
```

**Step 3: Verify client compiles**

Run: `cd client && cargo check`

**Step 4: Commit**

```bash
git add client/src/render/renderer.rs
git commit -m "feat: skip blast in on-target spell effect renderer"
```

---

### Task 8: Add blast spell icon for UI

**Files:**
- Create: `client/assets/ui/spells/blast.png` (32x32 spell icon — can reuse first frame of projectile.png scaled down, or create a placeholder)
- Modify: `client/src/render/renderer.rs:1682` (spell_icon_mappings)
- Modify: `client/assets/sprite_manifest.json` (spells_atlas section, if using atlas)

**Step 1: Create or copy a spell icon**

For now, extract the first frame (64x64) of projectile.png and scale to 32x32 for the spell icon. Or create a simple placeholder.

```bash
# Using ImageMagick if available, or just copy projectile.png as temporary icon
convert client/assets/sprites/effects/projectile.png -crop 64x64+0+0 -resize 32x32 client/assets/ui/spells/blast.png
```

If ImageMagick isn't available, we can use a Python script or just note this needs a manual icon.

**Step 2: Add to spell_icon_mappings**

In `client/src/render/renderer.rs` at the `spell_icon_mappings` array (line 1682), add:

```rust
("blast", "blast"),
```

**Step 3: Add to spells_atlas in manifest**

In `client/assets/sprite_manifest.json`, in the `spells_atlas.sprites` section, add an entry for blast. The exact x position depends on the current atlas layout — the last entry (tornado) is at x=224, w=32, so blast would go at x=256:

```json
"blast": {
    "x": 256,
    "y": 0,
    "w": 32,
    "h": 32
}
```

Note: The spells_atlas.png would need to be regenerated with the new icon packed in. For native builds this doesn't matter (individual files are loaded), but for WASM/Android it does. This can be done separately.

**Step 4: Verify client compiles**

Run: `cd client && cargo check`

**Step 5: Commit**

```bash
git add client/assets/ui/spells/blast.png client/src/render/renderer.rs client/assets/sprite_manifest.json
git commit -m "feat: add blast spell icon for UI"
```

---

### Task 9: Manual testing

**Test plan:**

1. Start the server: `cd rust-server && cargo run`
2. Start the client: `cd client && cargo run`
3. Create a character with magic level 1+
4. Target an attackable NPC
5. Cast "blast" from the spell book
6. Verify:
   - Cast animation plays on the player
   - Yellow glowing projectile travels from caster to target (~0.3s)
   - Projectile sprite animates (4 frames cycling)
   - Damage number appears at target when projectile arrives
   - No on-target spell effect animation plays (just the projectile + damage number)
   - Existing spells (Dark Hand, Lightning Bolt, etc.) still work with their on-target effects
   - Arrow projectiles for ranged attacks still render correctly
   - Spell cooldown (1s) works
   - Mana cost (2) is deducted
