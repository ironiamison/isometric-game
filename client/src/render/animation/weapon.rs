use super::*;

// ============================================================================
// Weapon Animation System
// ============================================================================

/// Result of weapon frame calculation
/// Contains frame indices and whether to flip horizontally
#[derive(Debug, Clone, Copy)]
pub struct WeaponFrameResult {
    /// Frame to render under player (always present)
    pub frame_under: u32,
    /// Optional frame to render over player (for attack frame 2 front directions)
    pub frame_over: Option<u32>,
    /// Whether to flip the sprite horizontally
    pub flip_h: bool,
}

/// Get the weapon frame indices for the current animation state and direction
///
/// Weapon sprite sheet layout (0-indexed):
/// - 0: Standing front (Down/Right)
/// - 1: Standing back (Up/Left)
/// - 2-5: Walking front (Down/Right)
/// - 6-9: Walking back (Up/Left)
/// - 10: SpellCast front
/// - 11: SpellCast back
/// - 12: Attack frame 1 front
/// - 13: Attack frame 2 front (rendered UNDER player)
/// - 14: Attack frame 1 back
/// - 15: Attack frame 2 back
/// - 16: Attack frame 2 front overlay (rendered OVER player - hilt erased)
/// - 17: ShootingBow (ranged weapons only)
pub fn get_weapon_frame(
    state: AnimationState,
    direction: Direction,
    anim_frame: u32,
) -> WeaponFrameResult {
    let use_back = is_up_or_left_direction(direction);
    let flip_h = should_flip_horizontal(direction);

    let (frame_under, frame_over) = match state {
        AnimationState::Idle => {
            if use_back {
                (1, None)
            } else {
                (0, None)
            }
        }
        AnimationState::Walking => {
            let frame_in_walk = anim_frame % 4;
            if use_back {
                (6 + frame_in_walk, None)
            } else {
                (2 + frame_in_walk, None)
            }
        }
        AnimationState::Casting => {
            if use_back {
                (11, None)
            } else {
                (10, None)
            }
        }
        AnimationState::Attacking => {
            let attack_frame = anim_frame % 2;
            if use_back {
                // Back attack: frames 14-15, no overlay
                if attack_frame == 0 {
                    (14, None)
                } else {
                    (15, None)
                }
            } else {
                // Front attack: frame 0 uses 12, frame 1 uses 13 under + 16 over
                if attack_frame == 0 {
                    (12, None)
                } else {
                    (13, Some(16))
                }
            }
        }
        AnimationState::ShootingBow => {
            // Use frame 17 for all directions (flipped horizontally for up/left)
            (17, None)
        }
        // Sitting animations don't show weapons
        AnimationState::SittingGround | AnimationState::SittingChair => {
            if use_back {
                (1, None)
            } else {
                (0, None)
            }
        }
    };

    WeaponFrameResult {
        frame_under,
        frame_over,
        flip_h,
    }
}

/// Get the pixel offset for weapon positioning relative to the player sprite
///
/// Returns (x_offset, y_offset) to adjust weapon position based on animation state and direction.
/// These offsets account for the difference in sprite sizes (weapon: 68x84, player: 34x78)
/// and align the weapon with the player's hand position.
/// Gender parameter allows for different positioning based on character model.
///
/// Initial values are conservative defaults - tune visually during testing.
pub fn get_weapon_offset(
    state: AnimationState,
    direction: Direction,
    anim_frame: u32,
    _gender: Gender,
) -> (f32, f32) {
    let use_back = is_up_or_left_direction(direction);

    // Base offset to center the larger weapon sprite over the player
    // Weapon is 68 wide, player is 34, so weapon needs to shift left by (68-34)/2 = 17
    // Weapon is 84 tall, player is 78, so weapon needs to shift up by (84-78)/2 = 3
    let base_x = -17.0;
    let base_y = -3.0;

    // Additional per-state/frame offsets for hand alignment
    let (state_x, state_y) = match state {
        AnimationState::Idle => {
            // Raise weapon to align with hand
            if use_back {
                (-8.0, -8.0)
            } else {
                (-7.0, -6.0)
            }
        }
        AnimationState::Walking => {
            // Raise weapon + slight bounce during walk cycle
            let walk_frame = anim_frame % 4;
            if use_back {
                match walk_frame {
                    0 => (-8.0, -8.0),
                    1 => (-8.0, -9.0),
                    2 => (-8.0, -8.0),
                    3 => (-8.0, -9.0),
                    _ => (-8.0, -8.0),
                }
            } else {
                match walk_frame {
                    0 => (-6.0, -8.0),
                    1 => (-6.0, -9.0),
                    2 => (-6.0, -8.0),
                    3 => (-6.0, -9.0),
                    _ => (-6.0, -8.0),
                }
            }
        }
        AnimationState::Attacking => {
            let attack_frame = anim_frame % 2;
            if use_back {
                // Shift for back-facing attacks
                if attack_frame == 0 {
                    (-7.0, -6.0)
                } else {
                    (-7.0, -6.0)
                }
            } else {
                // Shift left for front-facing attacks
                if attack_frame == 0 {
                    (-5.0, 0.0)
                } else {
                    (-5.0, 0.0)
                }
            }
        }
        AnimationState::Casting => (0.0, 0.0),
        AnimationState::ShootingBow => (-12.0, -4.0), // Shift bow left and up (mirrored for right/up)
        AnimationState::SittingGround | AnimationState::SittingChair => (0.0, 0.0),
    };

    // Invert state x offset when sprite is flipped horizontally to maintain hand alignment
    // (base_x is for centering and stays the same, state_x is for hand position and flips)
    let adjusted_state_x = if should_flip_horizontal(direction) {
        -state_x
    } else {
        state_x
    };

    (base_x + adjusted_state_x, base_y + state_y)
}
