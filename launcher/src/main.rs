#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashMap;
use std::env;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use directories::ProjectDirs;
use egui::TextureHandle;
use serde::{Deserialize, Serialize};
use sha2::Digest;

const DEFAULT_BASE_URL: &str = "https://example.invalid/launcher";
const DEFAULT_MANIFEST_PATH: &str = "manifest.json";
const DEFAULT_SERVER_URL: &str = "https://aeven.xyz";
const STATS_REFRESH_SECS: u64 = 30;

// -- Theme colors matching the game client --
const BG_DARK: egui::Color32 = egui::Color32::from_rgb(0x1e, 0x1e, 0x28);
const BG_PANEL: egui::Color32 = egui::Color32::from_rgb(0x28, 0x28, 0x36);
const BG_HEADER: egui::Color32 = egui::Color32::from_rgb(0x32, 0x32, 0x44);
const ACCENT_GREEN: egui::Color32 = egui::Color32::from_rgb(0x64, 0xc8, 0x78);
const ACCENT_GREEN_DIM: egui::Color32 = egui::Color32::from_rgb(0x3a, 0x7a, 0x48);
const TEXT_PRIMARY: egui::Color32 = egui::Color32::from_rgb(0xee, 0xee, 0xf0);
const TEXT_DIM: egui::Color32 = egui::Color32::from_rgb(0x8c, 0x8c, 0xa0);
const TEXT_MUTED: egui::Color32 = egui::Color32::from_rgb(0x60, 0x60, 0x74);
const BORDER_COLOR: egui::Color32 = egui::Color32::from_rgb(0x3c, 0x3c, 0x50);
const ERROR_RED: egui::Color32 = egui::Color32::from_rgb(0xe0, 0x50, 0x50);
const GOLD: egui::Color32 = egui::Color32::from_rgb(0xd4, 0xaa, 0x40);
const SILVER: egui::Color32 = egui::Color32::from_rgb(0xb0, 0xb0, 0xc0);
const BRONZE: egui::Color32 = egui::Color32::from_rgb(0xb0, 0x80, 0x50);
const ONLINE_GREEN: egui::Color32 = egui::Color32::from_rgb(0x50, 0xe8, 0x60);

// ── Config ──────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
struct LauncherConfig {
    base_url: String,
    manifest_path: Option<String>,
    app_name: Option<String>,
    server_url: Option<String>,
}

impl LauncherConfig {
    fn load() -> Self {
        let mut candidates: Vec<PathBuf> = Vec::new();
        if let Ok(path) = env::var("LAUNCHER_CONFIG") {
            candidates.push(PathBuf::from(path));
        }
        if let Ok(exe_path) = env::current_exe() {
            if let Some(dir) = exe_path.parent() {
                candidates.push(dir.join("launcher-config.toml"));
            }
        }
        candidates.push(PathBuf::from("launcher-config.toml"));

        for path in candidates {
            if let Ok(contents) = fs::read_to_string(&path) {
                if let Ok(mut config) = toml::from_str::<LauncherConfig>(&contents) {
                    if let Ok(base) = env::var("LAUNCHER_BASE_URL") {
                        config.base_url = base;
                    }
                    return config;
                }
            }
        }

        LauncherConfig {
            base_url: env::var("LAUNCHER_BASE_URL")
                .unwrap_or_else(|_| DEFAULT_BASE_URL.to_string()),
            manifest_path: Some(DEFAULT_MANIFEST_PATH.to_string()),
            app_name: Some("New Aeven".to_string()),
            server_url: Some(DEFAULT_SERVER_URL.to_string()),
        }
    }

    fn manifest_url(&self) -> String {
        let manifest_path = self
            .manifest_path
            .as_deref()
            .unwrap_or(DEFAULT_MANIFEST_PATH);
        let mut base = self.base_url.trim_end_matches('/').to_string();
        base.push('/');
        base.push_str(manifest_path.trim_start_matches('/'));
        base
    }

    fn server_url(&self) -> &str {
        self.server_url.as_deref().unwrap_or(DEFAULT_SERVER_URL)
    }
}

