use crate::game::state::AttackSoundType;
use crate::util::asset_path;
use macroquad::audio::{
    load_sound, play_sound, set_sound_volume, stop_sound, PlaySoundParams, Sound,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone)]
pub struct AudioSettings {
    pub music_volume: f32,
    pub sfx_volume: f32,
    // Per-channel mute. `#[serde(default)]` so saves from before the split
    // (which only had a single `muted`) still load cleanly.
    #[serde(default)]
    pub music_muted: bool,
    #[serde(default)]
    pub sfx_muted: bool,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            music_volume: 0.5,
            sfx_volume: 0.5,
            music_muted: false,
            sfx_muted: false,
        }
    }
}

/// Map a 0..1 slider position to an amplitude multiplier with a perceptual
/// taper. Human loudness perception is roughly logarithmic, so feeding the raw
/// linear slider value straight into the audio engine makes the bottom ~70% of
/// the slider barely attenuate anything. Squaring gives a curve that feels
/// linear to the ear (e.g. slider 0.5 -> 0.25 amplitude, ~-12 dB).
fn perceptual_volume(slider: f32) -> f32 {
    let s = slider.clamp(0.0, 1.0);
    s * s
}

pub struct AudioManager {
    current_music: Option<Sound>,
    current_music_path: Option<String>,
    settings: AudioSettings,
    /// Preloaded sound effects by name
    sfx: HashMap<String, Sound>,
    /// Sword attack sounds for random selection
    sword_sounds: Vec<Sound>,
    /// Bow attack sound
    bow_sound: Option<Sound>,
    /// Preloaded music tracks by path
    music: HashMap<String, Sound>,
}

impl AudioManager {
    /// Create AudioManager without preloading (call preload_all() separately during loading screen)
    pub fn new_without_preload() -> Self {
        let settings = load_settings();
        Self {
            current_music: None,
            current_music_path: None,
            settings,
            sfx: HashMap::new(),
            sword_sounds: Vec::new(),
            bow_sound: None,
            music: HashMap::new(),
        }
    }

    /// Create AudioManager and preload all audio
    pub async fn new() -> Self {
        let mut manager = Self::new_without_preload();
        manager.preload_all().await;
        manager
    }

    /// Preload all audio (SFX + music) - call this during loading screen
    pub async fn preload_all(&mut self) {
        self.preload_sfx().await;
        self.preload_music().await;
    }

    /// Preload music tracks at startup
    pub async fn preload_music(&mut self) {
        let music_files = [
            "assets/audio/menu.ogg",
            "assets/audio/start.ogg",
            "assets/audio/desert-boss-battle.ogg",
        ];

        for path in music_files {
            match load_sound(&asset_path(path)).await {
                Ok(sound) => {
                    self.music.insert(path.to_string(), sound);
                    log::info!("Preloaded music: {}", path);
                }
                Err(e) => {
                    log::warn!("Failed to preload music '{}': {:?}", path, e);
                }
            }
        }
    }

