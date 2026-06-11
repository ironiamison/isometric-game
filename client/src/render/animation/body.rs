use super::*;

// ============================================================================
// Body Armor Animation System (single-row sprite format)
// ============================================================================

/// Result of body armor frame calculation
#[derive(Debug, Clone, Copy)]
pub struct BodyArmorFrameResult {
    /// Frame index (0-based) in the single-row spritesheet
    pub frame: u32,
    /// Whether to flip the sprite horizontally
    pub flip_h: bool,
}

/// Get the body armor frame index for the current animation state and direction
///
/// Body armor sprite sheet layout (0-indexed, 22 frames total):
/// - 0-1: Standing front/back
/// - 2-5: Walking front (Down/Right)
/// - 6-9: Walking back (Up/Left)
/// - 10-11: Magic front/back
/// - 12-15: Attack (4 frames)
/// - 16-17: Sitting in chair front/back
/// - 18-19: Sitting on ground front/back
/// - 20-21: Archery front/back
pub fn get_body_armor_frame(
    state: AnimationState,
    direction: Direction,
    anim_frame: u32,
) -> BodyArmorFrameResult {
    let use_back = is_up_or_left_direction(direction);
    let flip_h = should_flip_horizontal(direction);

    let frame = match state {
        AnimationState::Idle => {
            if use_back {
                1
            } else {
                0
            }
        }
        AnimationState::Walking => {
            let frame_in_walk = anim_frame % 4;
            if use_back {
                6 + frame_in_walk // Frames 6-9
            } else {
                2 + frame_in_walk // Frames 2-5
            }
        }
        AnimationState::Attacking => {
            // Attack frame 1: use idle frame, Attack frame 2: use attack frame
            let attack_frame = anim_frame % 2;
            if attack_frame == 0 {
                // Frame 1: use idle
                if use_back {
                    1
                } else {
                    0
                }
            } else {
                // Frame 2: use attack (13 for front, 15 for back)
                if use_back {
                    15
                } else {
                    13
                }
            }
        }
        AnimationState::Casting => {
            // Magic frames 10-11
            if use_back {
                11
            } else {
                10
            }
        }
        AnimationState::ShootingBow => {
            // Archery frames 20-21
            if use_back {
                21
            } else {
                20
            }
        }
        AnimationState::SittingChair => {
            // Sitting chair frames 16-17
            if use_back {
                17
            } else {
                16
            }
        }
        AnimationState::SittingGround => {
            // Sitting ground frames 18-19
            if use_back {
                19
            } else {
                18
            }
        }
    };

    BodyArmorFrameResult { frame, flip_h }
}

/// Get the pixel offset for body armor positioning relative to the player sprite
///
/// Body armor covers the torso and should align with the player's body in each animation frame.
/// These offsets are similar to boots but positioned higher to cover the torso.
/// Gender parameter allows for different positioning based on character model.
pub fn get_body_armor_offset(
    state: AnimationState,
    direction: Direction,
    anim_frame: u32,
    gender: Gender,
) -> (f32, f32) {
    let use_back = is_up_or_left_direction(direction);

    // Base offset: body armor is 34 wide (same as player's 34)
    // Body armor is 77 tall, player is 78, nearly same height
    let base_x = 0.0; // Same width as player, no centering needed
    let base_y = 0.0; // Start at top of player sprite

    // Per-state offsets for alignment (similar to boots but for torso)
    let (state_x, state_y) = match state {
        AnimationState::Idle => {
            // Up/Left: right 1px for up (mirrored left for left), up 2px
            // Down/Right: up 3px (mirrored)
            if use_back {
                (-1.0, -2.0)
            } else {
                (0.0, -3.0)
            }
        }
        AnimationState::Walking => {
            // Up/Left: right 1px for up (mirrored left for left), up 3px
            // Down/Right: up 4px (mirrored)
            if use_back {
                (-1.0, -3.0)
            } else {
                (0.0, -4.0)
            }
        }
        AnimationState::Attacking => {
            let attack_frame = anim_frame % 2;
            if attack_frame == 0 {
                // Frame 1: right 1px for up (mirrored left for left), up 2px
                // Down/Right: up 3px (mirrored)
                if use_back {
                    (-1.0, -2.0)
                } else {
                    (0.0, -3.0)
                }
            } else {
                // Frame 2:
                // Up/Left: right 4px, up 2px for up (mirrored for left)
                // Down/Right: left 2px, up 3px (mirrored for right)
                if use_back {
                    (-4.0, -2.0)
                } else {
                    (-2.0, -3.0)
                }
            }
        }
        AnimationState::Casting => {
            if use_back {
                (-1.0, -2.0)
            } else {
                (0.0, -3.0)
            }
        }
        AnimationState::ShootingBow => {
            // Up/Left: right 1px for up (mirrored left for left), up 2px
            // Down/Right: left 4px, up 3px (mirrored for right)
            if use_back {
                (-1.0, -2.0)
            } else {
                (-4.0, -3.0)
            }
        }
        AnimationState::SittingChair => (0.0, -2.0),
        AnimationState::SittingGround => (0.0, 0.0),
    };

    // Invert x offset when flipped
    let adjusted_state_x = if should_flip_horizontal(direction) {
        -state_x
    } else {
        state_x
    };

    // Gender-specific adjustments for female body armor
    let (gender_adjust_x, gender_adjust_y) = match gender {
        Gender::Male => (0.0, 0.0),
        Gender::Female => {
            // Female body armor needs adjustment for up and left directions
            let x_adj = match direction {
                Direction::Up => -2.0,  // Move 2px to the left
                Direction::Left => 2.0, // Mirror: move 2px to the right (since sprite is flipped)
                _ => 0.0,
            };
            // Female body armor needs to be 2px higher when sitting on chair
            let y_adj = if state == AnimationState::SittingChair {
                -2.0
            } else {
                0.0
            };
            (x_adj, y_adj)
        }
    };

    (
        base_x + adjusted_state_x + gender_adjust_x,
        base_y + state_y + gender_adjust_y,
    )
}