// ── User settings (persisted) ───────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Default)]
struct UserSettings {
    close_on_launch: bool,
}

impl UserSettings {
    fn path() -> Option<PathBuf> {
        ProjectDirs::from("com", "New Aeven", "new_aeven")
            .map(|d| d.data_dir().join("launcher-settings.json"))
    }

    fn load() -> Self {
        Self::path()
            .and_then(|p| fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save(&self) {
        if let Some(path) = Self::path() {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string(self) {
                let _ = fs::write(path, json);
            }
        }
    }
}

// ── Manifest types ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct Manifest {
    version: String,
    platforms: HashMap<String, PlatformManifest>,
}

#[derive(Debug, Deserialize)]
struct PlatformManifest {
    entrypoint: String,
    files: Vec<FileEntry>,
}

#[derive(Debug, Deserialize, Clone)]
struct FileEntry {
    path: String,
    sha256: String,
    size: u64,
    url: Option<String>,
    executable: Option<bool>,
}

// ── Stats API types ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone, Default)]
struct StatsOverview {
    online_players: usize,
    total_characters: i64,
    total_accounts: i64,
}

#[derive(Debug, Deserialize, Clone)]
struct LeaderboardEntry {
    name: String,
    combat_level: i32,
    total_level: i32,
    monster_kills: i32,
}

#[derive(Debug, Clone, Default)]
struct LiveStats {
    overview: StatsOverview,
    leaderboard: Vec<LeaderboardEntry>,
    last_fetched: Option<Instant>,
    error: Option<String>,
}

// ── Update channel messages ─────────────────────────────────────────────────

#[derive(Debug)]
enum UpdateMsg {
    Status(String),
    Progress { downloaded: u64, total: u64 },
    Version(String),
    Done { entrypoint: PathBuf, work_dir: PathBuf },
    Error(String),
}

// ── Main app state ──────────────────────────────────────────────────────────

struct LauncherApp {
    status: String,
    progress: Option<(u64, u64)>,
    last_error: Option<String>,
    rx: Option<mpsc::Receiver<UpdateMsg>>,
    logo_bytes: Option<Vec<u8>>,
    logo_texture: Option<TextureHandle>,
    auto_launch: bool,
    launch_done: bool,
    ready_to_launch: Option<(PathBuf, PathBuf)>,
    client_version: Option<String>,
    stats: Arc<Mutex<LiveStats>>,
    close_on_launch: bool,
    pending_close: bool,
    styled: bool,
}

impl LauncherApp {
    fn new(config: &LauncherConfig) -> Self {
        let logo_bytes = load_logo_bytes();
        let stats = Arc::new(Mutex::new(LiveStats::default()));
        let settings = UserSettings::load();

        let stats_ref = Arc::clone(&stats);
        let server_url = config.server_url().to_string();
        thread::spawn(move || stats_poll_loop(server_url, stats_ref));

        let mut app = Self {
            status: "Checking for updates...".to_string(),
            progress: None,
            last_error: None,
            rx: None,
            logo_bytes,
            logo_texture: None,
            auto_launch: true,
            launch_done: false,
            ready_to_launch: None,
            client_version: None,
            stats,
            close_on_launch: settings.close_on_launch,
            pending_close: false,
            styled: false,
        };
        app.start_update();
        app
    }

    fn start_update(&mut self) {
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        self.status = "Checking for updates...".to_string();
        self.progress = None;
        self.last_error = None;
        self.launch_done = false;
        self.ready_to_launch = None;

        thread::spawn(move || {
            if let Err(err) = run_update(tx.clone()) {
                let _ = tx.send(UpdateMsg::Error(err));
            }
        });
    }

