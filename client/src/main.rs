#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Desktop is the only platform that uses this binary entry point. On android and
// wasm the entry point is exported from the library (see lib.rs), and `run_desktop`
// is gated out there, so the bin is reduced to an empty stub to keep `cargo build`
// working across every target.
#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
use isometric_client::{run_desktop, window_conf};

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
#[macroquad::main(window_conf)]
async fn main() {
    run_desktop().await;
}

#[cfg(any(target_arch = "wasm32", target_os = "android"))]
fn main() {}