// ============================================================================
// Head Equipment Animation System (simple 2-frame format: front/back)
// ============================================================================

/// Result of head frame calculation
#[derive(Debug, Clone, Copy)]
pub struct HeadFrameResult {
    /// Frame index (0 = front, 1 = back)
    pub frame: u32,
    /// Whether to flip the sprite horizontally
    pub flip_h: bool,
}

/// Get the head equipment frame index for the current direction
///
/// Head sprite sheet layout (0-indexed, 2 frames total):
/// - 0: Front-facing (Down, Right directions)
/// - 1: Back-facing (Up, Left directions)
pub fn get_head_frame(direction: Direction) -> HeadFrameResult {
    let use_back = is_up_or_left_direction(direction);
    let flip_h = should_flip_horizontal(direction);

    HeadFrameResult {
        frame: if use_back { 1 } else { 0 },
        flip_h,
    }
}

/// Get the pixel offset for head equipment positioning relative to the player sprite
///
/// Head equipment is positioned at the top of the player sprite and follows the same
/// offset patterns as hair during animations.
/// Gender parameter allows for different positioning based on character model.
pub fn get_head_offset(
    state: AnimationState,
    direction: Direction,
    anim_frame: u32,
    _gender: Gender,
) -> (f32, f32) {
    let use_back = is_up_or_left_direction(direction);

    // Base offset: head is 30 wide, player is 34, center it
    // Head starts at top of player
    let base_x = 2.0; // (34 - 30) / 2 = 2
    let base_y = -7.0; // Align with top of head, moved up 4px from hair

    // Per-state offsets (mirroring hair offsets)
    // Note: Up/Left directions get -1 X adjustment across all states
    let (state_x, state_y) = match state {
        AnimationState::Idle => {
            if use_back {
                (-3.0, 0.0)
            } else {
                (-1.0, 0.0)
            }
        }
        AnimationState::Walking => {
            if use_back {
                (-3.0, 0.0)
            } else {
                (-1.0, 0.0)
            }
        }
        AnimationState::Attacking => {
            let attack_frame = anim_frame % 2;
            if attack_frame == 0 {
                // Frame 1: same as idle
                if use_back {
                    (-3.0, 0.0)
                } else {
                    (-1.0, 0.0)
                }
            } else {
                // Frame 2: more dramatic shift
                // Up/Left: down 2px from previous, Down/Right: down 1px from previous
                if use_back {
                    (-6.0, 0.0)
                } else {
                    (-6.0, 3.0)
                }
            }
        }
        AnimationState::Casting => {
            if use_back {
                (-3.0, 0.0)
            } else {
                (-2.0, 0.0)
            }
        }
        AnimationState::ShootingBow => {
            if use_back {
                // Up/Left: adjusted positioning (mirrored for Left)
                (-2.0, 0.0)
            } else {
                // Down/Right: move left 1px
                (-2.0, 0.0)
            }
        }
        AnimationState::SittingChair => {
            if use_back {
                (-1.0, 7.0)
            } else {
                (0.0, 7.0)
            }
        }
        AnimationState::SittingGround => {
            if use_back {
                (-1.0, 0.0)
            } else {
                (0.0, 0.0)
            }
        }
    };

    // Invert x offset when flipped
    let adjusted_state_x = if should_flip_horizontal(direction) {
        -state_x
    } else {
        state_x
    };

    // Direction-specific adjustments
    let direction_adjust = match direction {
        Direction::Left => 1.0, // 1px to the right
        Direction::Up => -2.0,  // 2px to the left
        _ => 0.0,
    };

    (
        base_x + adjusted_state_x + direction_adjust,
        base_y + state_y,
    )
}
