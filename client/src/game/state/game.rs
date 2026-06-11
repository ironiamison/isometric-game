use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
}

pub struct GameState {
    // Connection
    pub connection_status: ConnectionStatus,
    pub local_player_id: Option<String>,
    pub selected_character_name: Option<String>,
    pub disconnect_requested: bool,
    pub reconnection_failed: bool,

    // World
    pub tilemap: Tilemap,
    pub chunk_manager: ChunkManager,
    pub players: HashMap<String, Player>,
    pub npcs: HashMap<String, Npc>,
    pub ground_items: HashMap<String, GroundItem>,
    /// Items waiting to spawn (with spawn time) - delays loot appearance until after death animation
    pub pending_ground_items: Vec<(GroundItem, f64)>,
    /// Farming patches received from server
    pub farming_patches: HashMap<String, FarmingPatch>,
    /// Farming patch lookup by position
    pub farming_patch_positions: HashMap<(i32, i32), String>,
    /// Which farming plots the player has unlocked
    pub unlocked_farming_plots: Vec<u32>,
    /// Active resource contract (if any)
    pub resource_contract: Option<ResourceContractInfo>,
    /// Ground tile overrides from server (farming plot tiles: locked=65, unlocked=62)
    pub ground_tile_overrides: HashMap<(i32, i32), u32>,
    /// Gathering marker positions received from server
    pub gathering_markers: Vec<GatheringMarker>,
    /// Lightweight static overworld atlas + POIs for the expanded world map.
    pub world_map_snapshot: Option<WorldMapSnapshot>,
    /// Whether the local player is currently gathering
    pub is_gathering: bool,
    /// Whether the local player is currently sitting on a chair
    pub is_sitting: bool,
    /// Chair positions on the map (received from server)
    pub chair_positions: Vec<(i32, i32)>,
    /// Chest positions on the map (received from server)
    pub chest_positions: Vec<(i32, i32)>,
    /// Pending chair to sit on after pathfinding completes
    pub pending_chair_sit: Option<(i32, i32)>,
    pub pending_harvest_patch: Option<String>,
    /// Timestamp when gathering started (for cast animation delay)
    pub gathering_started_at: f64,

    /// Active gathering buff on local player
    pub gathering_buff: Option<GatheringBuff>,
    /// Active potion buffs (attack/strength/defence boosts with timers)
    pub active_potion_buffs: Vec<ActivePotionBuff>,

    /// Depleted trees (position -> info for respawn timer)
    pub depleted_trees: HashMap<(i32, i32), DepletedTreeInfo>,
    /// Depleted rocks (position -> info for respawn timer)
    pub depleted_rocks: HashMap<(i32, i32), DepletedRockInfo>,
    /// Local dash cooldown tracking (game time when dash becomes available again)
    pub dash_cooldown_end: f64,
    /// Whether the local player is currently woodcutting
    pub is_woodcutting: bool,
    /// Timestamp when woodcutting started
    pub woodcutting_started_at: f64,
    /// Whether the local player is currently mining
    pub is_mining: bool,
    /// Timestamp when mining started
    pub mining_started_at: f64,
    /// Tree shake effects (when being chopped)
    pub tree_shake_effects: Vec<TreeShakeEffect>,
    /// Falling leaf particles
    pub leaf_particles: Vec<LeafParticle>,
    /// Trees falling down after being chopped
    pub falling_trees: Vec<FallingTreeEffect>,
    /// Rock shake effects (when being mined)
    pub rock_shake_effects: Vec<RockShakeEffect>,
    /// Rock debris particles
    pub rock_particles: Vec<RockParticle>,
    /// Animated click effects (walk/attack/interact indicators)
    pub click_effects: Vec<ClickEffect>,
    /// Bubble particles for fishing spot indicators
    pub fishing_bubbles: Vec<BubbleParticle>,
    /// Timer for spawning new bubbles at fishing markers
    pub bubble_spawn_timer: f64,
    /// Rocks crumbling after being fully mined
    pub crumbling_rocks: Vec<CrumblingRockEffect>,

