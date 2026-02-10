//! Quest System Module
//!
//! Hybrid quest system using TOML for metadata and Lua for complex logic.
//! Features hot-reloadable scripts, quest chains, and branching dialogue.

pub mod api;
pub mod definition;
pub mod events;
pub mod registry;
pub mod runner;
pub mod state;

pub use api::QuestContext;
pub use definition::{Objective, ObjectiveType, Quest, QuestChain, QuestDialogue, Reward};
pub use events::QuestEvent;
pub use registry::{HotReloadEvent, QuestRegistry};
pub use runner::{BonusReward, DialogueChoice, DialogueResult, QuestRunner, ScriptResult};
pub use state::{PlayerQuestState, QuestProgress, QuestStatus};