    /// Preload all sound effects at startup
    async fn preload_sfx(&mut self) {
        let sfx_files = [
            ("enter", "assets/audio/sfx/menu/enter.wav"),
            ("item_grab", "assets/audio/sfx/menu/item_grab.wav"),
            ("item_put", "assets/audio/sfx/menu/item_put.wav"),
            ("send_message", "assets/audio/sfx/menu/send_message.wav"),
            ("buy", "assets/audio/sfx/menu/buy.wav"),
            ("login_success", "assets/audio/sfx/menu/login_success.wav"),
            ("level_up", "assets/audio/sfx/menu/level_up.wav"),
            ("announce", "assets/audio/sfx/misc/announce.wav"),
            ("unarmed", "assets/audio/sfx/attack/unarmed.wav"),
            ("woodcut", "assets/audio/sfx/misc/woodcut.ogg"),
            ("mining", "assets/audio/sfx/misc/mining.ogg"),
            ("ui_open", "assets/audio/sfx/ui_open.ogg"),
            ("message_add", "assets/audio/sfx/message_add.ogg"),
            ("pop", "assets/audio/sfx/misc/pop.ogg"),
            ("quest_complete", "assets/audio/sfx/misc/quest_complete.ogg"),
            ("furnace", "assets/audio/sfx/misc/furnace.ogg"),
            ("death", "assets/audio/sfx/misc/death.ogg"),
            ("error", "assets/audio/sfx/error.wav"),
            ("rock_explode", "assets/audio/sfx/rock_explode.ogg"),
            ("aoe_rockfall", "assets/audio/sfx/spells/aoe_rockfall.ogg"),
        ];

        for (name, path) in sfx_files {
            match load_sound(&asset_path(path)).await {
                Ok(sound) => {
                    self.sfx.insert(name.to_string(), sound);
                    log::debug!("Loaded SFX: {}", name);
                }
                Err(e) => {
                    log::warn!("Failed to load SFX '{}': {:?}", path, e);
                }
            }
        }

        // Load sword attack sounds for random selection
        for i in 1..=4 {
            let path = asset_path(&format!("assets/audio/sfx/attack/sword_{}.wav", i));
            match load_sound(&path).await {
                Ok(sound) => {
                    self.sword_sounds.push(sound);
                    log::debug!("Loaded sword sound {}", i);
                }
                Err(e) => {
                    log::warn!("Failed to load sword sound '{}': {:?}", path, e);
                }
            }
        }

        // Load bow attack sound
        let bow_path = asset_path("assets/audio/sfx/attack/bow.wav");
        match load_sound(&bow_path).await {
            Ok(sound) => {
                self.bow_sound = Some(sound);
                log::debug!("Loaded bow sound");
            }
            Err(e) => {
                log::warn!("Failed to load bow sound '{}': {:?}", bow_path, e);
            }
        }

        log::info!(
            "Loaded {} SFX, {} sword sounds",
            self.sfx.len(),
            self.sword_sounds.len()
        );
    }

    /// Play a sound effect by name
    pub fn play_sfx(&self, name: &str) {
        if let Some(sound) = self.sfx.get(name) {
            play_sound(
                sound,
                PlaySoundParams {
                    looped: false,
                    volume: self.effective_sfx_volume(),
                },
            );
        } else {
            log::warn!("Unknown SFX: {}", name);
        }
    }

    /// Play attack sound based on weapon type
    pub fn play_attack_sound(&self, attack_type: AttackSoundType) {
        match attack_type {
            AttackSoundType::Ranged => {
                if let Some(sound) = &self.bow_sound {
                    play_sound(
                        sound,
                        PlaySoundParams {
                            looped: false,
                            volume: self.effective_sfx_volume(),
                        },
                    );
                }
            }
            AttackSoundType::Melee => {
                if !self.sword_sounds.is_empty() {
                    let idx = macroquad::rand::gen_range(0, self.sword_sounds.len());
                    play_sound(
                        &self.sword_sounds[idx],
                        PlaySoundParams {
                            looped: false,
                            volume: self.effective_sfx_volume(),
                        },
                    );
                }
            }
            AttackSoundType::Unarmed => {
                self.play_sfx("unarmed");
            }
        }
    }

    /// Play a preloaded music track (sync — won't load on demand)
    pub fn play_music_preloaded(&mut self, path: &str) {
        if self.current_music_path.as_deref() == Some(path) {
            return;
        }
        self.stop_music();
        let volume = self.effective_music_volume();
        if let Some(sound) = self.music.get(path) {
            log::info!("Playing preloaded music: {} (volume: {})", path, volume);
            play_sound(
                sound,
                PlaySoundParams {
                    looped: true,
                    volume,
                },
            );
            self.current_music = Some(sound.clone());
            self.current_music_path = Some(path.to_string());
        } else {
            log::warn!("Music not preloaded: {}", path);
        }
    }

    pub async fn play_music(&mut self, path: &str) {
        // Don't restart the same track
        if self.current_music_path.as_deref() == Some(path) {
            return;
        }

        // Stop any currently playing music
        self.stop_music();

        let volume = self.effective_music_volume();

        // Check if music is preloaded
        if let Some(sound) = self.music.get(path) {
            log::info!("Playing preloaded music: {} (volume: {})", path, volume);
            play_sound(
                sound,
                PlaySoundParams {
                    looped: true,
                    volume,
                },
            );
            self.current_music = Some(sound.clone());
            self.current_music_path = Some(path.to_string());
            return;
        }

        // Fall back to loading on demand
        let actual_path = asset_path(path);
        log::info!("Loading music from: {}", actual_path);
        match load_sound(&actual_path).await {
            Ok(sound) => {
                log::info!(
                    "Playing music with volume: {} (muted: {})",
                    volume,
                    self.settings.music_muted
                );
                play_sound(
                    &sound,
                    PlaySoundParams {
                        looped: true,
                        volume,
                    },
                );
                self.current_music = Some(sound);
                self.current_music_path = Some(path.to_string());
                log::info!("Music started successfully");
            }
            Err(e) => {
                log::error!("Failed to load music '{}': {:?}", path, e);
            }
        }
    }