    // Targeting
    pub selected_entity_id: Option<String>,

    // Combat feedback
    pub damage_events: Vec<DamageEvent>,
    pub level_up_events: Vec<LevelUpEvent>,
    /// Pending sound effects to play (queued by message handler, played by main loop)
    pub pending_sfx: Vec<String>,
    /// Pending music track change (queued by message handler, played by main loop)
    pub pending_music: Option<String>,
    /// Pending attack sounds queued by message handler
    pub pending_attack_sounds: Vec<AttackSoundType>,
    pub skill_xp_events: Vec<SkillXpEvent>,
    pub xp_globes: XpGlobesManager,
    pub xp_drop_feed: XpDropFeed,
    pub projectiles: Vec<Projectile>,

    // Chat bubbles above players
    pub chat_bubbles: Vec<ChatBubble>,

    // Inventory
    pub inventory: Inventory,

    // Item registry (loaded from server)
    pub item_registry: ItemRegistry,

    // Crafting
    pub recipe_definitions: Vec<RecipeDefinition>,
    pub discovered_recipes: HashSet<String>,

    // Camera and UI
    pub camera: Camera,
    pub ui_state: UiState,

    // Server tick (for ordering)
    pub server_tick: u64,
    /// Next client move sequence number (monotonic per session).
    pub next_move_seq: u32,
    /// Highest move sequence acknowledged by the server.
    pub last_acked_move_seq: u32,
    /// Unacked directional move commands awaiting server processing.
    pending_move_seqs: VecDeque<u32>,
    /// Adaptive movement mode for elevated latency.
    pub high_ping_movement_mode: bool,
    /// Number of server ticks we're catching up within the current frame.
    pub state_sync_catchup_ticks: u64,

    // Debug
    pub debug_mode: bool,
    /// Debug animation viewer: cycles through all animation states/frames
    /// None = off, Some((state_index, paused)) = active
    pub debug_anim_viewer: Option<(usize, bool)>,

    // Tile hover state (world coordinates of tile under mouse)
    pub hovered_tile: Option<(i32, i32)>,
    pub hovered_tile_z: i32,

    // Entity hover state (ID of entity under mouse cursor)
    pub hovered_entity_id: Option<String>,

    // Automated pathfinding state
    pub auto_path: Option<PathState>,

    /// Active auto-action state (OSRS-style click-to-act chase)
    pub auto_action_state: Option<AutoActionState>,

    /// Whether auto-retaliate is enabled (server-authoritative, toggled via character panel)
    pub auto_retaliate: bool,

    /// Timestamp of last chase re-path to throttle re-pathing frequency
    pub last_chase_repath_time: f64,

    /// Player ID we're following (right-click Follow)
    pub follow_target: Option<String>,
    /// Target's position when we arrived adjacent (waiting state)
    pub follow_arrived_target_pos: Option<(i32, i32)>,
    /// Timestamp when we first noticed the target moved from arrived position
    pub follow_target_move_time: f64,

    // Performance diagnostics (visible in debug mode)
    pub frame_timings: FrameTimings,

    // Map transition state
    pub map_transition: MapTransition,