    fn handle_messages(&mut self) {
        let Some(rx) = &self.rx else { return };
        while let Ok(msg) = rx.try_recv() {
            match msg {
                UpdateMsg::Status(text) => self.status = text,
                UpdateMsg::Progress { downloaded, total } => {
                    self.progress = Some((downloaded, total));
                }
                UpdateMsg::Version(v) => self.client_version = Some(v),
                UpdateMsg::Done { entrypoint, work_dir } => {
                    self.progress = None;
                    if !self.launch_done && self.auto_launch {
                        match launch_client(&entrypoint, &work_dir) {
                            Ok(()) => {
                                self.status = "Client launched!".to_string();
                                self.launch_done = true;
                                self.ready_to_launch = Some((entrypoint, work_dir));
                                if self.close_on_launch {
                                    self.pending_close = true;
                                }
                            }
                            Err(err) => {
                                self.status = "Launch failed.".to_string();
                                self.last_error = Some(err);
                            }
                        }
                    } else {
                        self.status = "Ready to play".to_string();
                        self.ready_to_launch = Some((entrypoint, work_dir));
                    }
                }
                UpdateMsg::Error(err) => {
                    self.status = "Update failed.".to_string();
                    self.last_error = Some(err);
                }
            }
        }
    }

    fn ensure_logo_texture(&mut self, ctx: &egui::Context) {
        if self.logo_texture.is_some() {
            return;
        }
        if let Some(bytes) = self.logo_bytes.take() {
            if let Ok(img) = image::load_from_memory(&bytes) {
                let rgba = img.to_rgba8();
                let size = [rgba.width() as usize, rgba.height() as usize];
                let pixels = rgba.into_raw();
                let color_image =
                    egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
                self.logo_texture = Some(ctx.load_texture(
                    "logo",
                    color_image,
                    egui::TextureOptions::NEAREST,
                ));
            }
        }
    }

    fn apply_theme(&mut self, ctx: &egui::Context) {
        if self.styled {
            return;
        }
        self.styled = true;

        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = BG_DARK;
        visuals.window_fill = BG_DARK;
        visuals.extreme_bg_color = BG_PANEL;
        visuals.faint_bg_color = BG_PANEL;

        visuals.widgets.noninteractive.bg_fill = BG_PANEL;
        visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, TEXT_DIM);
        visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, BORDER_COLOR);
        visuals.widgets.inactive.bg_fill = BG_HEADER;
        visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, TEXT_PRIMARY);
        visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, BORDER_COLOR);
        visuals.widgets.hovered.bg_fill = ACCENT_GREEN_DIM;
        visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, TEXT_PRIMARY);
        visuals.widgets.active.bg_fill = ACCENT_GREEN;
        visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, BG_DARK);
        visuals.selection.bg_fill = ACCENT_GREEN_DIM;
        visuals.selection.stroke = egui::Stroke::new(1.0, ACCENT_GREEN);

        ctx.set_visuals(visuals);
    }
}

