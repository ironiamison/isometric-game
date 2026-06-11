use super::*;

pub(super) async fn load_atlas(atlas_info: &SpriteAtlasInfo) -> Option<SpriteAtlas> {
    let path = asset_path(&format!("assets/{}", atlas_info.file));
    match load_texture(&path).await {
        Ok(tex) => {
            tex.set_filter(FilterMode::Nearest);
            let rects = atlas_info
                .sprites
                .iter()
                .map(|(key, sr)| {
                    (
                        key.clone(),
                        Rect::new(sr.x as f32, sr.y as f32, sr.w as f32, sr.h as f32),
                    )
                })
                .collect();
            log::info!(
                "Loaded atlas {} ({}x{}, {} sprites)",
                atlas_info.file,
                tex.width(),
                tex.height(),
                atlas_info.sprites.len()
            );
            Some(SpriteAtlas {
                texture: tex,
                rects,
            })
        }
        Err(e) => {
            log::warn!("Failed to load atlas {}: {}", path, e);
            None
        }
    }
}

pub(super) async fn load_spritesheet_atlas(
    atlas_info: &SpriteAtlasInfo,
) -> Option<(Texture2D, HashMap<String, Rect>)> {
    let path = asset_path(&format!("assets/{}", atlas_info.file));
    match load_texture(&path).await {
        Ok(tex) => {
            tex.set_filter(FilterMode::Nearest);
            let rects = atlas_info
                .sprites
                .iter()
                .map(|(key, sr)| {
                    (
                        key.clone(),
                        Rect::new(sr.x as f32, sr.y as f32, sr.w as f32, sr.h as f32),
                    )
                })
                .collect();
            log::info!(
                "Loaded spritesheet atlas {} ({}x{}, {} sprites)",
                atlas_info.file,
                tex.width(),
                tex.height(),
                atlas_info.sprites.len()
            );
            Some((tex, rects))
        }
        Err(e) => {
            log::warn!("Failed to load spritesheet atlas {}: {}", path, e);
            None
        }
    }
}

pub(super) async fn load_individual_sprites(
    items: &[String],
    base: &str,
    loaded: &mut usize,
    total: usize,
    label: &str,
) -> HashMap<String, Texture2D> {
    let mut sprites = HashMap::new();
    for item in items {
        let key = item.rsplit('/').next().unwrap_or(item).to_string();
        let path = asset_path(&format!("{}/{}.png", base, item));
        match load_texture(&path).await {
            Ok(tex) => {
                tex.set_filter(FilterMode::Nearest);
                sprites.insert(key, tex);
            }
            Err(e) => {
                log::warn!("Failed to load sprite {}: {}", path, e);
            }
        }
        *loaded += 1;
        #[cfg(target_arch = "wasm32")]
        Renderer::update_loading(*loaded, total, label);
    }
    log::info!("Loaded {} sprites for {}", sprites.len(), label);
    sprites
}