    /// Current interior map ID if in an interior (None = overworld)
    pub current_interior: Option<String>,
    /// Current instance ID if in an instance
    pub current_instance: Option<String>,
    /// KOTH minigame state (active when in KOTH arena)
    pub koth: Option<KothClientState>,
    /// Boss fight state (active when in boss arena)
    pub boss: Option<BossClientState>,
    /// Active AOE warning zones
    pub aoe_warnings: Vec<AoeWarningZone>,
    /// Active explosion effects
    pub explosions: Vec<ExplosionEffect>,
    /// Whether the KOTH checkpoint dialog is open
    pub koth_checkpoint_open: bool,
    /// KOTH checkpoint info for dialog display
    pub koth_checkpoint_info: Option<KothCheckpointInfo>,
    /// Whether the KOTH game over screen is showing
    pub koth_game_over: Option<KothGameOverInfo>,
    /// Pending portal to enter (set when player walks onto a portal)
    pub pending_portal_id: Option<String>,
    /// Last tile position checked for portal (to avoid triggering on spawn)
    pub last_portal_check_pos: Option<(i32, i32)>,
    /// Portal tile to ignore until player steps off it (prevents flip-flop on transitions).
    /// Set to spawn tile after any map transition; cleared when player moves to a different tile.
    pub portal_ignore_tile: Option<(i32, i32)>,
    /// Area banner for displaying location names during transitions
    pub area_banner: AreaBanner,

    /// Social/Friends system state
    pub social_state: SocialState,

    // Spell system state
    /// Active spell effects for rendering
    pub spell_effects: Vec<SpellEffect>,
    /// Spell cooldowns tracked on client for UI feedback
    pub spell_cooldowns: std::collections::HashMap<String, f64>, // spell_id -> time when cooldown expires
    /// Scroll spell definitions received from server
    pub scroll_spell_definitions: Vec<crate::game::spell::ScrollSpellDef>,
    /// Set of spell IDs the player has unlocked via scroll items
    pub unlocked_spells: std::collections::HashSet<String>,

    // Prayer system state
    /// Current prayer points
    pub prayer_points: i32,
    /// Maximum prayer points (based on prayer level)
    pub max_prayer_points: i32,
    /// Currently active prayers (by prayer ID)
    pub active_prayers: Vec<String>,

    /// Timestamp when last ping was sent (for latency measurement)
    pub ping_sent_at: Option<f64>,
    /// Whether the pending ping was a manual /ping command
    pub manual_ping: bool,

    /// Continuous ping tracking (for debug menu)
    pub ping_stats: PingStats,

    /// Fade-in progress when world first becomes ready (1.0 = fully black, 0.0 = done)
    pub world_fade_in: f32,
    /// Whether the world has ever been ready (to trigger fade-in once)
    pub world_was_ready: bool,

    /// Tutorial state machine (None if tutorial not active)
    pub tutorial: Option<TutorialManager>,
    /// Flag set by the welcome message handler when is_new_character is true
    pub tutorial_pending: bool,

    /// Whether this GameState is in spectator mode (login screen world view)
    pub spectator_mode: bool,

    /// Name of the all-time highest total level player (gold trophy)
    pub top_level_player_name: Option<String>,
    /// Name of the 2nd highest total level player (silver trophy)
    pub second_level_player_name: Option<String>,
}