impl eframe::App for LauncherApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.apply_theme(ctx);
        self.ensure_logo_texture(ctx);
        self.handle_messages();
        if self.pending_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        let stats = self.stats.lock().unwrap().clone();

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(BG_DARK).inner_margin(0.0))
            .show(ctx, |ui| {
                let w = ui.available_width();

                // ── Close button (top-right X) ──
                let close_rect = egui::Rect::from_min_size(
                    egui::pos2(w - 28.0, 4.0),
                    egui::vec2(24.0, 24.0),
                );
                let close_resp = ui.allocate_rect(close_rect, egui::Sense::click());
                let x_color = if close_resp.hovered() { TEXT_PRIMARY } else { TEXT_MUTED };
                let c = close_rect.center();
                let d = 5.0;
                ui.painter().line_segment([egui::pos2(c.x - d, c.y - d), egui::pos2(c.x + d, c.y + d)], egui::Stroke::new(1.5, x_color));
                ui.painter().line_segment([egui::pos2(c.x + d, c.y - d), egui::pos2(c.x - d, c.y + d)], egui::Stroke::new(1.5, x_color));
                if close_resp.clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }

                // ── 1. Logo + version ──
                ui.vertical_centered(|ui| {
                    ui.add_space(2.0);
                    if let Some(tex) = &self.logo_texture {
                        let sz = tex.size_vec2() * 0.65;
                        ui.image(egui::load::SizedTexture::new(tex.id(), sz));
                    } else {
                        ui.colored_label(
                            TEXT_PRIMARY,
                            egui::RichText::new("New Aeven").size(20.0).strong(),
                        );
                    }
                    ui.add_space(2.0);
                    if let Some(ver) = &self.client_version {
                        ui.colored_label(TEXT_MUTED, egui::RichText::new(format!("v{ver}")).size(11.0));
                    }
                });

                // ── 2. Status + progress + buttons (below logo, above leaderboard) ──
                ui.add_space(4.0);
                ui.vertical_centered(|ui| {
                    ui.colored_label(TEXT_DIM, egui::RichText::new(&self.status).size(12.0));

                    if let Some((downloaded, total)) = self.progress {
                        ui.add_space(4.0);
                        let frac = if total > 0 { downloaded as f32 / total as f32 } else { 0.0 };
                        let bar_w = w - 40.0;
                        let bar_h = 14.0;
                        let (bar_rect, _) = ui.allocate_exact_size(egui::vec2(bar_w, bar_h), egui::Sense::hover());
                        ui.painter().rect_filled(bar_rect, 4.0, BG_HEADER);
                        let fill = egui::Rect::from_min_size(bar_rect.min, egui::vec2(bar_w * frac, bar_h));
                        ui.painter().rect_filled(fill, 4.0, ACCENT_GREEN);
                        ui.painter().text(
                            bar_rect.center(), egui::Align2::CENTER_CENTER,
                            format!("{:.0}%", frac * 100.0), egui::FontId::proportional(9.0),
                            if frac > 0.5 { BG_DARK } else { TEXT_PRIMARY },
                        );
                        ui.add_space(2.0);
                        ui.colored_label(TEXT_MUTED, egui::RichText::new(format!(
                            "{} / {} MB", bytes_to_mb(downloaded), bytes_to_mb(total)
                        )).size(10.0));
                    }

                    if let Some(err) = &self.last_error {
                        ui.add_space(4.0);
                        ui.colored_label(ERROR_RED, egui::RichText::new(err).size(11.0));
                    }

                    ui.add_space(6.0);

                    // Button
                    let bw = 200.0;
                    if self.last_error.is_some() {
                        let b = egui::Button::new(egui::RichText::new("RETRY").size(14.0).strong().color(BG_DARK))
                            .fill(ACCENT_GREEN).corner_radius(4.0).min_size(egui::vec2(bw, 36.0));
                        if ui.add(b).clicked() { self.start_update(); }
                    } else if self.ready_to_launch.is_some() {
                        let b = egui::Button::new(egui::RichText::new("PLAY").size(14.0).strong().color(ACCENT_GREEN))
                            .fill(ACCENT_GREEN_DIM).stroke(egui::Stroke::new(1.5, ACCENT_GREEN))
                            .corner_radius(4.0).min_size(egui::vec2(bw, 36.0));
                        if ui.add(b).clicked() {
                            if let Some((ref ep, ref wd)) = self.ready_to_launch {
                                if launch_client(ep, wd).is_ok() && self.close_on_launch {
                                    self.pending_close = true;
                                }
                            }
                        }
                    } else {
                        let b = egui::Button::new(egui::RichText::new("UPDATING...").size(14.0).strong().color(TEXT_MUTED))
                            .fill(BG_HEADER).corner_radius(4.0).min_size(egui::vec2(bw, 36.0));
                        ui.add_enabled(false, b);
                    }

                    ui.add_space(6.0);

                    // Custom styled checkbox (centered, whole row clickable)
                    let prev = self.close_on_launch;
                    let cb_text = "Close launcher when game opens";
                    let box_size = 14.0;
                    let gap = 6.0;
                    let text_galley = ui.painter().layout_no_wrap(cb_text.to_string(), egui::FontId::proportional(11.0), TEXT_DIM);
                    let row_w = box_size + gap + text_galley.size().x;
                    let row_h = box_size.max(text_galley.size().y);
                    let (row_rect, resp) = ui.allocate_exact_size(egui::vec2(row_w, row_h), egui::Sense::click());
                    if resp.clicked() {
                        self.close_on_launch = !self.close_on_launch;
                    }
                    let box_rect = egui::Rect::from_min_size(
                        egui::pos2(row_rect.left(), row_rect.center().y - box_size * 0.5),
                        egui::vec2(box_size, box_size),
                    );
                    let border = if resp.hovered() { ACCENT_GREEN } else { TEXT_MUTED };
                    ui.painter().rect_stroke(box_rect, 2.0, egui::Stroke::new(1.0, border), egui::StrokeKind::Inside);
                    if self.close_on_launch {
                        ui.painter().rect_filled(box_rect.shrink(3.0), 1.0, ACCENT_GREEN);
                    }
                    let label_color = if resp.hovered() { TEXT_PRIMARY } else { TEXT_DIM };
                    ui.painter().text(
                        egui::pos2(box_rect.right() + gap, row_rect.center().y),
                        egui::Align2::LEFT_CENTER, cb_text, egui::FontId::proportional(11.0), label_color,
                    );
                    if self.close_on_launch != prev {
                        UserSettings { close_on_launch: self.close_on_launch }.save();
                    }
                });

                ui.add_space(6.0);

                // ── 3. Separator ──
                let sep = egui::Rect::from_min_size(
                    egui::pos2(16.0, ui.cursor().top()),
                    egui::vec2(w - 32.0, 1.0),
                );
                ui.painter().rect_filled(sep, 0.0, BORDER_COLOR);
                ui.add_space(6.0);

                // ── 4. Online count ──
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    let dot_center = egui::pos2(ui.cursor().left() + 5.0, ui.cursor().top() + 9.0);
                    ui.painter().circle_filled(
                        dot_center, 4.0,
                        if stats.overview.online_players > 0 { ONLINE_GREEN } else { TEXT_MUTED },
                    );
                    ui.add_space(14.0);
                    ui.colored_label(TEXT_PRIMARY, egui::RichText::new(format!(
                        "{} online", stats.overview.online_players
                    )).size(13.0).strong());
                    ui.add_space(16.0);
                    ui.colored_label(TEXT_DIM, egui::RichText::new(format!(
                        "{} adventurers  ·  {} accounts",
                        stats.overview.total_characters, stats.overview.total_accounts
                    )).size(11.0));
                });

                ui.add_space(6.0);

                // ── 5. Leaderboard (fills remaining space, clipped) ──
                let margin = 20.0;
                let remaining = (ui.available_height() - 16.0).max(80.0);
                let lb_rect = egui::Rect::from_min_size(
                    egui::pos2(margin, ui.cursor().top()),
                    egui::vec2(w - margin * 2.0, remaining),
                );
                ui.painter().rect_filled(lb_rect, 6.0, BG_PANEL);
                ui.painter().rect_stroke(lb_rect, 6.0, egui::Stroke::new(1.0, BORDER_COLOR), egui::StrokeKind::Outside);

                // Reserve the space in the parent layout
                ui.allocate_rect(lb_rect, egui::Sense::hover());

                // Clipped child UI inside the panel
                let content = lb_rect.shrink(10.0);
                let mut lb = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(content)
                        .layout(egui::Layout::top_down(egui::Align::LEFT)),
                );
                lb.set_clip_rect(lb_rect);

                // Leaderboard title
                lb.horizontal(|ui| {
                    ui.colored_label(ACCENT_GREEN, egui::RichText::new("LEADERBOARD").size(11.0).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.colored_label(TEXT_MUTED, egui::RichText::new("Top Players by Total Level").size(10.0));
                    });
                });
                lb.add_space(4.0);

                // Fixed column x-coordinates for pixel-perfect alignment
                let col_rank_x = content.left() + 8.0;
                let col_name_x = content.left() + 26.0;
                let col_kills_x = content.right() - 8.0;
                let col_cmbt_x = col_kills_x - 58.0;
                let col_total_x = col_cmbt_x - 58.0;
                let row_h = 20.0;
                let hdr_font = egui::FontId::proportional(10.0);
                let row_font = egui::FontId::proportional(11.0);

                // Column headers
                let hy = lb.cursor().top();
                let hr = egui::Rect::from_min_size(egui::pos2(content.left(), hy), egui::vec2(content.width(), row_h));
                lb.painter().rect_filled(hr, 2.0, BG_HEADER);
                let hcy = hy + row_h * 0.5;
                lb.painter().text(egui::pos2(col_rank_x, hcy), egui::Align2::LEFT_CENTER, "#", hdr_font.clone(), TEXT_MUTED);
                lb.painter().text(egui::pos2(col_name_x, hcy), egui::Align2::LEFT_CENTER, "Name", hdr_font.clone(), TEXT_MUTED);
                lb.painter().text(egui::pos2(col_total_x, hcy), egui::Align2::RIGHT_CENTER, "Total", hdr_font.clone(), TEXT_MUTED);
                lb.painter().text(egui::pos2(col_cmbt_x, hcy), egui::Align2::RIGHT_CENTER, "Cmbt", hdr_font.clone(), TEXT_MUTED);
                lb.painter().text(egui::pos2(col_kills_x, hcy), egui::Align2::RIGHT_CENTER, "Kills", hdr_font, TEXT_MUTED);
                lb.allocate_space(egui::vec2(content.width(), row_h));
                lb.add_space(2.0);

                // Rows
                if stats.leaderboard.is_empty() {
                    lb.add_space(20.0);
                    lb.vertical_centered(|ui| {
                        ui.colored_label(TEXT_MUTED, egui::RichText::new(
                            if stats.error.is_some() { "Could not load leaderboard" } else { "Loading..." }
                        ).size(11.0));
                    });
                } else {
                    for (i, entry) in stats.leaderboard.iter().take(7).enumerate() {
                        let rank = i + 1;
                        let rc = match rank { 1 => GOLD, 2 => SILVER, 3 => BRONZE, _ => TEXT_MUTED };
                        let nc = match rank { 1 => GOLD, _ => TEXT_PRIMARY };
                        let ry = lb.cursor().top();
                        let rcy = ry + row_h * 0.5;
                        lb.painter().text(egui::pos2(col_rank_x, rcy), egui::Align2::LEFT_CENTER, format!("{rank}"), row_font.clone(), rc);
                        lb.painter().text(egui::pos2(col_name_x, rcy), egui::Align2::LEFT_CENTER, &entry.name, row_font.clone(), nc);
                        lb.painter().text(egui::pos2(col_total_x, rcy), egui::Align2::RIGHT_CENTER, format!("{}", entry.total_level), row_font.clone(), ACCENT_GREEN);
                        lb.painter().text(egui::pos2(col_cmbt_x, rcy), egui::Align2::RIGHT_CENTER, format!("{}", entry.combat_level), row_font.clone(), TEXT_DIM);
                        lb.painter().text(egui::pos2(col_kills_x, rcy), egui::Align2::RIGHT_CENTER, format!("{}", entry.monster_kills), row_font.clone(), TEXT_DIM);
                        lb.allocate_space(egui::vec2(content.width(), row_h));
                    }
                }

                // "Updated ..." at bottom of leaderboard panel
                if let Some(last) = stats.last_fetched {
                    lb.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                        let ago = last.elapsed().as_secs();
                        let t = if ago < 5 { "just now".into() } else { format!("{}s ago", ago) };
                        ui.colored_label(TEXT_MUTED, egui::RichText::new(format!("Updated {t}")).size(9.0));
                    });
                }
            });

        ctx.request_repaint_after(Duration::from_millis(100));
    }
}

