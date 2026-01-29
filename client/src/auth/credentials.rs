#[cfg(target_arch = "wasm32")]
mod platform {
    use sapp_jsutils::JsObject;

    extern "C" {
        fn storage_set(key: JsObject, value: JsObject);
        fn storage_get(key: JsObject) -> JsObject;
        fn storage_remove(key: JsObject);
    }

    const KEY: &str = "remembered_username";

    pub fn save_username(username: &str) {
        unsafe {
            storage_set(JsObject::string(KEY), JsObject::string(username));
        }
    }

    pub fn load_username() -> Option<String> {
        unsafe {
            let obj = storage_get(JsObject::string(KEY));
            if obj.is_nil() {
                return None;
            }
            let mut buf = String::new();
            obj.to_string(&mut buf);
            if buf.is_empty() {
                None
            } else {
                Some(buf)
            }
        }
    }

    pub fn clear_username() {
        unsafe {
            storage_remove(JsObject::string(KEY));
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod platform {
    use std::fs;
    use std::path::PathBuf;

    fn config_path() -> PathBuf {
        #[cfg(not(target_os = "android"))]
        let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        #[cfg(target_os = "android")]
        let base = PathBuf::from("/data/data/com.newaeven.game");

        let mut path = base;
        path.push("newaeven");
        path.push("remembered_username.txt");
        path
    }

    pub fn save_username(username: &str) {
        let path = config_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(&path, username);
    }

    pub fn load_username() -> Option<String> {
        let path = config_path();
        fs::read_to_string(&path).ok().filter(|s| !s.is_empty())
    }

    pub fn clear_username() {
        let path = config_path();
        let _ = fs::remove_file(&path);
    }
}

pub use platform::*;
