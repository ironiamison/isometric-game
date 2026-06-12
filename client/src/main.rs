#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use isometric_client::{run_desktop, window_conf};

#[macroquad::main(window_conf)]
async fn main() {
    run_desktop().await;
}