impl GameState {
    pub fn new() -> Self {
        // Create a test tilemap (32x32 tiles) - kept for compatibility
        let tilemap = Tilemap::new_test_map(32, 32);

        Self {
            connection_status: ConnectionStatus::Disconnected,
            local_player_id: None,
            selected_character_name: None,
            disconnect_requested: false,
            reconnection_failed: false,
            tilemap,
            chunk_manager: ChunkManager::new(),
            players: HashMap::new(),
            npcs: HashMap::new(),
            ground_items: HashMap::new(),
            pending_ground_items: Vec::new(),
            farming_patches: HashMap::new(),
            farming_patch_positions: HashMap::new(),
            unlocked_farming_plots: vec![1],
            resource_contract: None,
            ground_tile_overrides: HashMap::new(),
            gathering_markers: Vec::new(),
            world_map_snapshot: None,
            is_gathering: false,
            is_sitting: false,
            chair_positions: Vec::new(),
            chest_positions: Vec::new(),
            pending_chair_sit: None,
            pending_harvest_patch: None,
            gathering_started_at: 0.0,

            gathering_buff: None,
            active_potion_buffs: Vec::new(),
            dash_cooldown_end: 0.0,
            depleted_trees: HashMap::new(),
            depleted_rocks: HashMap::new(),
            is_woodcutting: false,
            woodcutting_started_at: 0.0,
            is_mining: false,
            mining_started_at: 0.0,
            tree_shake_effects: Vec::new(),
            leaf_particles: Vec::new(),
            falling_trees: Vec::new(),
            rock_shake_effects: Vec::new(),
            rock_particles: Vec::new(),
            click_effects: Vec::new(),
            fishing_bubbles: Vec::new(),
            bubble_spawn_timer: 0.0,
            crumbling_rocks: Vec::new(),
            selected_entity_id: None,
            damage_events: Vec::new(),
            level_up_events: Vec::new(),
            pending_sfx: Vec::new(),
            pending_music: None,
            pending_attack_sounds: Vec::new(),
            skill_xp_events: Vec::new(),
            xp_globes: XpGlobesManager::new(),
            xp_drop_feed: XpDropFeed::new(),
            projectiles: Vec::new(),
            chat_bubbles: Vec::new(),
            inventory: Inventory::new(),
            item_registry: ItemRegistry::new(),
            recipe_definitions: Vec::new(),
            discovered_recipes: HashSet::new(),
            camera: Camera::default(),
            ui_state: UiState::default(),
            server_tick: 0,
            next_move_seq: 0,
            last_acked_move_seq: 0,
            pending_move_seqs: VecDeque::new(),
            high_ping_movement_mode: false,
            state_sync_catchup_ticks: 0,
            debug_mode: false,
            debug_anim_viewer: None,
            hovered_tile: None,
            hovered_tile_z: 0,
            hovered_entity_id: None,
            auto_path: None,
            auto_action_state: None,
            auto_retaliate: true,
            last_chase_repath_time: 0.0,
            follow_target: None,
            follow_arrived_target_pos: None,
            follow_target_move_time: 0.0,
            frame_timings: FrameTimings::default(),
            map_transition: MapTransition::default(),
            current_interior: None,
            current_instance: None,
            koth: None,
            boss: None,
            aoe_warnings: Vec::new(),
            explosions: Vec::new(),
            koth_checkpoint_open: false,
            koth_checkpoint_info: None,
            koth_game_over: None,
            pending_portal_id: None,
            last_portal_check_pos: None,
            portal_ignore_tile: None,
            area_banner: AreaBanner::default(),
            social_state: SocialState::default(),
            spell_effects: Vec::new(),
            spell_cooldowns: std::collections::HashMap::new(),
            scroll_spell_definitions: Vec::new(),
            unlocked_spells: std::collections::HashSet::new(),
            prayer_points: 0,
            max_prayer_points: 1,
            active_prayers: Vec::new(),
            ping_sent_at: None,
            manual_ping: false,
            ping_stats: PingStats::default(),
            world_fade_in: 0.0,
            world_was_ready: false,
            tutorial: None,
            tutorial_pending: false,
            spectator_mode: false,
            top_level_player_name: None,
            second_level_player_name: None,
        }
    }

    /// Clear the current auto-path (e.g., when player presses movement keys)
    pub fn clear_auto_path(&mut self) {
        self.auto_path = None;
    }

    pub fn next_move_sequence(&mut self, dx: f32, dy: f32) -> u32 {
        self.next_move_seq = self.next_move_seq.wrapping_add(1);
        let seq = self.next_move_seq;

        if dx.abs() > 0.1 || dy.abs() > 0.1 {
            self.pending_move_seqs.push_back(seq);
            while self.pending_move_seqs.len() > 128 {
                self.pending_move_seqs.pop_front();
            }
        } else {
            // Stop command: clear all pending predictions immediately so
            // lookahead goes to 0 and the visual stops predicting ahead.
            self.pending_move_seqs.clear();
        }

        seq
    }

