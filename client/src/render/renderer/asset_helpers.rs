use super::*;

impl Renderer {
    pub(super) fn build_animated_map(atlas_info: &Option<SpriteAtlasInfo>) -> HashMap<u32, u32> {
        let mut map = HashMap::new();
        if let Some(info) = atlas_info {
            for (key, sr) in &info.sprites {
                if let Some(frames) = sr.frames {
                    if frames > 1 {
                        if let Ok(id) = key.parse::<u32>() {
                            map.insert(id, frames);
                        }
                    }
                }
            }
        }
        map
    }

    /// Detect which NPC sprites have non-transparent second idle frames.
    /// Frame 1 is the 2nd idle frame for down/right, frame 3 is for up/left.
    /// If either frame contains any non-transparent pixels, the NPC has an idle animation.
    pub(super) fn detect_npc_idle_animations(npc_sprites: &SpritesheetStore) -> HashSet<String> {
        let mut set = HashSet::new();

        // Get the image data we need to sample pixels from
        let (image, keys_and_rects): (Option<Image>, Vec<(String, f32, f32, f32, f32)>) =
            match npc_sprites {
                SpritesheetStore::Atlas { texture, rects } => {
                    let img = texture.get_texture_data();
                    let entries: Vec<_> = rects
                        .iter()
                        .map(|(key, rect)| (key.clone(), rect.x, rect.y, rect.w, rect.h))
                        .collect();
                    (Some(img), entries)
                }
                SpritesheetStore::Individual(map) => {
                    // For individual textures, check each one separately
                    for (key, tex) in map {
                        let w = tex.width();
                        let h = tex.height();
                        let frame_w = w / 16.0;
                        let img = tex.get_texture_data();
                        if Self::frame_has_visible_pixels(
                            &img,
                            frame_w as u32 * 1,
                            0,
                            frame_w as u32,
                            h as u32,
                        ) || Self::frame_has_visible_pixels(
                            &img,
                            frame_w as u32 * 3,
                            0,
                            frame_w as u32,
                            h as u32,
                        ) {
                            set.insert(key.clone());
                        }
                    }
                    return set;
                }
            };

        if let Some(ref img) = image {
            for (key, atlas_x, atlas_y, w, h) in &keys_and_rects {
                let frame_w = (*w / 16.0) as u32;
                let ax = *atlas_x as u32;
                let ay = *atlas_y as u32;
                let fh = *h as u32;
                // Check frame 1 (2nd idle down/right) and frame 3 (2nd idle up/left)
                if Self::frame_has_visible_pixels(img, ax + frame_w * 1, ay, frame_w, fh)
                    || Self::frame_has_visible_pixels(img, ax + frame_w * 3, ay, frame_w, fh)
                {
                    set.insert(key.clone());
                }
            }
        }

        set
    }

    /// Check if a rectangular region of an image contains any non-transparent pixel.
    /// Returns false only if every pixel in the region has zero alpha.
    pub(super) fn frame_has_visible_pixels(img: &Image, x: u32, y: u32, w: u32, h: u32) -> bool {
        let img_w = img.width() as u32;
        let img_h = img.height() as u32;
        for py in 0..h {
            for px in 0..w {
                let sx = x + px;
                let sy = y + py;
                if sx < img_w && sy < img_h {
                    if img.get_pixel(sx, sy).a > 0.0 {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Get the animation source rect for a multi-frame sprite.
    /// Divides the full rect into `frames` equal horizontal slices and cycles through them.
    pub(super) fn get_animated_source_rect(
        source_rect: Option<Rect>,
        frames: u32,
    ) -> (Option<Rect>, f32) {
        if let Some(r) = source_rect {
            let frame_w = r.w / frames as f32;
            let fps = 4.0_f64; // ~4 FPS for ambient animations
            let frame_idx = ((get_time() * fps) as u32 % frames) as f32;
            (
                Some(Rect::new(r.x + frame_idx * frame_w, r.y, frame_w, r.h)),
                frame_w,
            )
        } else {
            (None, 0.0)
        }
    }
}