// ── Stats polling ───────────────────────────────────────────────────────────

fn stats_poll_loop(server_url: String, stats: Arc<Mutex<LiveStats>>) {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap_or_default();

    loop {
        let overview_url = format!("{}/api/stats/overview", server_url);
        let lb_url = format!("{}/api/stats/leaderboard?sort=total_level&limit=7", server_url);

        let overview: Option<StatsOverview> = client.get(&overview_url).send().ok().and_then(|r| r.json().ok());
        let leaderboard: Option<Vec<LeaderboardEntry>> = client.get(&lb_url).send().ok().and_then(|r| r.json().ok());

        if let Ok(mut s) = stats.lock() {
            if let Some(ref o) = overview {
                s.overview = o.clone();
                s.error = None;
            }
            if let Some(lb) = leaderboard {
                s.leaderboard = lb;
                s.error = None;
            }
            if s.overview.online_players == 0 && s.leaderboard.is_empty() && overview.is_none() {
                s.error = Some("Server unreachable".to_string());
            }
            s.last_fetched = Some(Instant::now());
        }

        thread::sleep(Duration::from_secs(STATS_REFRESH_SECS));
    }
}

// ── Update logic ────────────────────────────────────────────────────────────

fn run_update(tx: mpsc::Sender<UpdateMsg>) -> Result<(), String> {
    let config = LauncherConfig::load();
    let install_dir = client_install_dir(&config)?;
    fs::create_dir_all(&install_dir).map_err(|e| format!("Failed to create install dir: {e}"))?;

    tx.send(UpdateMsg::Status("Fetching manifest...".to_string())).ok();

    let manifest_url = config.manifest_url();
    let client = reqwest::blocking::Client::new();
    let manifest: Manifest = client
        .get(&manifest_url)
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("Failed to fetch manifest: {e}"))
        .and_then(|r| r.json().map_err(|e| format!("Invalid manifest JSON: {e}")))?;

    tx.send(UpdateMsg::Version(manifest.version.clone())).ok();

    let platform_key = platform_key();
    let platform = manifest.platforms.get(&platform_key)
        .ok_or_else(|| format!("No build for platform {platform_key}"))?;

    let mut to_download: Vec<FileEntry> = Vec::new();
    let mut total_bytes: u64 = 0;
    for file in &platform.files {
        if needs_download(&install_dir, file)? {
            total_bytes += file.size;
            to_download.push(file.clone());
        }
    }

    if to_download.is_empty() {
        tx.send(UpdateMsg::Status("Client is up to date".to_string())).ok();
    } else {
        let n = to_download.len();
        tx.send(UpdateMsg::Status(format!("Downloading {} file{}...", n, if n == 1 { "" } else { "s" }))).ok();

        let mut downloaded: u64 = 0;
        for (i, file) in to_download.iter().enumerate() {
            let url = file_url(&config, &platform_key, file);
            tx.send(UpdateMsg::Status(format!("Downloading ({}/{}) {}", i + 1, n, file.path))).ok();

            let dest_path = install_dir.join(&file.path);
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent).map_err(|e| format!("Failed to create dirs: {e}"))?;
            }

            let mut response = client.get(url).send().and_then(|r| r.error_for_status())
                .map_err(|e| format!("Download failed: {e}"))?;

            let tmp_path = dest_path.with_extension("download");
            let mut out = File::create(&tmp_path).map_err(|e| format!("Failed to write file: {e}"))?;
            let mut hasher = sha2::Sha256::new();
            let mut buffer = [0u8; 1024 * 64];
            loop {
                let read = response.read(&mut buffer).map_err(|e| format!("Download error: {e}"))?;
                if read == 0 { break; }
                out.write_all(&buffer[..read]).map_err(|e| format!("Write failed: {e}"))?;
                hasher.update(&buffer[..read]);
                downloaded += read as u64;
                tx.send(UpdateMsg::Progress { downloaded, total: total_bytes }).ok();
            }

            let digest = hex::encode(hasher.finalize());
            if normalize_hash(&digest) != normalize_hash(&file.sha256) {
                let _ = fs::remove_file(&tmp_path);
                return Err(format!("Hash mismatch for {}", file.path));
            }

            if dest_path.exists() {
                fs::remove_file(&dest_path).map_err(|e| format!("Failed to replace file: {e}"))?;
            }
            fs::rename(&tmp_path, &dest_path).map_err(|e| format!("Failed to finalize file: {e}"))?;

            if file.executable.unwrap_or(false) {
                set_executable(&dest_path)?;
            }
        }
    }

    let entrypoint_path = install_dir.join(&platform.entrypoint);
    if !entrypoint_path.exists() {
        return Err(format!("Entry point missing after update: {}", entrypoint_path.display()));
    }

    tx.send(UpdateMsg::Done { entrypoint: entrypoint_path, work_dir: install_dir }).ok();
    Ok(())
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn needs_download(install_dir: &Path, file: &FileEntry) -> Result<bool, String> {
    let path = install_dir.join(&file.path);
    if !path.exists() { return Ok(true); }
    let digest = sha256_file(&path)?;
    Ok(normalize_hash(&digest) != normalize_hash(&file.sha256))
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| format!("Failed to open file: {e}"))?;
    let mut hasher = sha2::Sha256::new();
    let mut buffer = [0u8; 1024 * 64];
    loop {
        let read = file.read(&mut buffer).map_err(|e| format!("Read error: {e}"))?;
        if read == 0 { break; }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn normalize_hash(hash: &str) -> String { hash.trim().to_lowercase() }

fn file_url(config: &LauncherConfig, platform_key: &str, file: &FileEntry) -> String {
    if let Some(url) = &file.url { return url.clone(); }
    let mut base = config.base_url.trim_end_matches('/').to_string();
    base.push('/');
    base.push_str(platform_key);
    base.push('/');
    base.push_str(file.path.trim_start_matches('/'));
    base
}

fn platform_key() -> String {
    let os = env::consts::OS;
    let arch = env::consts::ARCH;
    match (os, arch) {
        ("macos", "aarch64") => "macos-arm64".to_string(),
        ("macos", "x86_64") => "macos-x86_64".to_string(),
        ("windows", "x86_64") => "windows-x86_64".to_string(),
        ("linux", "x86_64") => "linux-x86_64".to_string(),
        _ => format!("{os}-{arch}"),
    }
}

fn client_install_dir(config: &LauncherConfig) -> Result<PathBuf, String> {
    let app_name = config.app_name.as_deref().unwrap_or("New Aeven");
    let dirs = ProjectDirs::from("com", "New Aeven", app_name)
        .ok_or_else(|| "Failed to resolve data directory".to_string())?;
    Ok(dirs.data_dir().join("client"))
}

fn launch_client(entrypoint: &Path, work_dir: &Path) -> Result<(), String> {
    use std::process::{Command, Stdio};
    Command::new(entrypoint)
        .current_dir(work_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to launch: {e}"))
}

fn bytes_to_mb(bytes: u64) -> String { format!("{:.1}", bytes as f64 / 1_048_576.0) }

fn load_logo_bytes() -> Option<Vec<u8>> {
    for path in asset_candidates("logo.png") {
        if let Ok(bytes) = fs::read(&path) { return Some(bytes); }
    }
    None
}

fn load_icon_data() -> Option<egui::IconData> {
    for name in &["app-icon.png", "logo.png"] {
        for path in asset_candidates(name) {
            if let Ok(bytes) = fs::read(&path) {
                if let Some(icon) = decode_icon(&bytes) { return Some(icon); }
            }
        }
    }
    None
}

fn decode_icon(png_bytes: &[u8]) -> Option<egui::IconData> {
    let img = image::load_from_memory(png_bytes).ok()?;
    let rgba = img.to_rgba8();
    Some(egui::IconData { rgba: rgba.as_raw().to_vec(), width: rgba.width(), height: rgba.height() })
}

fn asset_candidates(file_name: &str) -> Vec<PathBuf> {
    let mut c: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = env::current_exe() {
        if let Some(dir) = exe.parent() { c.push(dir.join("assets").join(file_name)); }
    }
    c.push(PathBuf::from("assets").join(file_name));
    c
}

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path).map_err(|e| format!("Failed to read metadata: {e}"))?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).map_err(|e| format!("Failed to set permissions: {e}"))
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> Result<(), String> { Ok(()) }

// ── Entry point ─────────────────────────────────────────────────────────────

fn main() -> eframe::Result {
    let config = LauncherConfig::load();

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([480.0, 580.0])
        .with_resizable(false);
    if let Some(icon) = load_icon_data() {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native(
        "New Aeven",
        options,
        Box::new(move |_cc| Ok(Box::new(LauncherApp::new(&config)))),
    )
}
