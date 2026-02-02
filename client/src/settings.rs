/// Settings persistence for control scheme (classic vs modern)

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
fn controls_path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|p| p.join("new-aeven").join("controls.toml"))
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
fn control_scheme_chosen_path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|p| p.join("new-aeven").join("control_scheme_chosen"))
}

#[cfg(target_os = "android")]
fn control_scheme_chosen_path() -> Option<std::path::PathBuf> {
    Some(std::path::PathBuf::from("control_scheme_chosen"))
}

#[cfg(target_os = "android")]
fn controls_path() -> Option<std::path::PathBuf> {
    Some(std::path::PathBuf::from("controls_settings.toml"))
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
pub fn load_classic_controls() -> bool {
    let Some(path) = controls_path() else {
        return false;
    };
    match std::fs::read_to_string(&path) {
        Ok(contents) => contents.trim() == "classic = true",
        Err(_) => false,
    }
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
pub fn save_classic_controls(classic: bool) {
    let Some(path) = controls_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let contents = if classic { "classic = true\n" } else { "classic = false\n" };
    let _ = std::fs::write(&path, contents);
}

#[cfg(target_os = "android")]
pub fn load_classic_controls() -> bool {
    let Some(path) = controls_path() else {
        return false;
    };
    match std::fs::read_to_string(&path) {
        Ok(contents) => contents.trim() == "classic = true",
        Err(_) => false,
    }
}

#[cfg(target_os = "android")]
pub fn save_classic_controls(classic: bool) {
    let Some(path) = controls_path() else {
        return;
    };
    let contents = if classic { "classic = true\n" } else { "classic = false\n" };
    let _ = std::fs::write(&path, contents);
}

#[cfg(target_arch = "wasm32")]
pub fn load_classic_controls() -> bool {
    quad_storage::STORAGE
        .lock()
        .expect("storage lock")
        .get("classic_controls")
        .map(|s| s == "true")
        .unwrap_or(false)
}

#[cfg(target_arch = "wasm32")]
pub fn save_classic_controls(classic: bool) {
    quad_storage::STORAGE
        .lock()
        .expect("storage lock")
        .set("classic_controls", if classic { "true" } else { "false" });
}

// --- Control scheme chosen persistence ---

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
pub fn load_control_scheme_chosen() -> bool {
    let Some(path) = control_scheme_chosen_path() else {
        return false;
    };
    path.exists()
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
pub fn save_control_scheme_chosen() {
    let Some(path) = control_scheme_chosen_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, "true");
}

#[cfg(target_os = "android")]
pub fn load_control_scheme_chosen() -> bool {
    let Some(path) = control_scheme_chosen_path() else {
        return false;
    };
    path.exists()
}

#[cfg(target_os = "android")]
pub fn save_control_scheme_chosen() {
    let Some(path) = control_scheme_chosen_path() else {
        return;
    };
    let _ = std::fs::write(&path, "true");
}

#[cfg(target_arch = "wasm32")]
pub fn load_control_scheme_chosen() -> bool {
    quad_storage::STORAGE
        .lock()
        .expect("storage lock")
        .get("control_scheme_chosen")
        .is_some()
}

#[cfg(target_arch = "wasm32")]
pub fn save_control_scheme_chosen() {
    quad_storage::STORAGE
        .lock()
        .expect("storage lock")
        .set("control_scheme_chosen", "true");
}
