use super::*;

// ============================================================================
// Boot Animation System (single-row sprite format)
// ============================================================================

/// Result of boot frame calculation
#[derive(Debug, Clone, Copy)]
pub struct BootFrameResult {
    /// Frame index (0-based) in the single-row spritesheet
    pub frame: u32,
    /// Whether to flip the sprite horizontally
    pub flip_h: bool,
}

/// Get the boot frame index for the current animation state and direction
///
/// Boot sprite sheet layout (0-indexed frames):
/// - 0: Standing front (Down/Right)
/// - 1: Standing back (Up/Left)
/// - 2-5: Walking front (Down/Right)
/// - 6-9: Walking back (Up/Left)
/// - 10: Attack front
/// - 11: Attack back
/// - 12: Sit chair front
/// - 13: Sit chair back
/// - 14: Sit ground front
/// - 15: Sit ground back
pub fn get_boot_frame(
    state: AnimationState,
    direction: Direction,
    anim_frame: u32,
) -> BootFrameResult {
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
            // Only use attack frame for 2nd attack frame, use idle for 1st
            let attack_frame = anim_frame % 2;
            if attack_frame == 0 {
                // 1st attack frame: use idle
                if use_back {
                    1
                } else {
                    0
                }
            } else {
                // 2nd attack frame: use attack
                if use_back {
                    11
                } else {
                    10
                }
            }
        }
        AnimationState::Casting => {
            // Use standing frame for casting
            if use_back {
                1
            } else {
                0
            }
        }
        AnimationState::ShootingBow => {
            // Use attack frames for shooting
            if use_back {
                11
            } else {
                10
            }
        }
        AnimationState::SittingChair => {
            if use_back {
                13
            } else {
                12
            }
        }
        AnimationState::SittingGround => {
            if use_back {
                15
            } else {
                14
            }
        }
    };

    BootFrameResult { frame, flip_h }
}

/// Get the pixel offset for boot positioning relative to the player sprite
///
/// Boots are positioned at the player's feet. These offsets adjust the boot sprite
/// to align properly with the player's foot position in each animation frame.
/// Gender parameter allows for different positioning based on character model.
pub fn get_boot_offset(
    state: AnimationState,
    direction: Direction,
    anim_frame: u32,
    _gender: Gender,
) -> (f32, f32) {
    let use_back = is_up_or_left_direction(direction);

    // Base offset: boots are 34 wide (same as player), so center them
    // Boot content is ~15px tall within a 27px frame, with 7px padding at top
    // Player feet are at y=67, boot content bottom is at y=21 within frame
    // So: base_y = player_feet(67) - boot_content_bottom(21) = 46
    let base_x = 0.0; // Center boots under player
    let base_y = 46.0; // Align boot content with player feet

    // Per-state offsets for alignment
    let (state_x, state_y) = match state {
        AnimationState::Idle => {
            // Up/Left: down 1, left 1 for left (mirrored for up)
            // Down/Right: no offset
            if use_back {
                (-1.0, 1.0)
            } else {
                (0.0, 0.0)
            }
        }
        AnimationState::Walking => {
            let walk_frame = anim_frame % 4;
            // Down/Right: right 1, down 2 (mirrored for right)
            // Up/Left: slight bounce only
            if use_back {
                match walk_frame {
                    0 => (0.0, 0.0),
                    1 => (0.0, -1.0),
                    2 => (0.0, 0.0),
                    3 => (0.0, -1.0),
                    _ => (0.0, 0.0),
                }
            } else {
                // Down/Right (mirrored for right)
                match walk_frame {
                    0 => (0.0, 1.0),
                    1 => (0.0, 0.0),
                    2 => (0.0, 1.0),
                    3 => (0.0, 0.0),
                    _ => (0.0, 1.0),
                }
            }
        }
        AnimationState::Attacking => {
            // 1st attack frame: use idle offset, 2nd attack frame: custom offsets
            let attack_frame = anim_frame % 2;
            if attack_frame == 0 {
                // 1st frame: same as idle
                if use_back {
                    (-1.0, 1.0)
                } else {
                    (0.0, 0.0)
                }
            } else {
                // 2nd frame: left 3, down 1 for left (mirrored for up); left 4, up 1 for down (mirrored for right)
                if use_back {
                    (-3.0, 1.0)
                } else {
                    (-4.0, -1.0)
                }
            }
        }
        AnimationState::Casting => (-1.0, 1.0),
        AnimationState::ShootingBow => {
            // Same offsets as 2nd attack frame
            if use_back {
                (-3.0, 1.0)
            } else {
                (-4.0, -1.0)
            }
        }
        AnimationState::SittingChair => (0.0, 0.0),
        AnimationState::SittingGround => (0.0, 0.0),
    };

    // Invert x offset when flipped
    let adjusted_state_x = if should_flip_horizontal(direction) {
        -state_x
    } else {
        state_x
    };

    (base_x + adjusted_state_x, base_y + state_y)
}
