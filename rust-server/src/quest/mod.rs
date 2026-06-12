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

pub use definition::{ObjectiveType, Quest};
pub use events::QuestEvent;
pub use registry::QuestRegistry;
#[cfg(debug_assertions)]
pub use registry::HotReloadEvent;
pub use runner::QuestRunner;
pub use state::{PlayerQuestState, QuestStatus};
