//! Custom shaders for equipment rendering

/// Vertex shader for head+hair composite rendering
pub const HEAD_HAIR_VERTEX: &str = r#"#version 100
precision lowp float;

attribute vec3 position;
attribute vec2 texcoord;

varying vec2 uv;

uniform mat4 Model;
uniform mat4 Projection;

void main() {
    gl_Position = Projection * Model * vec4(position, 1);
    uv = texcoord;
}
"#;

/// Fragment shader for head+hair composite rendering
///
/// This shader composites hair and head sprites based on special pixel values:
/// - Fully transparent pixels in head: hair is cut off (not visible)
/// - RGB (8, 0, 0) pixels in head: hair shows through
/// - Any other pixels: head is drawn (covers hair)
///
/// Uniforms:
/// - Texture: Head sprite sheet (primary)
/// - HairTexture: Hair sprite sheet
/// - HairUvTransform: vec4(offset_x, offset_y, scale_x, scale_y) to transform head UV to hair UV
/// - Tint: vec4 color tint to apply (default white with full alpha for normal rendering)
/// Vertex shader for animated water tiles
pub const WATER_VERTEX: &str = r#"#version 100
precision lowp float;

attribute vec3 position;
attribute vec2 texcoord;

varying vec2 uv;

uniform mat4 Model;
uniform mat4 Projection;

void main() {
    gl_Position = Projection * Model * vec4(position, 1);
    uv = texcoord;
}
"#;

/// Fragment shader for base water tiles — subtle color shimmer only, no UV distortion.
pub const WATER_FRAGMENT: &str = r#"#version 100
precision lowp float;

varying vec2 uv;

uniform sampler2D Texture;
uniform float Time;

void main() {
    vec4 color = texture2D(Texture, uv);

    if (color.a < 0.01) {
        discard;
    }

    // Subtle brightness shimmer
    float shimmer = sin(uv.x * 8.0 + uv.y * 6.0 + Time * 1.5) * 0.03;
    color.rgb += shimmer;

    // Slight blue tint shift
    color.b += sin(Time * 0.7) * 0.01;

    gl_FragColor = color;
}
"#;

/// Fragment shader for the wave overlay drawn on top of water tiles.
/// Uses world position so waves are continuous across tile boundaries.
///
/// Uniforms:
/// - Texture: tileset texture (used only for diamond alpha mask)
/// - Time: elapsed seconds
/// - WorldPos: vec2(world_x, world_y) tile coordinates
pub const WATER_OVERLAY_FRAGMENT: &str = r#"#version 100
precision lowp float;

varying vec2 uv;

uniform sampler2D Texture;
uniform float Time;
uniform vec2 WorldPos;

void main() {
    // Use base tile alpha as diamond mask
    float mask = texture2D(Texture, uv).a;
    if (mask < 0.01) {
        discard;
    }

    // Convert local UV to continuous world UV so waves span across tiles
    vec2 wuv = WorldPos + uv;

    // Layer 1: broad slow wave bands moving diagonally
    float wave1 = sin(wuv.x * 3.0 + wuv.y * 2.0 + Time * 0.3) * 0.5 + 0.5;
    wave1 = smoothstep(0.55, 0.75, wave1) * 0.06;

    // Layer 2: finer ripples moving the other direction
    float wave2 = sin(wuv.x * 6.0 - wuv.y * 4.0 + Time * 0.5) * 0.5 + 0.5;
    wave2 = smoothstep(0.6, 0.8, wave2) * 0.04;

    // Layer 3: very fine sparkle/caustic dots
    float caustic = sin(wuv.x * 10.0 + Time * 0.4) * sin(wuv.y * 8.0 - Time * 0.25);
    caustic = smoothstep(0.75, 0.92, caustic) * 0.05;

    float brightness = wave1 + wave2 + caustic;

    // White-ish highlight with low opacity
    gl_FragColor = vec4(0.8, 0.85, 1.0, brightness * mask);
}
"#;

pub const HEAD_HAIR_FRAGMENT: &str = r#"#version 100
precision lowp float;

varying vec2 uv;

uniform sampler2D Texture;      // Head sprite (with source rect already applied by macroquad)
uniform sampler2D HairTexture;  // Hair sprite (full texture, we compute UV)
uniform vec4 HairUvTransform;   // (offset_x, offset_y, scale_x, scale_y)
uniform vec4 Tint;              // Color tint (default: 1,1,1,1 for no tint)

void main() {
    vec4 head = texture2D(Texture, uv);

    // Transform UV from head space to hair space
    vec2 hair_uv = HairUvTransform.xy + uv * HairUvTransform.zw;

    // Check if hair UV is in valid range [0,1]
    bool hair_in_bounds = hair_uv.x >= 0.0 && hair_uv.x <= 1.0 && hair_uv.y >= 0.0 && hair_uv.y <= 1.0;

    vec4 hair = vec4(0.0);
    if (hair_in_bounds) {
        hair = texture2D(HairTexture, hair_uv);
    }

    // Detect (8,0,0) "show hair" marker
    // 8/255 = 0.0314, check with wider tolerance
    bool is_hair_marker = head.r > 0.01 && head.r < 0.08
                       && head.g < 0.02
                       && head.b < 0.02
                       && head.a > 0.5;

    if (is_hair_marker) {
        // Hair marker detected - show hair if in bounds, otherwise discard
        if (hair_in_bounds && hair.a > 0.01) {
            gl_FragColor = hair * Tint;
        } else {
            discard;  // No hair here, discard the marker pixel
        }
    } else if (head.a > 0.01) {
        gl_FragColor = head * Tint;    // Show head (opaque non-marker pixels)
    } else {
        discard;                       // Transparent = discard
    }
}
"#;