    pub fn acknowledge_move_sequence(&mut self, ack_seq: u32) {
        if ack_seq <= self.last_acked_move_seq {
            return;
        }
        self.last_acked_move_seq = ack_seq;
        while let Some(front) = self.pending_move_seqs.front().copied() {
            if front <= ack_seq {
                self.pending_move_seqs.pop_front();
            } else {
                break;
            }
        }
    }

    pub fn has_pending_move_sequences(&self) -> bool {
        !self.pending_move_seqs.is_empty()
    }

    pub fn reset_move_sequence_state(&mut self) {
        self.next_move_seq = 0;
        self.last_acked_move_seq = 0;
        self.pending_move_seqs.clear();
    }

    /// Clear pending move sequences without resetting the monotonic counter.
    /// Used on respawn where server seq state is preserved (unlike reconnect).
    pub fn clear_pending_moves(&mut self) {
        self.pending_move_seqs.clear();
    }

    /// Append a chat message and bump revision so renderer cache invalidates once.
    pub fn push_chat_message(&mut self, message: ChatMessage) {
        self.ui_state.chat_messages.push(message);
        self.ui_state.chat_revision = self.ui_state.chat_revision.wrapping_add(1);
    }

    /// Push a system chat message (convenience wrapper).
    pub fn push_system_chat(&mut self, text: String) {
        self.push_chat_message(ChatMessage::system(text));
    }

