use std::collections::HashMap;
use std::env;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use directories::ProjectDirs;
use eframe::egui;
use eframe::icon_data;
use egui_extras::RetainedImage;
use serde::Deserialize;
use sha2::Digest;

const DEFAULT_BASE_URL: &str = "https://example.invalid/launcher";
const DEFAULT_MANIFEST_PATH: &str = "manifest.json";

#[derive(Debug, Deserialize, Clone)]
struct LauncherConfig {
    base_url: String,
    manifest_path: Option<String>,
    app_name: Option<String>,
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
            base_url: env::var("LAUNCHER_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string()),
            manifest_path: Some(DEFAULT_MANIFEST_PATH.to_string()),
            app_name: Some("New Aeven".to_string()),
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
}

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

#[derive(Debug)]
enum UpdateMsg {
    Status(String),
    Progress { downloaded: u64, total: u64 },
    Done { entrypoint: PathBuf, work_dir: PathBuf },
    Error(String),
}

struct LauncherApp {
    status: String,
    progress: Option<(u64, u64)>,
    last_error: Option<String>,
    rx: Option<mpsc::Receiver<UpdateMsg>>,
    logo: Option<RetainedImage>,
    auto_launch: bool,
    launch_done: bool,
}

impl LauncherApp {
    fn new() -> Self {
        let logo = load_logo();
        let mut app = Self {
            status: "Starting up...".to_string(),
            progress: None,
            last_error: None,
            rx: None,
            logo,
            auto_launch: true,
            launch_done: false,
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
                UpdateMsg::Status(text) => {
                    self.status = text;
                }
                UpdateMsg::Progress { downloaded, total } => {
                    self.progress = Some((downloaded, total));
                }
                UpdateMsg::Done { entrypoint, work_dir } => {
                    self.status = "Launching client...".to_string();
                    self.progress = None;
                    if !self.launch_done && self.auto_launch {
                        match launch_client(&entrypoint, &work_dir) {
                            Ok(()) => {
                                self.status = "Client launched.".to_string();
                                self.launch_done = true;
                            }
                            Err(err) => {
                                self.status = "Launch failed.".to_string();
                                self.last_error = Some(err);
                            }
                        }
                    } else {
                        self.status = "Ready to launch.".to_string();
                    }
                }
                UpdateMsg::Error(err) => {
                    self.status = "Update failed.".to_string();
                    self.last_error = Some(err);
                }
            }
        }
    }
}

impl eframe::App for LauncherApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_messages();

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(10.0);
                if let Some(logo) = &self.logo {
                    logo.show(ui);
                    ui.add_space(8.0);
                } else {
                    ui.heading("New Aeven");
                    ui.add_space(8.0);
                }

                ui.label(&self.status);

                if let Some((downloaded, total)) = self.progress {
                    let fraction = if total > 0 {
                        downloaded as f32 / total as f32
                    } else {
                        0.0
                    };
                    let bar = egui::ProgressBar::new(fraction)
                        .show_percentage()
                        .desired_width(260.0);
                    ui.add(bar);
                    ui.label(format!("{} / {} MB", bytes_to_mb(downloaded), bytes_to_mb(total)));
                } else {
                    ui.add_space(12.0);
                }

                if let Some(err) = &self.last_error {
                    ui.add_space(8.0);
                    ui.colored_label(egui::Color32::RED, err);
                    ui.add_space(8.0);
                    if ui.button("Retry").clicked() {
                        self.start_update();
                    }
                }

                ui.add_space(16.0);
                if ui.button("Quit").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
        });

        ctx.request_repaint_after(Duration::from_millis(100));
    }
}

