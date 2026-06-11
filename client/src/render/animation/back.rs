use super::*;

// ============================================================================
// Back Slot Equipment Animation System
// ============================================================================
// Back slot items come in two varieties:
// 1. Static back items (quiver, cape) - 2 frames: front/back view, minimal animation
// 2. Offhand items (shields) - 16 frames: full animation like boots

// Static back item sprite dimensions (2 frames: front, back)
// normal_quiver.png: 100x63 total, 2 frames = 50x63 each
pub const BACK_STATIC_SPRITE_WIDTH: f32 = 50.0;
pub const BACK_STATIC_SPRITE_HEIGHT: f32 = 63.0;

// Offhand item sprite dimensions (16 frames in single row)
// royal_protector.png: 616x38 total, 16 frames = ~38.5x38 each
pub const OFFHAND_SPRITE_WIDTH: f32 = 38.5;
pub const OFFHAND_SPRITE_HEIGHT: f32 = 38.0;

/// Result of back static frame calculation (quiver, cape, etc.)
#[derive(Debug, Clone, Copy)]
pub struct BackStaticFrameResult {
    /// Frame index (0 = front, 1 = back)
    pub frame: u32,
    /// Whether to flip the sprite horizontally
    pub flip_h: bool,
    /// Whether item should be rendered
    pub visible: bool,
    /// Whether to render behind the player sprite (true for down/right directions)
    pub render_behind: bool,
}

/// Get the back static item frame for the current direction
///
/// Back static sprite sheet layout (0-indexed, 2 frames total):
/// - 0: View when player faces Up/Left (we see their back - full quiver visible)
/// - 1: View when player faces Down/Right (tip peeks out behind player)
///
/// These items sit on the player's back and are always visible (tip shows from front).
pub fn get_back_static_frame(direction: Direction) -> BackStaticFrameResult {
    let use_back_view = is_up_or_left_direction(direction);
    // Flip for Down and Up directions (Right and Left show unflipped)
    let flip_h = matches!(
        direction,
        Direction::Down | Direction::Up | Direction::DownLeft | Direction::UpRight
    );

    BackStaticFrameResult {
        // When player faces up/left (we see their back), use frame 0
        // When player faces down/right, use frame 1 (tip peeking out)
        frame: if use_back_view { 0 } else { 1 },
        flip_h,
        // Always visible - tip shows even from front
        visible: true,
        // Render behind player when facing down/right (tip peeks from behind)
        render_behind: !use_back_view,
    }
}

/// Get the pixel offset for back static item positioning relative to the player sprite
///
/// Back items sit on the player's back/shoulder area. These offsets align the item
/// with the player's back position in each animation frame.
/// Gender parameter allows for different positioning based on character model.
pub fn get_back_static_offset(
    state: AnimationState,
    direction: Direction,
    anim_frame: u32,
    _gender: Gender,
) -> (f32, f32) {
    // Base offset: position varies by direction
    // Quiver is 50 wide, 63 tall
    // Right/Up show on left side (mirrored), Down/Left show on right side
    let (base_x, base_y) = match direction {
        Direction::Up | Direction::UpRight => (-6.0, -10.0),
        Direction::Left | Direction::UpLeft => (-10.0, -10.0),
        Direction::Right | Direction::DownRight => (-16.0, -15.0),
        Direction::Down | Direction::DownLeft => (0.0, -15.0),
    };

    // Per-state offsets (static for idle/walking, only moves for attacks)
    let (state_x, state_y) = match state {
        AnimationState::Idle | AnimationState::Walking => (0.0, 0.0),
        AnimationState::Attacking => {
            // Shift during attack swing
            let attack_frame = anim_frame % 2;
            if attack_frame == 1 {
                (-2.0, 1.0) // Shift with body during swing
            } else {
                (0.0, 0.0)
            }
        }
        AnimationState::Casting => (0.0, 0.0),
        AnimationState::ShootingBow => {
            // Direction-specific shift when drawing bow
            match direction {
                Direction::Up | Direction::UpRight => (-1.0, 0.0),
                Direction::Left | Direction::UpLeft => (1.0, 0.0),
                Direction::Down | Direction::DownLeft => (-1.0, 0.0),
                Direction::Right | Direction::DownRight => (1.0, 0.0),
            }
        }
        AnimationState::SittingChair => (0.0, 7.0),
        AnimationState::SittingGround => (0.0, 0.0),
    };

    // Invert x offset when flipped (skip for ShootingBow which handles direction itself)
    let adjusted_state_x = if state == AnimationState::ShootingBow {
        state_x // Already direction-specific
    } else if should_flip_horizontal(direction) {
        -state_x
    } else {
        state_x
    };

    (base_x + adjusted_state_x, base_y + state_y)
}

/// Result of offhand frame calculation (shields, etc.)
#[derive(Debug, Clone, Copy)]
pub struct OffhandFrameResult {
    /// Frame index (0-based) in the single-row spritesheet
    pub frame: u32,
    /// Whether to flip the sprite horizontally
    pub flip_h: bool,
}