    /// Update all players in a server-authoritative step model.
    pub fn update(&mut self, delta: f32) {
        // Prune expired potion buffs
        let now = macroquad::time::get_time();
        self.active_potion_buffs.retain(|b| b.expires_at > now);

        // Trigger fade-in when world first becomes ready
        if !self.world_was_ready && self.is_world_ready() {
            self.world_was_ready = true;
            self.world_fade_in = 1.0;
        }

        // Tick down fade-in overlay
        if self.world_fade_in > 0.0 {
            self.world_fade_in = (self.world_fade_in - delta * 3.0).max(0.0); // ~0.33s fade
        }

        // Use smoothed delta for visual interpolation (reduces jitter from frame variance)
        let visual_delta = self.frame_timings.smoothed_delta;
        // Keep local movement tightly synced to real frame time to reduce
        // drift/corrections during rapid directional changes.
        let local_visual_delta = delta.clamp(1.0 / 240.0, 1.0 / 30.0);
        let local_id = self.local_player_id.clone();

        // Update all players (smooth interpolation toward server positions)
        // Note: woodcutting animations are now driven by server WoodcuttingSwing messages
        for (player_id, player) in self.players.iter_mut() {
            let step_delta = if local_id.as_ref() == Some(player_id) {
                local_visual_delta
            } else {
                visual_delta
            };
            player.interpolate_visual(step_delta);
        }

        // Update camera to follow local player
        if let Some(local_id) = &self.local_player_id {
            if let Some(player) = self.players.get(local_id) {
                if let Some((from_x, from_y)) = self.camera.transition_from {
                    // Only pan if within ~5 chunks (160 tiles) and in overworld
                    let dx = (player.x - from_x).abs();
                    let dy = (player.y - from_y).abs();
                    if dx > 160.0 || dy > 160.0 || self.current_instance.is_some() {
                        self.camera.transition_from = None;
                        self.camera.x = player.x;
                        self.camera.y = player.y;
                    } else {
                        // Smooth transition from spectator position to player
                        let dt = macroquad::time::get_frame_time();
                        self.camera.transition_progress += dt * 1.5; // ~0.67 seconds
                        if self.camera.transition_progress >= 1.0 {
                            self.camera.transition_from = None;
                            self.camera.x = player.x;
                            self.camera.y = player.y;
                        } else {
                            let t = smooth_step(self.camera.transition_progress);
                            self.camera.x = from_x + (player.x - from_x) * t;
                            self.camera.y = from_y + (player.y - from_y) * t;
                        }
                    }
                } else {
                    self.camera.x = player.x;
                    self.camera.y = player.y;
                }
                self.camera.z = player.z;
                self.camera.initialized = true;
            }
        }

        // Update NPCs (interpolation toward server positions)
        for npc in self.npcs.values_mut() {
            npc.update(visual_delta);
        }

        // Process pending ground items (spawn them after delay)
        let current_time = macroquad::time::get_time();
        let mut i = 0;
        while i < self.pending_ground_items.len() {
            if current_time >= self.pending_ground_items[i].1 {
                let (item, _) = self.pending_ground_items.swap_remove(i);
                self.ground_items.insert(item.id.clone(), item);
            } else {
                i += 1;
            }
        }

        // Clean up old damage events (older than 1.2 seconds)
        self.damage_events
            .retain(|event| current_time - event.time < 1.2);

        // Clean up old level up events (older than 2.0 seconds)
        self.level_up_events
            .retain(|event| current_time - event.time < 1.2);

        // Update tree effects
        self.tree_shake_effects.retain(|e| !e.is_finished());
        self.falling_trees.retain(|e| !e.is_finished());

        // Update leaf particles
        for leaf in &mut self.leaf_particles {
            leaf.update(delta);
        }
        self.leaf_particles.retain(|p| !p.is_finished());

        // Update rock effects
        self.rock_shake_effects.retain(|e| !e.is_finished());
        self.crumbling_rocks.retain(|e| !e.is_finished());
        for particle in &mut self.rock_particles {
            particle.update(delta);
        }
        self.rock_particles.retain(|p| !p.is_finished());

        // Update click effects
        for effect in &mut self.click_effects {
            effect.update(delta);
        }
        self.click_effects.retain(|e| !e.is_finished());

        // Update fishing bubble particles
        for bubble in &mut self.fishing_bubbles {
            bubble.update(delta, now);
        }
        self.fishing_bubbles.retain(|p| !p.is_finished(now));

        // Spawn new bubbles at fishing markers periodically (overworld only)
        self.bubble_spawn_timer -= delta as f64;
        if self.bubble_spawn_timer <= 0.0 && self.current_instance.is_some() {
            self.bubble_spawn_timer = 0.8;
            self.fishing_bubbles.clear();
        }
        if self.bubble_spawn_timer <= 0.0 && self.current_instance.is_none() {
            self.bubble_spawn_timer = 0.8; // Spawn a batch every 0.8s
            let fishing_markers: Vec<(f32, f32)> = self
                .gathering_markers
                .iter()
                .filter(|m| m.skill == "fishing")
                .map(|m| (m.x as f32, m.y as f32))
                .collect();
            for (mx, my) in &fishing_markers {
                // ~20% chance per marker per batch for subtle randomness
                if macroquad::rand::gen_range(0u32, 100) < 20 {
                    self.fishing_bubbles.push(BubbleParticle::new(*mx, *my));
                }
            }
            // Cap total bubbles to avoid runaway with many markers
            if self.fishing_bubbles.len() > 80 {
                self.fishing_bubbles
                    .drain(0..self.fishing_bubbles.len() - 80);
            }
        }

        // Clean up old skill XP events (older than 1.5 seconds)
        self.skill_xp_events
            .retain(|event| current_time - event.time < 1.5);

        // Update and clean up XP drops
        self.xp_drop_feed.update(delta);
        self.xp_drop_feed
            .drops
            .retain(|drop| current_time - drop.time < 2.0);

        // Clean up old chat bubbles (older than 5.0 seconds)
        self.chat_bubbles
            .retain(|bubble| current_time - bubble.time < 5.0);

        // Clean up expired NPC speech bubbles
        for npc in self.npcs.values_mut() {
            if let Some((_, time)) = &npc.speech_bubble {
                if current_time - time > 5.0 {
                    npc.speech_bubble = None;
                }
            }
        }

        // Clean up completed projectiles
        self.projectiles.retain(|p| !p.is_complete(current_time));

        // Clean up finished spell effects (max 3 seconds as fallback)
        self.spell_effects.retain(|effect| {
            let elapsed = current_time - effect.time;
            elapsed < 3.0
        });

        // Clean up expired AOE warnings
        self.aoe_warnings.retain(|w| {
            let elapsed = (current_time - w.created_at) * 1000.0;
            elapsed < (w.delay_ms as f64 + 500.0)
        });

        // Clean up expired explosions
        self.explosions
            .retain(|e| current_time - e.created_at < 1.0);

        // Clean up old quest completion events (older than 4 seconds)
        self.ui_state
            .quest_completed_events
            .retain(|event| current_time - event.time < 4.0);

        // Clean up old announcements (older than 8 seconds)
        self.ui_state
            .announcements
            .retain(|ann| current_time - ann.time < 8.0);

        // Update crafting progress (Task 14)
        if self.ui_state.crafting_in_progress {
            if let Some(started) = self.ui_state.crafting_started_at {
                let elapsed = ((macroquad::time::get_time() - started) * 1000.0) as f32;
                let duration = self.ui_state.crafting_duration_ms as f32;
                if duration > 0.0 {
                    self.ui_state.crafting_progress = (elapsed / duration).min(1.0);
                }
            }
        }

        // Update crafting completion animation timer (Task 20)
        if let Some((_, ref mut timer)) = self.ui_state.crafting_complete_animation {
            *timer += delta; // ~1 second animation
            if *timer >= 1.0 {
                // Animation done
            }
        }
        if self
            .ui_state
            .crafting_complete_animation
            .as_ref()
            .map_or(false, |(_, t)| *t >= 1.0)
        {
            self.ui_state.crafting_complete_animation = None;
        }

        // Auto-close furnace when player moves too far away
        if self.ui_state.furnace_open {
            if let Some((fx, fy)) = self.ui_state.furnace_tile {
                if let Some(player) = self.get_local_player() {
                    let px = player.x.round() as i32;
                    let py = player.y.round() as i32;
                    let dx = (px - fx).abs();
                    let dy = (py - fy).abs();
                    if dx > 3 || dy > 3 {
                        self.ui_state.furnace_open = false;
                        self.ui_state.furnace_tile = None;
                    }
                }
            }
        }

        // Auto-close anvil when player moves too far away
        if self.ui_state.anvil_open {
            if let Some((ax, ay)) = self.ui_state.anvil_tile {
                if let Some(player) = self.get_local_player() {
                    let px = player.x.round() as i32;
                    let py = player.y.round() as i32;
                    let dx = (px - ax).abs();
                    let dy = (py - ay).abs();
                    if dx > 3 || dy > 3 {
                        self.ui_state.anvil_open = false;
                        self.ui_state.anvil_tile = None;
                    }
                }
            }
        }

        // Auto-close alchemy station when player moves too far away
        if self.ui_state.alchemy_station_open {
            if let Some((ax, ay)) = self.ui_state.alchemy_station_tile {
                if let Some(player) = self.get_local_player() {
                    let px = player.x.round() as i32;
                    let py = player.y.round() as i32;
                    let dx = (px - ax).abs();
                    let dy = (py - ay).abs();
                    if dx > 3 || dy > 3 {
                        self.ui_state.alchemy_station_open = false;
                        self.ui_state.alchemy_station_tile = None;
                    }
                }
            }
        }

        // Auto-close workbench when player moves too far away
        if self.ui_state.workbench_open {
            if let Some((wx, wy)) = self.ui_state.workbench_tile {
                if let Some(player) = self.get_local_player() {
                    let px = player.x.round() as i32;
                    let py = player.y.round() as i32;
                    let dx = (px - wx).abs();
                    let dy = (py - wy).abs();
                    if dx > 3 || dy > 3 {
                        self.ui_state.workbench_open = false;
                        self.ui_state.workbench_tile = None;
                    }
                }
            }
        }

        // Update area banner timer
        self.area_banner.update(delta);

        // Update XP globes (fade timers, hover detection)
        // Calculate globe position to match renderer
        let margin = 12.0;
        let base_y = 25.0;
        let tag_height = 22.0;
        let bar_width = 120.0_f32.max(140.0);
        let (vw, _) = crate::util::virtual_screen_size();
        let bar_x = (vw - bar_width - margin).floor();
        let globe_stats_y = base_y + tag_height / 2.0 + 8.0;
        self.xp_globes.update(bar_x, globe_stats_y);
    }

