/// Client-side tutorial state machine for new player onboarding.
///
/// Tracks the player through 6 phases, each teaching one gameplay concept.
/// Driven entirely on the client — the server only provides `is_new_character`.
use macroquad::prelude::get_time;

use super::state::{ActiveDialogue, DialogueChoice};

/// Tutorial phases in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TutorialPhase {
    /// Waiting for the player to accept/skip the tutorial from Old Thomas's greeting.
    AwaitingAccept,
    /// Phase 1: Walk around.
    Movement,
    /// Phase 2: Talk to an NPC (open any dialogue).
    NpcInteraction,
    /// Phase 3: Attack an enemy.
    Combat,
    /// Phase 4: Open inventory.
    Inventory,
    /// Phase 5: Open skills panel.
    Skills,
    /// Phase 6: Handoff to Adventurer Guide (complete).
    Handoff,
    /// Tutorial finished (completed or skipped).
    Done,
}

/// Manages the tutorial flow for new players.
#[derive(Debug, Clone)]
pub struct TutorialManager {
    pub phase: TutorialPhase,
    pub classic_controls: bool,
    /// Number of tiles the player has moved since the movement phase started.
    pub tiles_moved: u32,
    /// Whether the hint bar should be visible.
    pub hint_visible: bool,
    /// Time when the current phase started (for fade animations).
    pub phase_start_time: f64,
    /// Whether we're waiting to show the next dialogue (after current dialogue closes).
    pub pending_dialogue: bool,
    /// Time when handoff phase started (for arrow duration).
    pub handoff_start_time: f64,
}

impl TutorialManager {
    pub fn new(classic_controls: bool) -> Self {
        Self {
            phase: TutorialPhase::AwaitingAccept,
            classic_controls,
            tiles_moved: 0,
            hint_visible: false,
            phase_start_time: get_time(),
            pending_dialogue: false,
            handoff_start_time: 0.0,
        }
    }

    /// Whether the tutorial is actively running (not done, not awaiting accept).
    pub fn is_active(&self) -> bool {
        !matches!(self.phase, TutorialPhase::Done | TutorialPhase::AwaitingAccept)
    }

    /// Whether the tutorial is completely finished.
    pub fn is_done(&self) -> bool {
        self.phase == TutorialPhase::Done
    }

    /// Advance to the next phase.
    pub fn advance(&mut self) {
        self.phase = match self.phase {
            TutorialPhase::AwaitingAccept => TutorialPhase::Movement,
            TutorialPhase::Movement => TutorialPhase::NpcInteraction,
            TutorialPhase::NpcInteraction => TutorialPhase::Combat,
            TutorialPhase::Combat => TutorialPhase::Inventory,
            TutorialPhase::Inventory => TutorialPhase::Skills,
            TutorialPhase::Skills => TutorialPhase::Handoff,
            TutorialPhase::Handoff => TutorialPhase::Done,
            TutorialPhase::Done => TutorialPhase::Done,
        };
        self.phase_start_time = get_time();
        self.pending_dialogue = true;
        if self.phase == TutorialPhase::Handoff {
            self.handoff_start_time = get_time();
        }
    }

    /// Skip the tutorial entirely.
    pub fn skip(&mut self) {
        self.phase = TutorialPhase::Done;
        self.hint_visible = false;
    }

