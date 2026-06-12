use super::*;

impl Renderer {
    /// Update the HTML loading overlay progress (WASM only, no-op on other platforms).
    #[cfg(target_arch = "wasm32")]
    pub(super) fn update_loading(loaded: usize, total: usize, label: &str) {
        use sapp_jsutils::JsObject;
        extern "C" {
            fn loading_set_progress(pct_times_100: i32);
            fn loading_set_label(label: JsObject);
            fn loading_hide();
        }
        let pct = if total > 0 {
            (loaded as f64 / total as f64 * 10000.0) as i32
        } else {
            0
        };
        unsafe {
            loading_set_progress(pct);
            loading_set_label(JsObject::string(label));
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) fn hide_loading() {
        extern "C" {
            fn loading_hide();
        }
        unsafe {
            loading_hide();
        }
    }

    pub async fn new(audio: &mut crate::audio::AudioManager) -> Self {
        // Load manifest first to compute total sprite count
        let manifest = SpriteManifest::load().await;

        // Fixed assets: 1 tileset + 14 players + 3 hair + 1 font + 8 UI textures + 1 shader + 4 arrows + 2 music = 34
        #[cfg(any(target_os = "android", target_arch = "wasm32"))]
        const FIXED_ASSETS: usize = 34;
        #[cfg(any(target_os = "android", target_arch = "wasm32"))]
        let manifest_total = manifest.equipment.len()
            + manifest.weapons.len()
            + manifest.inventory.len()
            + manifest.objects.len()
            + manifest.walls.len()
            + manifest.enemies.len();
        #[cfg(any(target_os = "android", target_arch = "wasm32"))]
        let total = FIXED_ASSETS + manifest_total;
        #[cfg(any(target_os = "android", target_arch = "wasm32"))]
        let mut loaded: usize = 0;

        // On WASM, update the HTML overlay. On other platforms, no-op.
        macro_rules! set_loading {
            ($label:expr) => {
                #[cfg(target_arch = "wasm32")]
                Self::update_loading(loaded, total, $label);
            };
        }

        // Preload audio first (music + SFX)
        set_loading!("Loading audio...");
        audio.preload_all().await;
        #[cfg(any(target_os = "android", target_arch = "wasm32"))]
        {
            loaded += 2; // menu.ogg + start.ogg
        }

        set_loading!("Loading tileset...");

        let tileset = match load_texture(&asset_path("assets/sprites/tiles.png")).await {
            Ok(tex) => {
                tex.set_filter(FilterMode::Nearest);
                Some(tex)
            }
            Err(e) => {
                log::warn!("Failed to load tileset: {}", e);
                None
            }
        };
        #[cfg(any(target_os = "android", target_arch = "wasm32"))]
        {
            loaded += 1;
        }

        // Load player sprites - atlas on WASM/Android, individual on desktop
        set_loading!("Loading player sprites...");
        #[cfg(any(target_os = "android", target_arch = "wasm32"))]
        let player_sprites: SpritesheetStore = if let Some(ref atlas_info) = manifest.players_atlas
        {
            if let Some((tex, rects)) = load_spritesheet_atlas(atlas_info).await {
                SpritesheetStore::Atlas {
                    texture: tex,
                    rects,
                }
            } else {
                let mut sprites = HashMap::new();
                for gender in GENDERS {
                    for skin in SKINS {
                        let key = format!("{}_{}", gender, skin);
                        let path = asset_path(&format!(
                            "assets/sprites/players/player_{}_{}.png",
                            gender, skin
                        ));
                        if let Ok(tex) = load_texture(&path).await {
                            tex.set_filter(FilterMode::Nearest);
                            sprites.insert(key, tex);
                        }
                    }
                }
                SpritesheetStore::Individual(sprites)
            }
        } else {
            let mut sprites = HashMap::new();
            for gender in GENDERS {
                for skin in SKINS {
                    let key = format!("{}_{}", gender, skin);
                    let path = asset_path(&format!(
                        "assets/sprites/players/player_{}_{}.png",
                        gender, skin
                    ));
                    if let Ok(tex) = load_texture(&path).await {
                        tex.set_filter(FilterMode::Nearest);
                        sprites.insert(key, tex);
                    }
                }
            }
            SpritesheetStore::Individual(sprites)
        };

        #[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
        let player_sprites: SpritesheetStore = if let Some(ref atlas_info) = manifest.players_atlas
        {
            if let Some((tex, rects)) = load_spritesheet_atlas(atlas_info).await {
                SpritesheetStore::Atlas {
                    texture: tex,
                    rects,
                }
            } else {
                let mut sprites = HashMap::new();
                for gender in GENDERS {
                    for skin in SKINS {
                        let key = format!("{}_{}", gender, skin);
                        let path = format!("assets/sprites/players/player_{}_{}.png", gender, skin);
                        if let Ok(tex) = load_texture(&path).await {
                            tex.set_filter(FilterMode::Nearest);
                            sprites.insert(key, tex);
                        }
                    }
                }
                SpritesheetStore::Individual(sprites)
            }
        } else {
            let mut sprites = HashMap::new();
            for gender in GENDERS {
                for skin in SKINS {
                    let key = format!("{}_{}", gender, skin);
                    let path = format!("assets/sprites/players/player_{}_{}.png", gender, skin);
                    if let Ok(tex) = load_texture(&path).await {
                        tex.set_filter(FilterMode::Nearest);
                        sprites.insert(key, tex);
                    }
                }
            }
            SpritesheetStore::Individual(sprites)
        };
        log::info!("Loaded {} player sprite variants", player_sprites.len());

        // Load hair sprites - atlas on WASM/Android, individual on desktop
        set_loading!("Loading hair sprites...");
        #[cfg(any(target_os = "android", target_arch = "wasm32"))]
        let hair_sprites: SpritesheetStore = if let Some(ref atlas_info) = manifest.hair_atlas {
            if let Some((tex, rects)) = load_spritesheet_atlas(atlas_info).await {
                SpritesheetStore::Atlas {
                    texture: tex,
                    rects,
                }
            } else {
                let mut sprites = HashMap::new();
                for style in 0..6 {
                    let path = asset_path(&format!("assets/sprites/hair/hair_{}.png", style));
                    if let Ok(tex) = load_texture(&path).await {
                        tex.set_filter(FilterMode::Nearest);
                        sprites.insert(format!("male_{}", style), tex);
                    }
                    let path =
                        asset_path(&format!("assets/sprites/hair/hair_female_{}.png", style));
                    if let Ok(tex) = load_texture(&path).await {
                        tex.set_filter(FilterMode::Nearest);
                        sprites.insert(format!("female_{}", style), tex);
                    }
                }
                SpritesheetStore::Individual(sprites)
            }
        } else {
            let mut sprites = HashMap::new();
            for style in 0..6 {
                let path = asset_path(&format!("assets/sprites/hair/hair_{}.png", style));
                if let Ok(tex) = load_texture(&path).await {
                    tex.set_filter(FilterMode::Nearest);
                    sprites.insert(format!("male_{}", style), tex);
                }
                let path = asset_path(&format!("assets/sprites/hair/hair_female_{}.png", style));
                if let Ok(tex) = load_texture(&path).await {
                    tex.set_filter(FilterMode::Nearest);
                    sprites.insert(format!("female_{}", style), tex);
                }
            }
            SpritesheetStore::Individual(sprites)
        };

        #[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
        let hair_sprites: SpritesheetStore = if let Some(ref atlas_info) = manifest.hair_atlas {
            if let Some((tex, rects)) = load_spritesheet_atlas(atlas_info).await {
                SpritesheetStore::Atlas {
                    texture: tex,
                    rects,
                }
            } else {
                let mut sprites = HashMap::new();
                for style in 0..6 {
                    let path = format!("assets/sprites/hair/hair_{}.png", style);
                    if let Ok(tex) = load_texture(&path).await {
                        tex.set_filter(FilterMode::Nearest);
                        sprites.insert(format!("male_{}", style), tex);
                    }
                    let path = format!("assets/sprites/hair/hair_female_{}.png", style);
                    if let Ok(tex) = load_texture(&path).await {
                        tex.set_filter(FilterMode::Nearest);
                        sprites.insert(format!("female_{}", style), tex);
                    }
                }
                SpritesheetStore::Individual(sprites)
            }
        } else {
            let mut sprites = HashMap::new();
            for style in 0..6 {
                let path = format!("assets/sprites/hair/hair_{}.png", style);
                if let Ok(tex) = load_texture(&path).await {
                    tex.set_filter(FilterMode::Nearest);
                    sprites.insert(format!("male_{}", style), tex);
                }
                let path = format!("assets/sprites/hair/hair_female_{}.png", style);
                if let Ok(tex) = load_texture(&path).await {
                    tex.set_filter(FilterMode::Nearest);
                    sprites.insert(format!("female_{}", style), tex);
                }
            }
            SpritesheetStore::Individual(sprites)
        };
        log::info!("Loaded {} hair sprite variants", hair_sprites.len());

        // Helper to load an atlas texture and build a SpriteStore

        // Helper to load a spritesheet atlas (for animation spritesheets)

        // Load individual sprites into a HashMap (for non-atlas categories)

        // On WASM/Android, load sprite categories - use atlases when available.
        // On desktop, prefer atlases when available and fall back to directory scanning.
        #[cfg(any(target_os = "android", target_arch = "wasm32"))]
        let (
            equipment_sprites,
            weapon_sprites,
            weapon_frame_sizes,
            item_sprites,
            object_sprites,
            wall_sprites,
            npc_sprites,
            npc_overflow_sprites,
            farming_sprites,
            spell_effect_textures,
        ) = {
            // Load equipment - atlas if available
            set_loading!("Loading equipment...");
            let equipment: SpritesheetStore = if let Some(ref atlas_info) = manifest.equipment_atlas
            {
                if let Some((tex, rects)) = load_spritesheet_atlas(atlas_info).await {
                    loaded += manifest.equipment.len();
                    #[cfg(target_arch = "wasm32")]
                    Self::update_loading(loaded, total, "Loading equipment...");
                    SpritesheetStore::Atlas {
                        texture: tex,
                        rects,
                    }
                } else {
                    SpritesheetStore::Individual(
                        load_individual_sprites(
                            &manifest.equipment,
                            "assets/sprites",
                            &mut loaded,
                            total,
                            "Loading equipment...",
                        )
                        .await,
                    )
                }
            } else {
                SpritesheetStore::Individual(
                    load_individual_sprites(
                        &manifest.equipment,
                        "assets/sprites",
                        &mut loaded,
                        total,
                        "Loading equipment...",
                    )
                    .await,
                )
            };

            // Load weapons - atlas if available
            set_loading!("Loading weapons...");
            let weapons: SpritesheetStore = if let Some(ref atlas_info) = manifest.weapons_atlas {
                if let Some((tex, rects)) = load_spritesheet_atlas(atlas_info).await {
                    loaded += manifest.weapons.len();
                    #[cfg(target_arch = "wasm32")]
                    Self::update_loading(loaded, total, "Loading weapons...");
                    SpritesheetStore::Atlas {
                        texture: tex,
                        rects,
                    }
                } else {
                    SpritesheetStore::Individual(
                        load_individual_sprites(
                            &manifest.weapons,
                            "assets/sprites/weapons",
                            &mut loaded,
                            total,
                            "Loading weapons...",
                        )
                        .await,
                    )
                }
            } else {
                SpritesheetStore::Individual(
                    load_individual_sprites(
                        &manifest.weapons,
                        "assets/sprites/weapons",
                        &mut loaded,
                        total,
                        "Loading weapons...",
                    )
                    .await,
                )
            };
            // Build weapon frame sizes map
            let wf_sizes: HashMap<String, (f32, f32)> = manifest
                .weapon_frame_sizes
                .iter()
                .map(|(k, v)| (k.clone(), (v[0], v[1])))
                .collect();

            // Load items - atlas if available
            set_loading!("Loading items...");
            let items: SpriteStore = if let Some(ref atlas_info) = manifest.inventory_atlas {
                if let Some(atlas) = load_atlas(atlas_info).await {
                    loaded += manifest.inventory.len();
                    #[cfg(target_arch = "wasm32")]
                    Self::update_loading(loaded, total, "Loading items...");
                    SpriteStore::Atlas(atlas)
                } else {
                    SpriteStore::Individual(
                        load_individual_sprites(
                            &manifest.inventory,
                            "assets/sprites/inventory",
                            &mut loaded,
                            total,
                            "Loading items...",
                        )
                        .await,
                    )
                }
            } else {
                SpriteStore::Individual(
                    load_individual_sprites(
                        &manifest.inventory,
                        "assets/sprites/inventory",
                        &mut loaded,
                        total,
                        "Loading items...",
                    )
                    .await,
                )
            };

            // Load objects - atlas if available
            set_loading!("Loading objects...");
            let objects: SpriteStore = if let Some(ref atlas_info) = manifest.objects_atlas {
                if let Some(atlas) = load_atlas(atlas_info).await {
                    loaded += manifest.objects.len();
                    #[cfg(target_arch = "wasm32")]
                    Self::update_loading(loaded, total, "Loading objects...");
                    SpriteStore::Atlas(atlas)
                } else {
                    SpriteStore::Individual(
                        load_individual_sprites(
                            &manifest.objects,
                            "assets/sprites/objects",
                            &mut loaded,
                            total,
                            "Loading objects...",
                        )
                        .await,
                    )
                }
            } else {
                SpriteStore::Individual(
                    load_individual_sprites(
                        &manifest.objects,
                        "assets/sprites/objects",
                        &mut loaded,
                        total,
                        "Loading objects...",
                    )
                    .await,
                )
            };

            // Load walls - atlas if available
            set_loading!("Loading walls...");
            let walls: SpriteStore = if let Some(ref atlas_info) = manifest.walls_atlas {
                if let Some(atlas) = load_atlas(atlas_info).await {
                    loaded += manifest.walls.len();
                    #[cfg(target_arch = "wasm32")]
                    Self::update_loading(loaded, total, "Loading walls...");
                    SpriteStore::Atlas(atlas)
                } else {
                    SpriteStore::Individual(
                        load_individual_sprites(
                            &manifest.walls,
                            "assets/sprites/walls",
                            &mut loaded,
                            total,
                            "Loading walls...",
                        )
                        .await,
                    )
                }
            } else {
                SpriteStore::Individual(
                    load_individual_sprites(
                        &manifest.walls,
                        "assets/sprites/walls",
                        &mut loaded,
                        total,
                        "Loading walls...",
                    )
                    .await,
                )
            };

            // Load NPCs/enemies - atlas if available
            set_loading!("Loading NPCs...");
            let (npcs, npc_overflow): (SpritesheetStore, HashMap<String, Texture2D>) =
                if let Some(ref atlas_info) = manifest.enemies_atlas {
                    if let Some((tex, rects)) = load_spritesheet_atlas(atlas_info).await {
                        loaded += manifest.enemies.len();
                        #[cfg(target_arch = "wasm32")]
                        Self::update_loading(loaded, total, "Loading NPCs...");
                        // Load any sprites from manifest that didn't fit in the atlas
                        let mut overflow = HashMap::new();
                        for name in &manifest.enemies {
                            if !rects.contains_key(name.as_str()) {
                                let path =
                                    asset_path(&format!("assets/sprites/enemies/{}.png", name));
                                if let Ok(tex) = load_texture(&path).await {
                                    tex.set_filter(FilterMode::Nearest);
                                    overflow.insert(name.clone(), tex);
                                }
                            }
                        }
                        (
                            SpritesheetStore::Atlas {
                                texture: tex,
                                rects,
                            },
                            overflow,
                        )
                    } else {
                        (
                            SpritesheetStore::Individual(
                                load_individual_sprites(
                                    &manifest.enemies,
                                    "assets/sprites/enemies",
                                    &mut loaded,
                                    total,
                                    "Loading NPCs...",
                                )
                                .await,
                            ),
                            HashMap::new(),
                        )
                    }
                } else {
                    (
                        SpritesheetStore::Individual(
                            load_individual_sprites(
                                &manifest.enemies,
                                "assets/sprites/enemies",
                                &mut loaded,
                                total,
                                "Loading NPCs...",
                            )
                            .await,
                        ),
                        HashMap::new(),
                    )
                };

            // Load farming sprites - atlas if available
            set_loading!("Loading farming...");
            let farming: SpritesheetStore = if let Some(ref atlas_info) = manifest.farming_atlas {
                if let Some((tex, rects)) = load_spritesheet_atlas(atlas_info).await {
                    SpritesheetStore::Atlas {
                        texture: tex,
                        rects,
                    }
                } else {
                    let crop_names = [
                        "potato",
                        "onion",
                        "tomato",
                        "cabbage",
                        "strawberry",
                        "sweetcorn",
                        "wheat",
                        "carrot",
                        "spinach",
                        "greenleaf",
                        "ashveil",
                        "bloodcap",
                        "marshbloom",
                        "nightthorn",
                        "tangleroot",
                        "cactus",
                    ];
                    let mut sprites = HashMap::new();
                    for crop in &crop_names {
                        let path =
                            asset_path(&format!("assets/sprites/farming/farming_{}.png", crop));
                        if let Ok(tex) = load_texture(&path).await {
                            tex.set_filter(FilterMode::Nearest);
                            sprites.insert(crop.to_string(), tex);
                        }
                    }
                    SpritesheetStore::Individual(sprites)
                }
            } else {
                let crop_names = [
                    "potato",
                    "onion",
                    "tomato",
                    "cabbage",
                    "strawberry",
                    "sweetcorn",
                    "wheat",
                    "carrot",
                    "spinach",
                    "greenleaf",
                    "ashveil",
                    "bloodcap",
                    "marshbloom",
                    "nightthorn",
                    "tangleroot",
                    "cactus",
                ];
                let mut sprites = HashMap::new();
                for crop in &crop_names {
                    let path = asset_path(&format!("assets/sprites/farming/farming_{}.png", crop));
                    if let Ok(tex) = load_texture(&path).await {
                        tex.set_filter(FilterMode::Nearest);
                        sprites.insert(crop.to_string(), tex);
                    }
                }
                SpritesheetStore::Individual(sprites)
            };

            // Load spell effects - atlas if available
            set_loading!("Loading effects...");
            let effects: SpritesheetStore = if let Some(ref atlas_info) = manifest.effects_atlas {
                if let Some((tex, rects)) = load_spritesheet_atlas(atlas_info).await {
                    SpritesheetStore::Atlas {
                        texture: tex,
                        rects,
                    }
                } else {
                    let mut sprites = HashMap::new();
                    for name in &[
                        "dark_hand",
                        "lightning_bolt",
                        "dark_eater",
                        "rock_fall",
                        "self_heal",
                        "bubbles_warp",
                        "tornado",
                        "rocks_aoe",
                        "air_blast",
                        "water_blast",
                        "earth_blast",
                        "fire_blast",
                    ] {
                        let path = asset_path(&format!("assets/sprites/effects/{}.png", name));
                        if let Ok(tex) = load_texture(&path).await {
                            tex.set_filter(FilterMode::Nearest);
                            sprites.insert(name.to_string(), tex);
                        }
                    }
                    SpritesheetStore::Individual(sprites)
                }
            } else {
                let mut sprites = HashMap::new();
                for name in &[
                    "dark_hand",
                    "lightning_bolt",
                    "dark_eater",
                    "rock_fall",
                    "self_heal",
                    "bubbles_warp",
                    "tornado",
                    "rocks_aoe",
                    "air_blast",
                    "water_blast",
                    "earth_blast",
                    "fire_blast",
                ] {
                    let path = asset_path(&format!("assets/sprites/effects/{}.png", name));
                    if let Ok(tex) = load_texture(&path).await {
                        tex.set_filter(FilterMode::Nearest);
                        sprites.insert(name.to_string(), tex);
                    }
                }
                SpritesheetStore::Individual(sprites)
            };

            (
                equipment,
                weapons,
                wf_sizes,
                items,
                objects,
                walls,
                npcs,
                npc_overflow,
                farming,
                effects,
            )
        };

        #[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
        let (
            equipment_sprites,
            weapon_sprites,
            weapon_frame_sizes,
            item_sprites,
            object_sprites,
            wall_sprites,
            npc_sprites,
            npc_overflow_sprites,
            farming_sprites,
            spell_effect_textures,
        ) = {
            use crate::util::load_sprites_from_dir_or_manifest;

            let equipment = if let Some(ref atlas_info) = manifest.equipment_atlas {
                if let Some((tex, rects)) = load_spritesheet_atlas(atlas_info).await {
                    SpritesheetStore::Atlas {
                        texture: tex,
                        rects,
                    }
                } else {
                    SpritesheetStore::Individual(
                        load_sprites_from_dir_or_manifest(
                            "assets/sprites/equipment",
                            &manifest.equipment,
                            "assets/sprites",
                        )
                        .await,
                    )
                }
            } else {
                SpritesheetStore::Individual(
                    load_sprites_from_dir_or_manifest(
                        "assets/sprites/equipment",
                        &manifest.equipment,
                        "assets/sprites",
                    )
                    .await,
                )
            };

            let weapons = if let Some(ref atlas_info) = manifest.weapons_atlas {
                if let Some((tex, rects)) = load_spritesheet_atlas(atlas_info).await {
                    SpritesheetStore::Atlas {
                        texture: tex,
                        rects,
                    }
                } else {
                    SpritesheetStore::Individual(
                        load_sprites_from_dir_or_manifest(
                            "assets/sprites/weapons",
                            &manifest.weapons,
                            "assets/sprites/weapons",
                        )
                        .await,
                    )
                }
            } else {
                SpritesheetStore::Individual(
                    load_sprites_from_dir_or_manifest(
                        "assets/sprites/weapons",
                        &manifest.weapons,
                        "assets/sprites/weapons",
                    )
                    .await,
                )
            };
            // Build weapon frame sizes map
            let wf_sizes: HashMap<String, (f32, f32)> = manifest
                .weapon_frame_sizes
                .iter()
                .map(|(k, v)| (k.clone(), (v[0], v[1])))
                .collect();

            let items = if let Some(ref atlas_info) = manifest.inventory_atlas {
                if let Some(atlas) = load_atlas(atlas_info).await {
                    SpriteStore::Atlas(atlas)
                } else {
                    SpriteStore::Individual(
                        load_sprites_from_dir_or_manifest(
                            "assets/sprites/inventory",
                            &manifest.inventory,
                            "assets/sprites/inventory",
                        )
                        .await,
                    )
                }
            } else {
                SpriteStore::Individual(
                    load_sprites_from_dir_or_manifest(
                        "assets/sprites/inventory",
                        &manifest.inventory,
                        "assets/sprites/inventory",
                    )
                    .await,
                )
            };

            let objects = if let Some(ref atlas_info) = manifest.objects_atlas {
                if let Some(atlas) = load_atlas(atlas_info).await {
                    SpriteStore::Atlas(atlas)
                } else {
                    SpriteStore::Individual(
                        load_sprites_from_dir_or_manifest(
                            "assets/sprites/objects",
                            &manifest.objects,
                            "assets/sprites/objects",
                        )
                        .await,
                    )
                }
            } else {
                SpriteStore::Individual(
                    load_sprites_from_dir_or_manifest(
                        "assets/sprites/objects",
                        &manifest.objects,
                        "assets/sprites/objects",
                    )
                    .await,
                )
            };

            let walls = if let Some(ref atlas_info) = manifest.walls_atlas {
                if let Some(atlas) = load_atlas(atlas_info).await {
                    SpriteStore::Atlas(atlas)
                } else {
                    SpriteStore::Individual(
                        load_sprites_from_dir_or_manifest(
                            "assets/sprites/walls",
                            &manifest.walls,
                            "assets/sprites/walls",
                        )
                        .await,
                    )
                }
            } else {
                SpriteStore::Individual(
                    load_sprites_from_dir_or_manifest(
                        "assets/sprites/walls",
                        &manifest.walls,
                        "assets/sprites/walls",
                    )
                    .await,
                )
            };

            let (npcs, npc_overflow) = if let Some(ref atlas_info) = manifest.enemies_atlas {
                if let Some((tex, rects)) = load_spritesheet_atlas(atlas_info).await {
                    // Load any sprites from manifest that didn't fit in the atlas
                    let mut overflow = HashMap::new();
                    for name in &manifest.enemies {
                        if !rects.contains_key(name.as_str()) {
                            let path = format!("assets/sprites/enemies/{}.png", name);
                            if let Ok(tex) = load_texture(&path).await {
                                tex.set_filter(FilterMode::Nearest);
                                overflow.insert(name.clone(), tex);
                            }
                        }
                    }
                    (
                        SpritesheetStore::Atlas {
                            texture: tex,
                            rects,
                        },
                        overflow,
                    )
                } else {
                    (
                        SpritesheetStore::Individual(
                            load_sprites_from_dir_or_manifest(
                                "assets/sprites/enemies",
                                &manifest.enemies,
                                "assets/sprites/enemies",
                            )
                            .await,
                        ),
                        HashMap::new(),
                    )
                }
            } else {
                (
                    SpritesheetStore::Individual(
                        load_sprites_from_dir_or_manifest(
                            "assets/sprites/enemies",
                            &manifest.enemies,
                            "assets/sprites/enemies",
                        )
                        .await,
                    ),
                    HashMap::new(),
                )
            };

            let farming = if let Some(ref atlas_info) = manifest.farming_atlas {
                if let Some((tex, rects)) = load_spritesheet_atlas(atlas_info).await {
                    SpritesheetStore::Atlas {
                        texture: tex,
                        rects,
                    }
                } else {
                    let crop_names = [
                        "potato",
                        "onion",
                        "tomato",
                        "cabbage",
                        "strawberry",
                        "sweetcorn",
                        "wheat",
                        "carrot",
                        "spinach",
                        "greenleaf",
                        "ashveil",
                        "bloodcap",
                        "marshbloom",
                        "nightthorn",
                        "tangleroot",
                        "cactus",
                    ];
                    let mut sprites = HashMap::new();
                    for crop in &crop_names {
                        let path = format!("assets/sprites/farming/farming_{}.png", crop);
                        if let Ok(tex) = load_texture(&path).await {
                            tex.set_filter(FilterMode::Nearest);
                            sprites.insert(crop.to_string(), tex);
                        }
                    }
                    SpritesheetStore::Individual(sprites)
                }
            } else {
                let crop_names = [
                    "potato",
                    "onion",
                    "tomato",
                    "cabbage",
                    "strawberry",
                    "sweetcorn",
                    "wheat",
                    "carrot",
                    "spinach",
                    "greenleaf",
                    "ashveil",
                    "bloodcap",
                    "marshbloom",
                    "nightthorn",
                    "tangleroot",
                    "cactus",
                ];
                let mut sprites = HashMap::new();
                for crop in &crop_names {
                    let path = format!("assets/sprites/farming/farming_{}.png", crop);
                    if let Ok(tex) = load_texture(&path).await {
                        tex.set_filter(FilterMode::Nearest);
                        sprites.insert(crop.to_string(), tex);
                    }
                }
                SpritesheetStore::Individual(sprites)
            };

            let effects = if let Some(ref atlas_info) = manifest.effects_atlas {
                if let Some((tex, rects)) = load_spritesheet_atlas(atlas_info).await {
                    SpritesheetStore::Atlas {
                        texture: tex,
                        rects,
                    }
                } else {
                    SpritesheetStore::Individual(
                        load_sprites_from_dir_or_manifest(
                            "assets/sprites/effects",
                            &[],
                            "assets/sprites/effects",
                        )
                        .await,
                    )
                }
            } else {
                SpritesheetStore::Individual(
                    load_sprites_from_dir_or_manifest(
                        "assets/sprites/effects",
                        &[],
                        "assets/sprites/effects",
                    )
                    .await,
                )
            };

            (
                equipment,
                weapons,
                wf_sizes,
                items,
                objects,
                walls,
                npcs,
                npc_overflow,
                farming,
                effects,
            )
        };

        set_loading!("Loading fonts...");

        let font =
            BitmapFont::load_or_default("assets/fonts/monogram/ttf/monogram-extended.ttf").await;
        #[cfg(any(target_os = "android", target_arch = "wasm32"))]
        {
            loaded += 1;
        }

        set_loading!("Loading UI...");

        // Load quest complete banner texture
        let quest_complete_texture =
            match load_texture(&asset_path("assets/ui/quest_complete.png")).await {
                Ok(tex) => {
                    tex.set_filter(FilterMode::Nearest);
                    log::info!(
                        "Loaded quest complete texture: {}x{}",
                        tex.width(),
                        tex.height()
                    );
                    Some(tex)
                }
                Err(e) => {
                    log::warn!("Failed to load quest complete texture: {}", e);
                    None
                }
            };

        // Load gold nugget icon for inventory
        let gold_nugget_texture = match load_texture(&asset_path("assets/ui/gold_nugget.png")).await
        {
            Ok(tex) => {
                tex.set_filter(FilterMode::Nearest);
                log::info!(
                    "Loaded gold nugget texture: {}x{}",
                    tex.width(),
                    tex.height()
                );
                Some(tex)
            }
            Err(e) => {
                log::warn!("Failed to load gold nugget texture: {}", e);
                None
            }
        };

        // Load circular stone backdrop for shop item icons
        let circular_stone_texture =
            match load_texture(&asset_path("assets/ui/circular_stone.png")).await {
                Ok(tex) => {
                    tex.set_filter(FilterMode::Nearest);
                    log::info!(
                        "Loaded circular stone texture: {}x{}",
                        tex.width(),
                        tex.height()
                    );
                    Some(tex)
                }
                Err(e) => {
                    log::warn!("Failed to load circular stone texture: {}", e);
                    None
                }
            };

        // Load menu button icons sprite sheet
        let menu_button_icons =
            match load_texture(&asset_path("assets/ui/background_icons.png")).await {
                Ok(tex) => {
                    tex.set_filter(FilterMode::Nearest);
                    log::info!("Loaded menu button icons: {}x{}", tex.width(), tex.height());
                    Some(tex)
                }
                Err(e) => {
                    log::warn!("Failed to load menu button icons: {}", e);
                    None
                }
            };

        // Load UI icons sprite sheet (24x24 icons in 10x10 grid)
        let ui_icons = match load_texture(&asset_path("assets/ui/ui_icons.png")).await {
            Ok(tex) => {
                tex.set_filter(FilterMode::Nearest);
                log::info!("Loaded UI icons: {}x{}", tex.width(), tex.height());
                Some(tex)
            }
            Err(e) => {
                log::warn!("Failed to load UI icons: {}", e);
                None
            }
        };

        // Load small icons for NPC name tags
        let chat_small_icon = match load_texture(&asset_path("assets/ui/chat_small.png")).await {
            Ok(tex) => {
                tex.set_filter(FilterMode::Nearest);
                Some(tex)
            }
            Err(e) => {
                log::warn!("Failed to load chat_small icon: {}", e);
                None
            }
        };

        let fishing_skill_icon =
            match load_texture(&asset_path("assets/ui/fishing_skill.png")).await {
                Ok(tex) => {
                    tex.set_filter(FilterMode::Nearest);
                    Some(tex)
                }
                Err(e) => {
                    log::warn!("Failed to load fishing_skill icon: {}", e);
                    None
                }
            };

        let coin_small_icon = match load_texture(&asset_path("assets/ui/coin_small.png")).await {
            Ok(tex) => {
                tex.set_filter(FilterMode::Nearest);
                Some(tex)
            }
            Err(e) => {
                log::warn!("Failed to load coin_small icon: {}", e);
                None
            }
        };

        let destination_flag =
            match load_texture(&asset_path("assets/ui/destination_flag.png")).await {
                Ok(tex) => {
                    tex.set_filter(FilterMode::Nearest);
                    Some(tex)
                }
                Err(e) => {
                    log::warn!("Failed to load destination_flag icon: {}", e);
                    None
                }
            };

        let click_walk_texture = match load_texture(&asset_path("assets/ui/walk_click.png")).await {
            Ok(tex) => {
                tex.set_filter(FilterMode::Nearest);
                Some(tex)
            }
            Err(e) => {
                log::warn!("Failed to load walk_click texture: {}", e);
                None
            }
        };
        let click_attack_texture =
            match load_texture(&asset_path("assets/ui/attack_click.png")).await {
                Ok(tex) => {
                    tex.set_filter(FilterMode::Nearest);
                    Some(tex)
                }
                Err(e) => {
                    log::warn!("Failed to load attack_click texture: {}", e);
                    None
                }
            };
        let click_interact_texture =
            match load_texture(&asset_path("assets/ui/interact_click.png")).await {
                Ok(tex) => {
                    tex.set_filter(FilterMode::Nearest);
                    Some(tex)
                }
                Err(e) => {
                    log::warn!("Failed to load interact_click texture: {}", e);
                    None
                }
            };

        let map_icons = match load_texture(&asset_path("assets/ui/map-icons.png")).await {
            Ok(tex) => {
                tex.set_filter(FilterMode::Nearest);
                log::info!("Loaded map icons: {}x{}", tex.width(), tex.height());
                Some(tex)
            }
            Err(e) => {
                log::warn!("Failed to load map icons: {}", e);
                None
            }
        };

        // Pre-compute pixel-perfect outline textures for map icon hover state.
        // For each icon, we find transparent pixels adjacent to opaque pixels and
        // paint them white. The outline texture uses 18x18 per icon (1px border).
        let map_icons_outlines = map_icons.as_ref().map(|tex| {
            let img = tex.get_texture_data();
            let icon_count = (img.width / 16) as i32;
            let out_w = (icon_count * 18) as u16;
            let out_h = 18u16;
            let mut outline = Image::gen_image_color(out_w, out_h, Color::new(0.0, 0.0, 0.0, 0.0));
            let outline_color = Color::new(1.0, 1.0, 1.0, 0.9);

            for icon_idx in 0..icon_count {
                let src_x0 = icon_idx * 16;
                let dst_x0 = icon_idx * 18;
                for oy in 0..18i32 {
                    for ox in 0..18i32 {
                        let lx = ox - 1; // local coord in icon space
                        let ly = oy - 1;
                        // Current pixel: transparent if outside icon bounds
                        let is_transparent = if (0..16).contains(&lx) && (0..16).contains(&ly) {
                            img.get_pixel((src_x0 + lx) as u32, ly as u32).a < 0.5
                        } else {
                            true
                        };
                        if is_transparent {
                            // Check 4 cardinal neighbors for opaque pixels within icon bounds
                            for &(dx, dy) in &[(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                                let nlx = lx + dx;
                                let nly = ly + dy;
                                if (0..16).contains(&nlx)
                                    && (0..16).contains(&nly)
                                    && img.get_pixel((src_x0 + nlx) as u32, nly as u32).a >= 0.5
                                {
                                    outline.set_pixel(
                                        (dst_x0 + ox) as u32,
                                        oy as u32,
                                        outline_color,
                                    );
                                    break;
                                }
                            }
                        }
                    }
                }
            }

            let outline_tex = Texture2D::from_image(&outline);
            outline_tex.set_filter(FilterMode::Nearest);
            outline_tex
        });

        // Load arrow projectile spritesheet
        let arrow_projectile_texture =
            match load_texture(&asset_path("assets/sprites/arrow_angles.png")).await {
                Ok(tex) => {
                    tex.set_filter(FilterMode::Nearest);
                    Some(tex)
                }
                Err(e) => {
                    log::warn!("Failed to load arrow_angles spritesheet: {}", e);
                    None
                }
            };

        // Load auto-retaliate icon
        let auto_retaliate_icon =
            match load_texture(&asset_path("assets/ui/auto_retaliate.png")).await {
                Ok(tex) => {
                    tex.set_filter(FilterMode::Nearest);
                    Some(tex)
                }
                Err(e) => {
                    log::warn!("Failed to load auto_retaliate icon: {}", e);
                    None
                }
            };

        // Load exit portal arrow textures
        let exit_arrow_up = match load_texture(&asset_path("assets/ui/up_arrow.png")).await {
            Ok(tex) => {
                tex.set_filter(FilterMode::Nearest);
                Some(tex)
            }
            Err(e) => {
                log::warn!("Failed to load up_arrow icon: {}", e);
                None
            }
        };
        let exit_arrow_down = match load_texture(&asset_path("assets/ui/down_arrow.png")).await {
            Ok(tex) => {
                tex.set_filter(FilterMode::Nearest);
                Some(tex)
            }
            Err(e) => {
                log::warn!("Failed to load down_arrow icon: {}", e);
                None
            }
        };
        let exit_arrow_left = match load_texture(&asset_path("assets/ui/left_arrow.png")).await {
            Ok(tex) => {
                tex.set_filter(FilterMode::Nearest);
                Some(tex)
            }
            Err(e) => {
                log::warn!("Failed to load left_arrow icon: {}", e);
                None
            }
        };
        let exit_arrow_right = match load_texture(&asset_path("assets/ui/right_arrow.png")).await {
            Ok(tex) => {
                tex.set_filter(FilterMode::Nearest);
                Some(tex)
            }
            Err(e) => {
                log::warn!("Failed to load right_arrow icon: {}", e);
                None
            }
        };

        // farming_sprites loaded via atlas/manifest in earlier block
        log::info!("Farming sprites loaded: {}", farming_sprites.len());

        // Load prayer icons - atlas on WASM/Android, individual files on desktop
        #[cfg(any(target_os = "android", target_arch = "wasm32"))]
        let prayer_icons: SpriteStore = if let Some(ref atlas_info) = manifest.prayers_atlas {
            if let Some(atlas) = load_atlas(atlas_info).await {
                SpriteStore::Atlas(atlas)
            } else {
                SpriteStore::Individual(HashMap::new())
            }
        } else {
            SpriteStore::Individual(HashMap::new())
        };
        #[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
        let prayer_icons: SpriteStore = {
            let prayer_names = [
                "clarity",
                "thick_skin",
                "burst_of_strength",
                "improved_clarity",
                "rock_skin",
                "superhuman_strength",
                "resourcefulness",
                "rapid_heal",
                "steel_skin",
                "incredible_clarity",
                "ultimate_strength",
                "protection",
                "greater_resourcefulness",
                "greater_protection",
            ];
            let mut icons = HashMap::new();
            for prayer in &prayer_names {
                let path = asset_path(&format!("assets/ui/prayers/{}.png", prayer));
                if let Ok(tex) = load_texture(&path).await {
                    tex.set_filter(FilterMode::Nearest);
                    icons.insert(prayer.to_string(), tex);
                }
            }
            SpriteStore::Individual(icons)
        };
        log::info!("Loaded {} prayer icons", prayer_icons.len());

        // Load spell icons - atlas on WASM/Android, individual files on desktop
        #[cfg(any(target_os = "android", target_arch = "wasm32"))]
        let spell_icons: SpriteStore = if let Some(ref atlas_info) = manifest.spells_atlas {
            if let Some(atlas) = load_atlas(atlas_info).await {
                SpriteStore::Atlas(atlas)
            } else {
                SpriteStore::Individual(HashMap::new())
            }
        } else {
            SpriteStore::Individual(HashMap::new())
        };
        #[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
        let spell_icons: SpriteStore = {
            // spell_id -> icon_filename mapping (spell ids don't always match filenames)
            let spell_icon_mappings = [
                ("dark_hand", "dark_hand"),
                ("lightning_bolt", "lightning_bolt"),
                ("dark_eater", "dark_eater"),
                ("rock_fall", "rock_fall"),
                ("heal", "heal"),
                ("return_home", "return_home"),
                ("greater_heal", "greater_heal"),
                ("tornado", "tornado"),
                ("air_blast", "air_blast"),
                ("water_blast", "water_blast"),
                ("earth_blast", "earth_blast"),
                ("fire_blast", "fire_blast"),
            ];
            let mut icons = HashMap::new();
            for (spell_id, icon_name) in &spell_icon_mappings {
                let path = asset_path(&format!("assets/ui/spells/{}.png", icon_name));
                if let Ok(tex) = load_texture(&path).await {
                    tex.set_filter(FilterMode::Nearest);
                    icons.insert(spell_id.to_string(), tex);
                }
            }
            SpriteStore::Individual(icons)
        };
        log::info!("Loaded {} spell icons", spell_icons.len());

        // Load miscellaneous UI icons atlas (WASM/Android only)
        #[cfg(any(target_os = "android", target_arch = "wasm32"))]
        let ui_misc_atlas: Option<SpriteAtlas> =
            if let Some(ref atlas_info) = manifest.ui_misc_atlas {
                load_atlas(atlas_info).await
            } else {
                None
            };
        #[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
        let ui_misc_atlas: Option<SpriteAtlas> = None;

        // spell_effect_textures loaded via atlas/manifest in earlier block
        log::info!(
            "Spell effect textures loaded: {}",
            spell_effect_textures.len()
        );

        set_loading!("Loading shaders...");

        // Load head+hair composite shader material
        let head_hair_material = match load_material(
            ShaderSource::Glsl {
                vertex: shaders::HEAD_HAIR_VERTEX,
                fragment: shaders::HEAD_HAIR_FRAGMENT,
            },
            MaterialParams {
                textures: vec!["HairTexture".to_string()],
                uniforms: vec![
                    UniformDesc::new("HairUvTransform", UniformType::Float4),
                    UniformDesc::new("Tint", UniformType::Float4),
                ],
                ..Default::default()
            },
        ) {
            Ok(mat) => {
                log::info!("Loaded head+hair composite shader");
                Some(mat)
            }
            Err(e) => {
                log::warn!("Failed to load head+hair shader: {}. Head equipment will render without hair masking.", e);
                None
            }
        };

        // Water shaders disabled for now
        let water_material: Option<Material> = None;
        let water_overlay_material: Option<Material> = None;

        // Build animated sprite lookup maps from manifest frame metadata
        let animated_objects = Self::build_animated_map(&manifest.objects_atlas);
        let animated_walls = Self::build_animated_map(&manifest.walls_atlas);
        if !animated_objects.is_empty() {
            log::info!("Found {} animated object sprites", animated_objects.len());
        }
        if !animated_walls.is_empty() {
            log::info!("Found {} animated wall sprites", animated_walls.len());
        }

        // Detect which NPC sprites have non-transparent second idle frames
        let mut npc_idle_anim_set = Self::detect_npc_idle_animations(&npc_sprites);
        // Remove any entries explicitly marked as having no idle animation
        for name in &manifest.no_idle_animation {
            npc_idle_anim_set.remove(name);
        }
        if !npc_idle_anim_set.is_empty() {
            log::info!(
                "Found {} NPCs with idle animations: {:?}",
                npc_idle_anim_set.len(),
                npc_idle_anim_set
            );
        }

        #[cfg(target_arch = "wasm32")]
        Self::hide_loading();

        Self {
            player_color: Color::from_rgba(100, 150, 255, 255),
            local_player_color: Color::from_rgba(100, 255, 150, 255),
            tileset,
            player_sprites,
            hair_sprites,
            equipment_sprites,
            weapon_sprites,
            weapon_frame_sizes,
            item_sprites,
            object_sprites,
            wall_sprites,
            npc_sprites,
            npc_overflow_sprites,
            npc_idle_anim_set,
            font,
            quest_complete_texture,
            gold_nugget_texture,
            circular_stone_texture,
            menu_button_icons,
            ui_icons,
            fishing_skill_icon,
            chat_small_icon,
            coin_small_icon,
            destination_flag,
            click_walk_texture,
            click_attack_texture,
            click_interact_texture,
            map_icons,
            map_icons_outlines,
            farming_sprites,
            prayer_icons,
            spell_icons,
            ui_misc_atlas,
            spell_effect_textures,
            head_hair_material,
            water_material,
            water_overlay_material,
            arrow_projectile_texture,
            auto_retaliate_icon,
            exit_arrow_up,
            exit_arrow_down,
            exit_arrow_left,
            exit_arrow_right,
            chat_lines_cache: RefCell::new(ChatLinesCache::default()),
            tileset_image_cache: RefCell::new(None),
            minimap_tile_color_cache: RefCell::new(HashMap::new()),
            text_measure_cache: RefCell::new(HashMap::new()),
            text_wrap_cache: RefCell::new(HashMap::new()),
            font_scale: Cell::new(1.0),
            xp_drop_pos: Cell::new(None),
            silhouette_rt: RefCell::new(None),
            animated_objects,
            animated_walls,
            falling_tree_positions: RefCell::new(HashSet::new()),
            tree_shake_offsets: RefCell::new(HashMap::new()),
            crumbling_rock_positions: RefCell::new(HashSet::new()),
            rock_shake_offsets: RefCell::new(HashMap::new()),
        }
    }
}