    pub fn get_local_player(&self) -> Option<&Player> {
        self.local_player_id
            .as_ref()
            .and_then(|id| self.players.get(id))
    }

    /// Get recipes filtered by the current shop's crafting categories and stations.
    /// Returns all recipes if no shop is open.
    pub fn shop_filtered_recipes(&self) -> Vec<RecipeDefinition> {
        if let Some(ref shop) = self.ui_state.shop_data {
            if shop.crafting_categories.is_empty() {
                Vec::new()
            } else {
                self.recipe_definitions
                    .iter()
                    .filter(|r| shop.crafting_categories.contains(&r.category))
                    .filter(|r| {
                        if shop.crafting_stations.is_empty() {
                            true
                        } else if let Some(ref station) = r.station {
                            shop.crafting_stations.contains(station)
                        } else {
                            true
                        }
                    })
                    .cloned()
                    .collect()
            }
        } else {
            self.recipe_definitions.clone()
        }
    }

    /// Returns true when the world is ready to render (player exists and their chunk is loaded)
    pub fn is_world_ready(&self) -> bool {
        if self.spectator_mode {
            // In spectator mode, check if spawn chunk is loaded (no local player)
            let spawn_chunk = crate::game::chunk::ChunkCoord::from_world(15, 4);
            return self.chunk_manager.chunks().contains_key(&spawn_chunk);
        }
        if let Some(player) = self.get_local_player() {
            if self.current_instance.is_some() {
                // Interiors are loaded as a single chunk at (0,0) regardless of player position
                let origin = crate::game::chunk::ChunkCoord { x: 0, y: 0 };
                self.chunk_manager.chunks().contains_key(&origin)
            } else {
                let chunk_coord =
                    crate::game::chunk::ChunkCoord::from_world_f32(player.x, player.y);
                self.chunk_manager.chunks().contains_key(&chunk_coord)
            }
        } else {
            false
        }
    }