    /// Get the hint text for the current phase, respecting control scheme.
    pub fn hint_text(&self) -> &'static str {
        match self.phase {
            TutorialPhase::Movement => {
                if self.classic_controls {
                    "Use Arrow Keys to move"
                } else {
                    "Use WASD to move"
                }
            }
            TutorialPhase::NpcInteraction => "Walk near an NPC and click them to interact",
            TutorialPhase::Combat => {
                if self.classic_controls {
                    "Press Ctrl to attack nearby enemies"
                } else {
                    "Press Space to attack nearby enemies"
                }
            }
            TutorialPhase::Inventory => "Press I to open your inventory",
            TutorialPhase::Skills => "Press T to view your skills",
            TutorialPhase::Handoff => "Go talk to the Adventurer Guide!",
            _ => "",
        }
    }

    /// Build the Old Thomas dialogue for the current phase.
    /// Returns None if no dialogue should be shown for this phase.
    pub fn phase_dialogue(&self) -> Option<ActiveDialogue> {
        let (text, choices) = match self.phase {
            TutorialPhase::AwaitingAccept => (
                "Welcome to New Aeven, friend! I'm Old Thomas. I've been around these parts longer than most.\n\nWant me to show you the ropes?".to_string(),
                vec![
                    DialogueChoice {
                        id: "accept".to_string(),
                        text: "Yes, show me around!".to_string(),
                    },
                    DialogueChoice {
                        id: "skip".to_string(),
                        text: "No thanks, I'll figure it out.".to_string(),
                    },
                ],
            ),
            TutorialPhase::Movement => {
                let key = if self.classic_controls {
                    "Arrow Keys"
                } else {
                    "WASD"
                };
                (
                    format!("Let's start with the basics! Try walking around using {}. Go on, stretch those legs!", key),
                    vec![DialogueChoice {
                        id: "ok".to_string(),
                        text: "Got it!".to_string(),
                    }],
                )
            }
            TutorialPhase::NpcInteraction => (
                "Great job! Now, you'll want to talk to the folks around here. Walk up to any NPC and click on them to interact.".to_string(),
                vec![DialogueChoice {
                    id: "ok".to_string(),
                    text: "Will do!".to_string(),
                }],
            ),
            TutorialPhase::Combat => {
                let key = if self.classic_controls { "Ctrl" } else { "Space" };
                (
                    format!("You're a natural! But the world out here isn't always friendly. See those crows? Try taking one out with {}.", key),
                    vec![DialogueChoice {
                        id: "ok".to_string(),
                        text: "Time to fight!".to_string(),
                    }],
                )
            }
            TutorialPhase::Inventory => (
                "Nice work, adventurer! Enemies drop useful things. Press I to open your inventory and see what you've got.".to_string(),
                vec![DialogueChoice {
                    id: "ok".to_string(),
                    text: "Let me check!".to_string(),
                }],
            ),
            TutorialPhase::Skills => (
                "Did you notice? You earned experience from that fight! Press T to see your skills. They grow as you play.".to_string(),
                vec![DialogueChoice {
                    id: "ok".to_string(),
                    text: "Interesting!".to_string(),
                }],
            ),
            TutorialPhase::Handoff => (
                "You're ready to go, friend! If you want some direction, go talk to the Adventurer Guide nearby. They'll set you on the right path.\n\nGood luck out there!".to_string(),
                vec![DialogueChoice {
                    id: "ok".to_string(),
                    text: "Thanks, Old Thomas!".to_string(),
                }],
            ),
            TutorialPhase::Done => return None,
        };

        Some(ActiveDialogue {
            quest_id: "__tutorial__".to_string(),
            npc_id: String::new(),
            speaker: "Old Thomas".to_string(),
            text,
            choices,
            show_time: get_time(),
        })
    }

    /// Notify that the player moved a tile.
    pub fn on_player_moved(&mut self) {
        if self.phase == TutorialPhase::Movement {
            self.tiles_moved += 1;
            if self.tiles_moved >= 3 {
                self.advance();
            }
        }
    }

    /// Notify that the player opened a dialogue (with any NPC).
    pub fn on_dialogue_opened(&mut self) {
        if self.phase == TutorialPhase::NpcInteraction {
            self.advance();
        }
    }

    /// Notify that the player dealt damage or killed an enemy.
    pub fn on_combat_action(&mut self) {
        if self.phase == TutorialPhase::Combat {
            self.advance();
        }
    }

    /// Notify that the player opened the inventory.
    pub fn on_inventory_opened(&mut self) {
        if self.phase == TutorialPhase::Inventory {
            self.advance();
        }
    }

    /// Notify that the player opened the skills panel.
    pub fn on_skills_opened(&mut self) {
        if self.phase == TutorialPhase::Skills {
            self.advance();
        }
    }
}