fn run_update(tx: mpsc::Sender<UpdateMsg>) -> Result<(), String> {
    let config = LauncherConfig::load();
    let install_dir = client_install_dir(&config)?;
    fs::create_dir_all(&install_dir).map_err(|e| format!("Failed to create install dir: {e}"))?;

    tx.send(UpdateMsg::Status("Fetching manifest...".to_string()))
        .ok();

    let manifest_url = config.manifest_url();
    let client = reqwest::blocking::Client::new();
    let manifest: Manifest = client
        .get(&manifest_url)
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("Failed to fetch manifest: {e}"))
        .and_then(|r| r.json().map_err(|e| format!("Invalid manifest JSON: {e}")))?;

    let platform_key = platform_key();
    let platform = manifest
        .platforms
        .get(&platform_key)
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
        tx.send(UpdateMsg::Status("Client is up to date.".to_string()))
            .ok();
    } else {
        tx.send(UpdateMsg::Status(format!(
            "Downloading update ({})...",
            manifest.version
        )))
        .ok();

        let mut downloaded: u64 = 0;
        for file in &to_download {
            let url = file_url(&config, &platform_key, file);
            tx.send(UpdateMsg::Status(format!("Downloading {}", file.path)))
                .ok();

            let dest_path = install_dir.join(&file.path);
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create dirs: {e}"))?;
            }

            let mut response = client
                .get(url)
                .send()
                .and_then(|r| r.error_for_status())
                .map_err(|e| format!("Download failed: {e}"))?;

            let tmp_path = dest_path.with_extension("download");
            let mut out = File::create(&tmp_path)
                .map_err(|e| format!("Failed to write file: {e}"))?;
            let mut hasher = sha2::Sha256::new();

            let mut buffer = [0u8; 1024 * 64];
            loop {
                let read = response
                    .read(&mut buffer)
                    .map_err(|e| format!("Download error: {e}"))?;
                if read == 0 {
                    break;
                }
                out.write_all(&buffer[..read])
                    .map_err(|e| format!("Write failed: {e}"))?;
                hasher.update(&buffer[..read]);
                downloaded += read as u64;
                tx.send(UpdateMsg::Progress {
                    downloaded,
                    total: total_bytes,
                })
                .ok();
            }

            let digest = hex::encode(hasher.finalize());
            if normalize_hash(&digest) != normalize_hash(&file.sha256) {
                let _ = fs::remove_file(&tmp_path);
                return Err(format!("Hash mismatch for {}", file.path));
            }

            if dest_path.exists() {
                fs::remove_file(&dest_path)
                    .map_err(|e| format!("Failed to replace file: {e}"))?;
            }
            fs::rename(&tmp_path, &dest_path)
                .map_err(|e| format!("Failed to finalize file: {e}"))?;

            if file.executable.unwrap_or(false) {
                set_executable(&dest_path)?;
            }
        }
    }

    let entrypoint_path = install_dir.join(&platform.entrypoint);
    if !entrypoint_path.exists() {
        return Err(format!(
            "Entry point missing after update: {}",
            entrypoint_path.display()
        ));
    }

    tx.send(UpdateMsg::Done {
        entrypoint: entrypoint_path,
        work_dir: install_dir,
    })
    .ok();

    Ok(())
}

fn needs_download(install_dir: &Path, file: &FileEntry) -> Result<bool, String> {
    let path = install_dir.join(&file.path);
    if !path.exists() {
        return Ok(true);
    }
    let digest = sha256_file(&path)?;
    Ok(normalize_hash(&digest) != normalize_hash(&file.sha256))
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| format!("Failed to open file: {e}"))?;
    let mut hasher = sha2::Sha256::new();
    let mut buffer = [0u8; 1024 * 64];
    loop {
        let read = file.read(&mut buffer).map_err(|e| format!("Read error: {e}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn normalize_hash(hash: &str) -> String {
    hash.trim().to_lowercase()
}

fn file_url(config: &LauncherConfig, platform_key: &str, file: &FileEntry) -> String {
    if let Some(url) = &file.url {
        return url.clone();
    }
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
    std::process::Command::new(entrypoint)
        .current_dir(work_dir)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to launch: {e}"))
}

fn bytes_to_mb(bytes: u64) -> String {
    format!("{:.1}", bytes as f64 / 1_048_576.0)
}

fn load_logo() -> Option<RetainedImage> {
    for path in asset_candidates("logo.png") {
        if let Ok(bytes) = fs::read(&path) {
            if let Ok(image) = RetainedImage::from_image_bytes("logo", &bytes) {
                return Some(image);
            }
        }
    }
    None
}

fn load_icon_data() -> Option<egui::IconData> {
    for path in asset_candidates("app-icon.png") {
        if let Ok(bytes) = fs::read(&path) {
            if let Ok(icon) = icon_data::from_png_bytes(&bytes) {
                return Some(icon);
            }
        }
    }
    for path in asset_candidates("logo.png") {
        if let Ok(bytes) = fs::read(&path) {
            if let Ok(icon) = icon_data::from_png_bytes(&bytes) {
                return Some(icon);
            }
        }
    }
    None
}

fn asset_candidates(file_name: &str) -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(exe_path) = env::current_exe() {
        if let Some(dir) = exe_path.parent() {
            candidates.push(dir.join("assets").join(file_name));
        }
    }
    candidates.push(PathBuf::from("assets").join(file_name));
    candidates
}

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path)
        .map_err(|e| format!("Failed to read metadata: {e}"))?
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).map_err(|e| format!("Failed to set permissions: {e}"))
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> Result<(), String> {
    Ok(())
}

fn main() -> eframe::Result<()> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([420.0, 380.0])
        .with_resizable(false);
    if let Some(icon) = load_icon_data() {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "New Aeven Launcher",
        options,
        Box::new(|_cc| Box::new(LauncherApp::new())),
    )
}
