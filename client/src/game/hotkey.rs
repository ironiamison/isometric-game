//! Hotkey bar data model — unified slots that can hold items or spells

use serde::{Deserialize, Serialize};

/// What a single hotkey slot is bound to
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum HotkeySlotBinding {
    Empty,
    Item { item_id: String },
    Spell { spell_id: String },
}

impl Default for HotkeySlotBinding {
    fn default() -> Self {
        Self::Empty
    }
}

/// A single preset containing 5 hotkey slot bindings
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HotkeyPreset {
    pub slots: [HotkeySlotBinding; 5],
}

impl Default for HotkeyPreset {
    fn default() -> Self {
        Self {
            slots: Default::default(),
        }
    }
}

/// Full hotkey bar configuration: 5 presets, one active at a time
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HotkeyBarConfig {
    pub presets: [HotkeyPreset; 5],
    pub active_preset: usize,
}

impl Default for HotkeyBarConfig {
    fn default() -> Self {
        Self {
            presets: Default::default(),
            active_preset: 0,
        }
    }
}

impl HotkeyBarConfig {
    /// Get the currently active preset (immutable)
    pub fn active(&self) -> &HotkeyPreset {
        &self.presets[self.active_preset]
    }

    /// Get the currently active preset (mutable)
    pub fn active_mut(&mut self) -> &mut HotkeyPreset {
        &mut self.presets[self.active_preset]
    }

    /// Cycle to the next preset (wraps around)
    pub fn cycle_up(&mut self) {
        self.active_preset = (self.active_preset + 1) % 5;
    }

    /// Cycle to the previous preset (wraps around)
    pub fn cycle_down(&mut self) {
        self.active_preset = (self.active_preset + 4) % 5;
    }
}