    /// Update map transition animation
    pub fn update_transition(&mut self, delta: f32) {
        const FADE_DURATION: f32 = 0.25;

        match self.map_transition.state {
            TransitionState::FadingOut => {
                self.map_transition.progress += delta / FADE_DURATION;
                if self.map_transition.progress >= 1.0 {
                    self.map_transition.progress = 1.0;
                    self.map_transition.state = TransitionState::Loading;
                }
            }
            TransitionState::FadingIn => {
                self.map_transition.progress -= delta / FADE_DURATION;
                if self.map_transition.progress <= 0.0 {
                    self.map_transition.progress = 0.0;
                    self.map_transition.state = TransitionState::None;
                }
            }
            _ => {}
        }
    }

    /// Start a map transition
    pub fn start_transition(
        &mut self,
        map_type: String,
        map_id: String,
        spawn_x: f32,
        spawn_y: f32,
        instance_id: String,
    ) {
        self.map_transition = MapTransition {
            state: TransitionState::FadingOut,
            progress: 0.0,
            target_map_type: map_type,
            target_map_id: map_id,
            target_spawn_x: spawn_x,
            target_spawn_y: spawn_y,
            instance_id,
        };
    }

    /// Check if input should be blocked due to transition
    pub fn is_transitioning(&self) -> bool {
        self.map_transition.state != TransitionState::None
    }
}