/// Get the offhand item frame index for the current animation state and direction
///
/// Offhand sprite sheet layout (0-indexed, 16 frames total):
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
pub fn get_offhand_frame(
    state: AnimationState,
    direction: Direction,
    anim_frame: u32,
) -> OffhandFrameResult {
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
                if use_back {
                    1
                } else {
                    0
                }
            } else {
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

    OffhandFrameResult { frame, flip_h }
}

/// Get the pixel offset for offhand item positioning relative to the player sprite
///
/// Offhand items (shields) are held on the off-hand side. These offsets align the item
/// with the player's off-hand position in each animation frame.
/// Gender parameter allows for different positioning based on character model.
pub fn get_offhand_offset(
    state: AnimationState,
    direction: Direction,
    anim_frame: u32,
    _gender: Gender,
) -> (f32, f32) {
    let use_back = is_up_or_left_direction(direction);

    // Base offset: position on off-hand side
    // Offhand is ~38 wide, player is 34, center it: (34 - 38) / 2 = -2
    let base_x = -2.0; // Slight offset to off-hand side
    let base_y = 20.0; // Position at arm/torso level (shield is 38 tall)

    // Per-state offsets
    let (state_x, state_y) = match state {
        AnimationState::Idle => {
            if use_back {
                (-1.0, 0.0)
            } else {
                (0.0, 0.0)
            }
        }
        AnimationState::Walking => {
            let walk_frame = anim_frame % 4;
            if use_back {
                match walk_frame {
                    0 => (0.0, 0.0),
                    1 => (0.0, -1.0),
                    2 => (0.0, 0.0),
                    3 => (0.0, -1.0),
                    _ => (0.0, 0.0),
                }
            } else {
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
            let attack_frame = anim_frame % 2;
            if attack_frame == 0 {
                if use_back {
                    (-1.0, 0.0)
                } else {
                    (0.0, 0.0)
                }
            } else {
                // Shield moves during attack
                if use_back {
                    (-3.0, 1.0)
                } else {
                    (-4.0, -1.0)
                }
            }
        }
        AnimationState::Casting => (0.0, 0.0),
        AnimationState::ShootingBow => {
            // Shield on back/side during bow shooting
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

// ============================================================================
// Hair Offset System (gender-aware)
// ============================================================================

/// Hair sprite dimensions
pub const HAIR_SPRITE_WIDTH: f32 = 28.0;
pub const HAIR_SPRITE_HEIGHT: f32 = 54.0;

/// Get the pixel offset for hair positioning relative to the player sprite
///
/// Hair is positioned at the top of the player sprite. These offsets adjust the hair
/// to align properly with the player's head in each animation frame.
/// Gender-specific offsets allow for different head shapes/positions.
pub fn get_hair_offset(
    state: AnimationState,
    direction: Direction,
    anim_frame: u32,
    gender: Gender,
    flip_h: bool,
) -> (f32, f32) {
    let is_back = is_up_or_left_direction(direction);
    let is_attack_frame_2 = state == AnimationState::Attacking && (anim_frame % 2) == 1;
    let is_shooting_bow = state == AnimationState::ShootingBow;
    let is_sitting_chair = state == AnimationState::SittingChair;

    // Base y offset for sitting
    let sit_offset_y = if is_sitting_chair { 7.0 } else { 0.0 };

    let (base_x, base_y) = if is_attack_frame_2 {
        let y_offset = if is_back { -2.0 } else { 2.0 };
        let x_offset = if is_back {
            if flip_h {
                5.0
            } else {
                -5.0
            }
        } else {
            if flip_h {
                6.0
            } else {
                -6.0
            }
        };
        (x_offset, y_offset)
    } else if is_shooting_bow {
        let x_offset = if is_back {
            if flip_h {
                1.0
            } else {
                -1.0
            }
        } else {
            if flip_h {
                2.0
            } else {
                -2.0
            }
        };
        (x_offset, -3.0)
    } else {
        let x_offset = if is_back {
            if flip_h {
                2.0
            } else {
                -2.0
            }
        } else {
            if flip_h {
                1.0
            } else {
                -1.0
            }
        };
        (x_offset, -3.0)
    };

    // Apply gender-specific adjustments
    let (gender_adjust_x, gender_adjust_y) = match gender {
        Gender::Male => (0.0, 0.0),
        Gender::Female => {
            // Female hair needs adjustment for left and up directions
            let (x_adj, y_adj_dir) = match direction {
                Direction::Up => (-2.0, 1.0),  // Move 2px to the left, 1px down
                Direction::Left => (1.0, 0.0), // Mirror: move 1px to the right (since sprite is flipped)
                _ => (0.0, 0.0),
            };
            // Female hair needs to be 3px higher when sitting on chair
            let y_adj_sit = if is_sitting_chair { -3.0 } else { 0.0 };
            (x_adj, y_adj_dir + y_adj_sit)
        }
    };

    (
        base_x + gender_adjust_x,
        base_y + sit_offset_y + gender_adjust_y,
    )
}