    pub fn stop_music(&mut self) {
        if let Some(sound) = self.current_music.take() {
            stop_sound(&sound);
        }
        self.current_music_path = None;
    }

    pub fn set_music_volume(&mut self, volume: f32) {
        self.settings.music_volume = volume.clamp(0.0, 1.0);
        self.apply_music_volume();
        save_settings(&self.settings);
    }

    pub fn set_sfx_volume(&mut self, volume: f32) {
        self.settings.sfx_volume = volume.clamp(0.0, 1.0);
        save_settings(&self.settings);
    }

    pub fn toggle_music_mute(&mut self) {
        self.settings.music_muted = !self.settings.music_muted;
        self.apply_music_volume();
        save_settings(&self.settings);
    }

    pub fn toggle_sfx_mute(&mut self) {
        self.settings.sfx_muted = !self.settings.sfx_muted;
        save_settings(&self.settings);
    }

    pub fn effective_music_volume(&self) -> f32 {
        if self.settings.music_muted {
            0.0
        } else {
            perceptual_volume(self.settings.music_volume)
        }
    }

    pub fn effective_sfx_volume(&self) -> f32 {
        if self.settings.sfx_muted {
            0.0
        } else {
            perceptual_volume(self.settings.sfx_volume)
        }
    }

    pub fn music_volume(&self) -> f32 {
        self.settings.music_volume
    }

    pub fn sfx_volume(&self) -> f32 {
        self.settings.sfx_volume
    }

    pub fn is_music_muted(&self) -> bool {
        self.settings.music_muted
    }

    pub fn is_sfx_muted(&self) -> bool {
        self.settings.sfx_muted
    }

    fn apply_music_volume(&self) {
        if let Some(ref sound) = self.current_music {
            set_sound_volume(sound, self.effective_music_volume());
        }
    }
}

// Platform-specific settings persistence

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
fn settings_path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|p| p.join("new-aeven").join("audio.toml"))
}

#[cfg(target_os = "android")]
fn settings_path() -> Option<std::path::PathBuf> {
    crate::settings::android_files_dir().map(|d| d.join("audio_settings.toml"))
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
fn load_settings() -> AudioSettings {
    let Some(path) = settings_path() else {
        return AudioSettings::default();
    };

    match std::fs::read_to_string(&path) {
        Ok(contents) => toml::from_str(&contents).unwrap_or_default(),
        Err(_) => AudioSettings::default(),
    }
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
fn save_settings(settings: &AudioSettings) {
    let Some(path) = settings_path() else {
        return;
    };

    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    if let Ok(contents) = toml::to_string_pretty(settings) {
        let _ = std::fs::write(&path, contents);
    }
}

#[cfg(target_os = "android")]
fn load_settings() -> AudioSettings {
    // On Android, settings file is in the app's current directory
    let Some(path) = settings_path() else {
        return AudioSettings::default();
    };

    match std::fs::read_to_string(&path) {
        Ok(contents) => toml::from_str(&contents).unwrap_or_default(),
        Err(_) => AudioSettings::default(),
    }
}

#[cfg(target_os = "android")]
fn save_settings(settings: &AudioSettings) {
    let Some(path) = settings_path() else {
        return;
    };

    if let Ok(contents) = toml::to_string_pretty(settings) {
        let _ = std::fs::write(&path, contents);
    }
}

#[cfg(target_arch = "wasm32")]
fn load_settings() -> AudioSettings {
    quad_storage::STORAGE
        .lock()
        .expect("storage lock")
        .get("audio_settings")
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

#[cfg(target_arch = "wasm32")]
fn save_settings(settings: &AudioSettings) {
    if let Ok(json) = serde_json::to_string(settings) {
        quad_storage::STORAGE
            .lock()
            .expect("storage lock")
            .set("audio_settings", &json);
    }
}
