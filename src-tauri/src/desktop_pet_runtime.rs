use crate::desktop_pet::{DesktopPetPackage, PetEvent, PetFrame, PetStateKind, PetStateMachine};
use chrono::TimeZone;
use eframe::egui::{self, Color32, Pos2, Rect, TextureHandle, Vec2};
use image::RgbaImage;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

const DETAIL_WIDTH: f32 = 236.0;
const DETAIL_HEIGHT: f32 = 88.0;
const DETAIL_GAP: f32 = 10.0;
const DISCO_HOP_SECONDS: f32 = 0.72;
const DISCO_HOPS_PER_LEG: i32 = 8;
const DISCO_CROUCH_OUT_END: f32 = 0.25;
const DISCO_LEAP_END: f32 = 0.78;
// Keep normal clicks responsive while allowing a short, deliberate movement to start dragging.
const PET_CLICK_DRAG_THRESHOLD_SQ: f32 = 25.0;
const PET_DOUBLE_CLICK_WINDOW: Duration = Duration::from_millis(420);
const PET_SINGLE_CLICK_DELAY: Duration = Duration::from_millis(450);
const PET_DOUBLE_CLICK_DISTANCE_SQ: f32 = 900.0;

#[cfg(target_os = "windows")]
fn system_ctrl_pressed() -> bool {
    const VK_CONTROL: i32 = 0x11;
    extern "system" {
        fn GetAsyncKeyState(v_key: i32) -> i16;
    }
    unsafe { GetAsyncKeyState(VK_CONTROL) & 0x8000u16 as i16 != 0 }
}

#[cfg(not(target_os = "windows"))]
fn system_ctrl_pressed() -> bool {
    false
}

fn native_pet_log(message: &str) {
    let path = std::env::temp_dir().join("lanchat-desktop-pet.log");
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        use std::io::Write;
        let _ = writeln!(file, "{} {message}", chrono::Utc::now().to_rfc3339());
    }
}

fn run_desktop_pet_window(
    mut options: eframe::NativeOptions,
    state: Arc<Mutex<DesktopPetRuntimeState>>,
    package: Arc<Mutex<Option<DesktopPetPackage>>>,
    repaint: Arc<Mutex<Option<egui::Context>>>,
    action_sink: DesktopPetActionSink,
) {
    #[cfg(target_os = "windows")]
    {
        use winit::platform::windows::EventLoopBuilderExtWindows;
        options.event_loop_builder = Some(Box::new(|builder| {
            builder.with_any_thread(true);
        }));
    }
    let result = eframe::run_native(
        "LanChat 桌宠",
        options,
        Box::new(move |cc| {
            Ok(Box::new(DesktopPetApp::new(
                cc,
                state,
                package,
                repaint,
                action_sink.clone(),
            )))
        }),
    );
    native_pet_log(&format!("eframe returned: {result:?}"));
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DesktopPetRuntimeState {
    pub enabled: bool,
    pub pending_count: u32,
    pub temperature: u8,
    pub latest_alert_id: Option<String>,
    pub latest_sender: Option<String>,
    pub latest_sender_address: Option<String>,
    pub latest_content: Option<String>,
    pub latest_created_at: Option<i64>,
    #[serde(default)]
    pub incoming_call_id: Option<String>,
    #[serde(default)]
    pub incoming_call_sender: Option<String>,
    #[serde(default)]
    pub incoming_call_media: Option<String>,
    pub feedbackable: bool,
    pub flashing: bool,
    pub disco: bool,
    pub theme_accent: Option<String>,
    #[serde(default = "default_true")]
    pub random_move_enabled: bool,
    #[serde(default = "default_true")]
    pub random_life_enabled: bool,
    #[serde(default = "default_disco_movement_mode")]
    pub disco_movement_mode: String,
}

fn default_true() -> bool {
    true
}

fn default_disco_movement_mode() -> String {
    "jump".to_string()
}

fn is_missing_alert_detail(value: Option<&String>) -> bool {
    value.is_none_or(|value| {
        let value = value.trim();
        value.is_empty() || matches!(value, "未知设备" | "未知 IP" | "未知IP")
    })
}

// Frontend state updates can arrive out of order. For the same alert, preserve
// known sender metadata instead of replacing it with an incomplete snapshot.
fn merge_runtime_state(
    previous: &DesktopPetRuntimeState,
    mut incoming: DesktopPetRuntimeState,
) -> DesktopPetRuntimeState {
    if previous.latest_alert_id != incoming.latest_alert_id || incoming.latest_alert_id.is_none() {
        return incoming;
    }

    if is_missing_alert_detail(incoming.latest_sender.as_ref()) {
        incoming.latest_sender = previous.latest_sender.clone();
    }
    if is_missing_alert_detail(incoming.latest_sender_address.as_ref()) {
        incoming.latest_sender_address = previous.latest_sender_address.clone();
    }
    if incoming
        .latest_content
        .as_deref()
        .is_none_or(|content| content.trim().is_empty())
    {
        incoming.latest_content = previous.latest_content.clone();
    }
    if incoming.latest_created_at.is_none() {
        incoming.latest_created_at = previous.latest_created_at;
    }
    incoming
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DesktopPetAction {
    action: String,
    alert_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
enum DesktopPetProcessCommand {
    State(DesktopPetRuntimeState),
    Package(Option<DesktopPetPackage>),
    Shutdown,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
enum DesktopPetProcessEvent {
    Action(DesktopPetAction),
    Log(String),
}

#[derive(Clone)]
enum DesktopPetActionSink {
    Stdout,
}

struct ActivePetClip {
    package_key: String,
    state: PetStateKind,
    direction: Option<String>,
    clip_id: String,
}

#[derive(Clone)]
pub struct DesktopPetController {
    state: Arc<Mutex<DesktopPetRuntimeState>>,
    package: Arc<Mutex<Option<DesktopPetPackage>>>,
    repaint: Arc<Mutex<Option<egui::Context>>>,
    process: Arc<Mutex<Option<DesktopPetProcessHandle>>>,
    app: AppHandle,
}

struct DesktopPetProcessHandle {
    child: Child,
    stdin: ChildStdin,
}

fn should_rehydrate_pet_process(enabled: bool) -> bool {
    enabled
}

#[cfg(test)]
mod desktop_pet_enable_tests {
    use super::{merge_runtime_state, should_rehydrate_pet_process, DesktopPetRuntimeState};

    #[test]
    fn enabling_pet_requires_state_and_package_rehydration() {
        assert!(should_rehydrate_pet_process(true));
        assert!(!should_rehydrate_pet_process(false));
    }

    #[test]
    fn same_alert_keeps_known_sender_details_when_a_later_snapshot_is_incomplete() {
        let previous = DesktopPetRuntimeState {
            latest_alert_id: Some("alert-1".to_string()),
            latest_sender: Some("王二".to_string()),
            latest_sender_address: Some("192.168.1.23".to_string()),
            latest_content: Some("呱呱~呱~~".to_string()),
            latest_created_at: Some(1_700_000_000_000),
            flashing: true,
            ..Default::default()
        };
        let incoming = DesktopPetRuntimeState {
            latest_alert_id: Some("alert-1".to_string()),
            latest_sender: Some("未知设备".to_string()),
            latest_sender_address: None,
            latest_content: None,
            latest_created_at: None,
            flashing: true,
            ..Default::default()
        };

        let merged = merge_runtime_state(&previous, incoming);

        assert_eq!(merged.latest_sender.as_deref(), Some("王二"));
        assert_eq!(
            merged.latest_sender_address.as_deref(),
            Some("192.168.1.23")
        );
        assert_eq!(merged.latest_content.as_deref(), Some("呱呱~呱~~"));
        assert_eq!(merged.latest_created_at, Some(1_700_000_000_000));
    }
}

impl DesktopPetController {
    pub fn start(app: AppHandle) -> Self {
        let state = Arc::new(Mutex::new(DesktopPetRuntimeState {
            enabled: true,
            ..Default::default()
        }));
        let package = Arc::new(Mutex::new(None));
        let repaint = Arc::new(Mutex::new(None));
        let controller = Self {
            state: state.clone(),
            package: package.clone(),
            repaint: repaint.clone(),
            process: Arc::new(Mutex::new(None)),
            app: app.clone(),
        };
        controller.ensure_process();
        controller
    }

    pub fn update(&self, next: DesktopPetRuntimeState) {
        let enabled = next.enabled;
        if let Ok(mut state) = self.state.lock() {
            *state = merge_runtime_state(&state, next);
        }
        self.send_command(DesktopPetProcessCommand::State(self.state()));
        if let Ok(context) = self.repaint.lock() {
            if let Some(context) = context.as_ref() {
                context.send_viewport_cmd(egui::ViewportCommand::Visible(enabled));
                context.request_repaint();
            }
        }
    }

    pub fn set_enabled(&self, enabled: bool) {
        if let Ok(mut state) = self.state.lock() {
            state.enabled = enabled;
        }
        self.send_command(DesktopPetProcessCommand::State(self.state()));
        if should_rehydrate_pet_process(enabled) {
            let package = self.package.lock().ok().and_then(|current| current.clone());
            self.send_command(DesktopPetProcessCommand::Package(package));
        }
        if let Ok(context) = self.repaint.lock() {
            if let Some(context) = context.as_ref() {
                context.send_viewport_cmd(egui::ViewportCommand::Visible(enabled));
                context.request_repaint();
            }
        }
    }

    pub fn set_package(&self, package: Option<DesktopPetPackage>) {
        if let Ok(mut current) = self.package.lock() {
            *current = package.clone();
        }
        self.send_command(DesktopPetProcessCommand::Package(package));
        if let Ok(context) = self.repaint.lock() {
            if let Some(context) = context.as_ref() {
                context.request_repaint();
            }
        }
    }

    pub fn shutdown(&self) {
        self.send_command(DesktopPetProcessCommand::Shutdown);
        if let Ok(mut process) = self.process.lock() {
            if let Some(mut handle) = process.take() {
                let _ = handle.child.kill();
                let _ = handle.child.wait();
            }
        }
    }

    fn state(&self) -> DesktopPetRuntimeState {
        self.state
            .lock()
            .map(|value| value.clone())
            .unwrap_or_default()
    }

    fn ensure_process(&self) {
        let mut current = match self.process.lock() {
            Ok(value) => value,
            Err(_) => return,
        };
        if current
            .as_mut()
            .is_some_and(|handle| handle.child.try_wait().ok().flatten().is_none())
        {
            return;
        }
        native_pet_log("starting desktop pet child process");
        let Ok(exe) = std::env::current_exe() else {
            native_pet_log("failed to resolve current executable for desktop pet child process");
            return;
        };
        let mut command = Command::new(exe);
        command
            .arg("--desktop-pet")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let Ok(mut child) = command.spawn() else {
            native_pet_log("failed to spawn desktop pet child process");
            return;
        };
        if let Some(stdout) = child.stdout.take() {
            start_desktop_pet_stdout_reader(self.app.clone(), stdout);
        }
        if let Some(stderr) = child.stderr.take() {
            start_desktop_pet_stderr_reader(stderr);
        }
        let Some(stdin) = child.stdin.take() else {
            native_pet_log("desktop pet child process stdin is unavailable");
            let _ = child.kill();
            return;
        };
        *current = Some(DesktopPetProcessHandle { child, stdin });
    }

    fn send_command(&self, command: DesktopPetProcessCommand) {
        self.ensure_process();
        let Ok(mut current) = self.process.lock() else {
            return;
        };
        let Some(handle) = current.as_mut() else {
            return;
        };
        let Ok(line) = serde_json::to_string(&command) else {
            return;
        };
        if writeln!(handle.stdin, "{line}").is_err() || handle.stdin.flush().is_err() {
            native_pet_log("desktop pet child process stdin write failed");
            if let Some(mut failed) = current.take() {
                let _ = failed.child.kill();
                let _ = failed.child.wait();
            }
        }
    }
}

fn start_desktop_pet_stdout_reader(app: AppHandle, stdout: impl std::io::Read + Send + 'static) {
    let _ = std::thread::Builder::new()
        .name("lanchat-desktop-pet-stdout".to_string())
        .spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                if line.trim().is_empty() {
                    continue;
                }
                match serde_json::from_str::<DesktopPetProcessEvent>(&line) {
                    Ok(DesktopPetProcessEvent::Action(action)) => {
                        let _ = app.emit("desktop_pet_action", &action);
                    }
                    Ok(DesktopPetProcessEvent::Log(message)) => {
                        native_pet_log(&format!("child: {message}"));
                    }
                    Err(error) => {
                        native_pet_log(&format!(
                            "invalid desktop pet child event: {error}; {line}"
                        ));
                    }
                }
            }
            native_pet_log("desktop pet stdout reader exited");
        });
}

fn start_desktop_pet_stderr_reader(stderr: impl std::io::Read + Send + 'static) {
    let _ = std::thread::Builder::new()
        .name("lanchat-desktop-pet-stderr".to_string())
        .spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                if !line.trim().is_empty() {
                    native_pet_log(&format!("child stderr: {line}"));
                }
            }
        });
}

pub fn run_desktop_pet_process() {
    native_pet_log("desktop pet child process entered");
    let state = Arc::new(Mutex::new(DesktopPetRuntimeState {
        enabled: true,
        ..Default::default()
    }));
    let package = Arc::new(Mutex::new(None));
    let repaint = Arc::new(Mutex::new(None));
    start_desktop_pet_stdin_reader(state.clone(), package.clone(), repaint.clone());
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([180.0, 160.0])
            .with_min_inner_size([96.0, 92.0])
            .with_max_inner_size([520.0, 380.0])
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top()
            .with_taskbar(false)
            .with_resizable(true),
        ..Default::default()
    };
    run_desktop_pet_window(
        options,
        state,
        package,
        repaint,
        DesktopPetActionSink::Stdout,
    );
}

fn start_desktop_pet_stdin_reader(
    state: Arc<Mutex<DesktopPetRuntimeState>>,
    package: Arc<Mutex<Option<DesktopPetPackage>>>,
    repaint: Arc<Mutex<Option<egui::Context>>>,
) {
    let _ = std::thread::Builder::new()
        .name("lanchat-desktop-pet-stdin".to_string())
        .spawn(move || {
            let reader = BufReader::new(std::io::stdin());
            for line in reader.lines().map_while(Result::ok) {
                if line.trim().is_empty() {
                    continue;
                }
                match serde_json::from_str::<DesktopPetProcessCommand>(&line) {
                    Ok(DesktopPetProcessCommand::State(next)) => {
                        let enabled = next.enabled;
                        if let Ok(mut current) = state.lock() {
                            *current = next;
                        }
                        if let Ok(context) = repaint.lock() {
                            if let Some(context) = context.as_ref() {
                                context.send_viewport_cmd(egui::ViewportCommand::Visible(enabled));
                                context.request_repaint();
                            }
                        }
                    }
                    Ok(DesktopPetProcessCommand::Package(next)) => {
                        if let Ok(mut current) = package.lock() {
                            *current = next;
                        }
                        if let Ok(context) = repaint.lock() {
                            if let Some(context) = context.as_ref() {
                                context.request_repaint();
                            }
                        }
                    }
                    Ok(DesktopPetProcessCommand::Shutdown) => {
                        std::process::exit(0);
                    }
                    Err(error) => {
                        native_pet_log(&format!("invalid desktop pet command: {error}; {line}"));
                    }
                }
            }
            native_pet_log("desktop pet stdin reader exited");
            std::process::exit(0);
        });
}

struct DesktopPetApp {
    state: Arc<Mutex<DesktopPetRuntimeState>>,
    package: Arc<Mutex<Option<DesktopPetPackage>>>,
    action_sink: DesktopPetActionSink,
    details_open: bool,
    detail_last_open: bool,
    detail_side: i8,
    last_size: Vec2,
    started_at: Instant,
    initial_positioned: bool,
    move_direction: i8,
    dynamic_package_key: Option<String>,
    dynamic_textures: HashMap<PathBuf, TextureHandle>,
    dynamic_texture_order: VecDeque<PathBuf>,
    runtime_machine: PetStateMachine,
    runtime_state_started: Instant,
    next_idle_action_move: bool,
    active_clip: Option<ActivePetClip>,
    active_clip_started: Instant,
    active_clip_duration: Duration,
    sequence_target_count: u32,
    sequence_completed_count: u32,
    sequence_interval: Duration,
    sequence_interval_started: Option<Instant>,
    sequence_finished: bool,
    sequence_key: Option<(String, PetStateKind, Option<String>)>,
    last_package_frame: Option<PetFrame>,
    pet_press_pos: Option<Pos2>,
    pet_press_dragged: bool,
    last_pet_click_at: Option<Instant>,
    last_pet_click_pos: Option<Pos2>,
    pending_single_click_at: Option<Instant>,
    pending_single_click_alert_active: bool,
    disco_origin: Option<Pos2>,
    was_disco: bool,
}

fn should_restore_disco_origin(was_disco: bool, disco_active: bool) -> bool {
    was_disco && !disco_active
}

#[cfg(test)]
mod disco_position_tests {
    use super::should_restore_disco_origin;

    #[test]
    fn restores_origin_only_when_disco_stops() {
        assert!(!should_restore_disco_origin(false, false));
        assert!(!should_restore_disco_origin(false, true));
        assert!(!should_restore_disco_origin(true, true));
        assert!(should_restore_disco_origin(true, false));
    }
}

impl DesktopPetApp {
    fn new(
        _cc: &eframe::CreationContext<'_>,
        state: Arc<Mutex<DesktopPetRuntimeState>>,
        package: Arc<Mutex<Option<DesktopPetPackage>>>,
        repaint: Arc<Mutex<Option<egui::Context>>>,
        action_sink: DesktopPetActionSink,
    ) -> Self {
        install_cjk_fonts(&_cc.egui_ctx);
        if let Ok(mut context) = repaint.lock() {
            *context = Some(_cc.egui_ctx.clone());
        }
        Self {
            state,
            package,
            action_sink,
            details_open: false,
            detail_last_open: false,
            detail_side: -1,
            last_size: Vec2::new(180.0, 160.0),
            started_at: Instant::now(),
            initial_positioned: false,
            move_direction: 0,
            dynamic_package_key: None,
            dynamic_textures: HashMap::new(),
            dynamic_texture_order: VecDeque::new(),
            runtime_machine: PetStateMachine::new(),
            runtime_state_started: Instant::now(),
            next_idle_action_move: false,
            active_clip: None,
            active_clip_started: Instant::now(),
            active_clip_duration: Duration::ZERO,
            sequence_target_count: 0,
            sequence_completed_count: 0,
            sequence_interval: Duration::ZERO,
            sequence_interval_started: None,
            sequence_finished: false,
            sequence_key: None,
            last_package_frame: None,
            pet_press_pos: None,
            pet_press_dragged: false,
            last_pet_click_at: None,
            last_pet_click_pos: None,
            pending_single_click_at: None,
            pending_single_click_alert_active: false,
            disco_origin: None,
            was_disco: false,
        }
    }

    fn emit_action(&self, action: &str, alert_id: Option<String>) {
        native_pet_log(&format!("emit action: {action}"));
        let payload = DesktopPetAction {
            action: action.to_string(),
            alert_id,
        };
        match &self.action_sink {
            DesktopPetActionSink::Stdout => {
                let event = DesktopPetProcessEvent::Action(payload);
                if let Ok(line) = serde_json::to_string(&event) {
                    println!("{line}");
                    let _ = std::io::stdout().flush();
                }
            }
        }
    }

    fn package_key(package: &DesktopPetPackage) -> String {
        format!(
            "{}:{}:{}",
            package.manifest.id,
            package.manifest.version,
            package.root.display()
        )
    }

    fn sync_dynamic_package(&mut self, package: &DesktopPetPackage) {
        let key = Self::package_key(package);
        if self.dynamic_package_key.as_deref() != Some(key.as_str()) {
            self.dynamic_package_key = Some(key);
            self.dynamic_textures.clear();
            self.dynamic_texture_order.clear();
            self.runtime_machine.handle(PetEvent::PackageChanged);
            self.runtime_state_started = Instant::now();
            self.reset_active_sequence();
        }
    }

    fn random_inclusive(min: u32, max: u32) -> u32 {
        if min >= max {
            min
        } else {
            fastrand::u32(min..=max)
        }
    }

    fn reset_active_sequence(&mut self) {
        self.active_clip = None;
        self.active_clip_started = Instant::now();
        self.active_clip_duration = Duration::ZERO;
        self.sequence_target_count = 0;
        self.sequence_completed_count = 0;
        self.sequence_interval = Duration::ZERO;
        self.sequence_interval_started = None;
        self.sequence_finished = false;
        self.sequence_key = None;
        self.last_package_frame = None;
    }

    fn begin_sequence(
        &mut self,
        package: &DesktopPetPackage,
        state: PetStateKind,
        direction: Option<String>,
    ) {
        let config = package.playback_config(state);
        self.sequence_key = Some((Self::package_key(package), state, direction));
        self.sequence_target_count =
            Self::random_inclusive(config.min_action_count, config.max_action_count);
        self.sequence_completed_count = 0;
        self.sequence_interval = Duration::ZERO;
        self.sequence_interval_started = None;
        self.sequence_finished = false;
        self.active_clip = None;
        self.last_package_frame = None;
    }

    fn dynamic_texture(&mut self, ctx: &egui::Context, frame: &PetFrame) -> Option<TextureHandle> {
        if let Some(texture) = self.dynamic_textures.get(&frame.path) {
            return Some(texture.clone());
        }
        let image = image::open(&frame.path).ok()?.to_rgba8();
        let cropped = Self::crop_transparent_padding(&image, 18);
        let size = [cropped.width() as usize, cropped.height() as usize];
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, cropped.as_raw());
        let texture = ctx.load_texture(
            format!("desktop-pet:{}", frame.path.display()),
            color_image,
            egui::TextureOptions::LINEAR,
        );
        self.dynamic_textures
            .insert(frame.path.clone(), texture.clone());
        self.dynamic_texture_order.push_back(frame.path.clone());
        while self.dynamic_texture_order.len() > 24 {
            if let Some(path) = self.dynamic_texture_order.pop_front() {
                self.dynamic_textures.remove(&path);
            }
        }
        Some(texture)
    }

    fn draw_package_frame(
        &mut self,
        ctx: &egui::Context,
        painter: &egui::Painter,
        rect: Rect,
        package: &DesktopPetPackage,
        state: PetStateKind,
        direction: Option<&str>,
    ) -> bool {
        self.sync_dynamic_package(package);
        let Some(frame) = self.active_package_frame(package, state, direction) else {
            return false;
        };
        let Some(texture) = self.dynamic_texture(ctx, &frame) else {
            return false;
        };
        painter.image(
            texture.id(),
            Self::fitted_texture_rect(&texture, rect),
            Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
        true
    }

    fn active_package_frame(
        &mut self,
        package: &DesktopPetPackage,
        state: PetStateKind,
        direction: Option<&str>,
    ) -> Option<PetFrame> {
        let package_key = Self::package_key(package);
        let direction_key = direction.map(str::to_string);
        let next_sequence_key = (package_key.clone(), state, direction_key.clone());
        if self.sequence_key.as_ref() != Some(&next_sequence_key) {
            self.reset_active_sequence();
            self.begin_sequence(package, state, direction_key.clone());
        }
        if self.sequence_finished {
            return self.last_package_frame.clone();
        }
        if let Some(started) = self.sequence_interval_started {
            if started.elapsed() < self.sequence_interval {
                return self.last_package_frame.clone();
            }
            self.sequence_interval_started = None;
            self.sequence_interval = Duration::ZERO;
        }

        let elapsed = self.active_clip_started.elapsed().as_secs_f32();
        let current_clip = self.active_clip.as_ref().and_then(|active| {
            (active.package_key == package_key
                && active.state == state
                && active.direction == direction_key)
                .then_some(active.clip_id.as_str())
        });
        if let Some(clip_id) = current_clip {
            if let Some(clip) = package
                .clip_candidates(state, direction)
                .into_iter()
                .find(|clip| clip.id == clip_id)
            {
                if self.active_clip_started.elapsed() < self.active_clip_duration {
                    let frame = DesktopPetPackage::frame_in_clip(clip, elapsed).cloned();
                    if frame.is_some() {
                        self.last_package_frame = frame.clone();
                    }
                    return frame;
                }
                self.last_package_frame = clip.frames.last().cloned();
            }
            self.active_clip = None;
            self.sequence_completed_count = self.sequence_completed_count.saturating_add(1);
            if self.sequence_completed_count >= self.sequence_target_count {
                if matches!(state, PetStateKind::Interact | PetStateKind::Life) {
                    self.sequence_finished = true;
                    return self.last_package_frame.clone();
                }
                let config = package.playback_config(state);
                self.sequence_target_count =
                    Self::random_inclusive(config.min_action_count, config.max_action_count);
                self.sequence_completed_count = 0;
            }
            let config = package.playback_config(state);
            self.sequence_interval = Duration::from_millis(Self::random_inclusive(
                config.min_interval_ms,
                config.max_interval_ms,
            ) as u64);
            self.sequence_interval_started = Some(Instant::now());
            return self.last_package_frame.clone();
        }

        let candidate_count = package.clip_candidates(state, direction).len();
        if candidate_count == 0 {
            self.active_clip = None;
            return None;
        }
        let index = fastrand::usize(..candidate_count);
        let clip = package.clip_by_uniform_index(state, direction, index)?;
        self.active_clip = Some(ActivePetClip {
            package_key,
            state,
            direction: direction_key,
            clip_id: clip.id.clone(),
        });
        self.active_clip_started = Instant::now();
        let cycle = DesktopPetPackage::clip_cycle_seconds(clip);
        let config = package.playback_config(state);
        let configured_millis =
            Self::random_inclusive(config.min_duration_ms, config.max_duration_ms);
        let configured_seconds = if configured_millis == 0 {
            cycle
        } else {
            configured_millis as f32 / 1000.0
        };
        self.active_clip_duration = Duration::from_secs_f32(cycle.max(configured_seconds));
        let frame = DesktopPetPackage::frame_in_clip(clip, 0.0).cloned();
        self.last_package_frame = frame.clone();
        frame
    }

    fn transition_runtime(&mut self, event: PetEvent) {
        let previous = self.runtime_machine.current();
        self.runtime_machine.handle(event);
        if previous != self.runtime_machine.current() {
            self.runtime_state_started = Instant::now();
            self.reset_active_sequence();
        }
    }

    fn update_runtime_state(&mut self, state: &DesktopPetRuntimeState) -> PetStateKind {
        let alert_active = state.flashing || state.pending_count > 0 || state.disco;
        if alert_active {
            self.transition_runtime(PetEvent::AlertRaised);
            return self.runtime_machine.current();
        }
        if self.runtime_machine.current() == PetStateKind::Alert {
            self.transition_runtime(PetEvent::AlertCleared);
        }
        let elapsed = self.runtime_state_started.elapsed();
        match self.runtime_machine.current() {
            PetStateKind::Interact if self.sequence_finished => {
                self.transition_runtime(PetEvent::InteractionFinished);
            }
            PetStateKind::Move if elapsed >= Duration::from_secs(5) => {
                self.transition_runtime(PetEvent::MovementFinished);
            }
            PetStateKind::Life if self.sequence_finished => {
                self.transition_runtime(PetEvent::LifeFinished);
            }
            PetStateKind::Idle if elapsed >= Duration::from_secs(20) => {
                if self.next_idle_action_move && state.random_move_enabled {
                    self.transition_runtime(PetEvent::MoveTimer);
                } else if !self.next_idle_action_move && state.random_life_enabled {
                    self.transition_runtime(PetEvent::LifeTimer);
                } else if state.random_move_enabled {
                    self.transition_runtime(PetEvent::MoveTimer);
                } else if state.random_life_enabled {
                    self.transition_runtime(PetEvent::LifeTimer);
                } else {
                    self.runtime_state_started = Instant::now();
                }
                self.next_idle_action_move = !self.next_idle_action_move;
            }
            _ => {}
        }
        self.runtime_machine.current()
    }

    fn crop_transparent_padding(image: &RgbaImage, padding: u32) -> RgbaImage {
        let mut left = image.width();
        let mut top = image.height();
        let mut right = 0;
        let mut bottom = 0;
        for (x, y, pixel) in image.enumerate_pixels() {
            if pixel[3] > 8 {
                left = left.min(x);
                top = top.min(y);
                right = right.max(x);
                bottom = bottom.max(y);
            }
        }
        if right <= left || bottom <= top {
            return image.clone();
        }
        let left = left.saturating_sub(padding);
        let top = top.saturating_sub(padding);
        let right = (right + padding).min(image.width() - 1);
        let bottom = (bottom + padding).min(image.height() - 1);
        image::imageops::crop_imm(image, left, top, right - left + 1, bottom - top + 1).to_image()
    }

    fn fitted_texture_rect(texture: &TextureHandle, rect: Rect) -> Rect {
        let size = texture.size_vec2();
        if size.x <= 0.0 || size.y <= 0.0 {
            return rect;
        }
        let scale = (rect.width() / size.x).min(rect.height() / size.y);
        Rect::from_center_size(rect.center(), size * scale)
    }

    fn format_alert_time(value: Option<i64>) -> String {
        let Some(value) = value else {
            return String::new();
        };
        let millis = if value.abs() > 10_000_000_000 {
            value
        } else {
            value * 1000
        };
        chrono::Local
            .timestamp_millis_opt(millis)
            .single()
            .map(|time| time.format("%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| value.to_string())
    }

    fn parse_hex_color(value: Option<&str>, fallback: Color32) -> Color32 {
        let Some(value) = value
            .map(str::trim)
            .and_then(|value| value.strip_prefix('#'))
        else {
            return fallback;
        };
        if value.len() != 6 {
            return fallback;
        }
        let Ok(red) = u8::from_str_radix(&value[0..2], 16) else {
            return fallback;
        };
        let Ok(green) = u8::from_str_radix(&value[2..4], 16) else {
            return fallback;
        };
        let Ok(blue) = u8::from_str_radix(&value[4..6], 16) else {
            return fallback;
        };
        Color32::from_rgb(red, green, blue)
    }

    fn alert_detail_background(state: &DesktopPetRuntimeState) -> Color32 {
        let accent = Self::parse_hex_color(
            state.theme_accent.as_deref(),
            Color32::from_rgb(22, 119, 255),
        );
        Color32::from_rgba_unmultiplied(
            ((accent.r() as u16 + 255 * 5) / 6) as u8,
            ((accent.g() as u16 + 255 * 5) / 6) as u8,
            ((accent.b() as u16 + 255 * 5) / 6) as u8,
            232,
        )
    }

    fn detail_panel_rect(available: Vec2, pet_rect: Rect, side: i8) -> Rect {
        let y = (pet_rect.center().y - DETAIL_HEIGHT * 0.5)
            .clamp(6.0, (available.y - DETAIL_HEIGHT - 6.0).max(6.0));
        let x = if side < 0 {
            (pet_rect.left() - DETAIL_GAP - DETAIL_WIDTH).max(6.0)
        } else {
            (pet_rect.right() + DETAIL_GAP).min((available.x - DETAIL_WIDTH - 6.0).max(6.0))
        };
        Rect::from_min_size(Pos2::new(x, y), Vec2::new(DETAIL_WIDTH, DETAIL_HEIGHT))
    }

    fn draw_alert_details(
        &mut self,
        ui: &mut egui::Ui,
        panel: Rect,
        state: &DesktopPetRuntimeState,
    ) {
        let painter = ui.painter();
        painter.rect_filled(panel, 12.0, Self::alert_detail_background(state));
        let text_color = Color32::from_rgb(32, 38, 45);
        let muted_color = Color32::from_rgb(116, 126, 138);
        let sender = state.latest_sender.as_deref().unwrap_or("告警");
        let sender_address = state.latest_sender_address.as_deref().unwrap_or("未知 IP");
        let sender_line = format!("{}：{}", sender, sender_address);
        let created_at = Self::format_alert_time(state.latest_created_at);
        let title = state.latest_content.as_deref().unwrap_or("");

        painter.text(
            panel.left_top() + Vec2::new(12.0, 10.0),
            egui::Align2::LEFT_TOP,
            sender_line,
            egui::FontId::proportional(11.0),
            muted_color,
        );
        painter.text(
            panel.left_top() + Vec2::new(12.0, 34.0),
            egui::Align2::LEFT_TOP,
            title,
            egui::FontId::proportional(13.0),
            text_color,
        );
        painter.text(
            panel.right_bottom() + Vec2::new(-12.0, -18.0),
            egui::Align2::RIGHT_CENTER,
            created_at,
            egui::FontId::proportional(10.5),
            muted_color,
        );

        if state.feedbackable {
            let real = Rect::from_min_size(
                panel.left_bottom() + Vec2::new(12.0, -30.0),
                Vec2::new(46.0, 22.0),
            );
            let false_alarm = Rect::from_min_size(
                panel.left_bottom() + Vec2::new(66.0, -30.0),
                Vec2::new(46.0, 22.0),
            );
            painter.rect_filled(real, 7.0, Color32::from_rgb(28, 170, 106));
            painter.rect_filled(false_alarm, 7.0, Color32::from_rgb(210, 68, 75));
            painter.text(
                real.center(),
                egui::Align2::CENTER_CENTER,
                "真实",
                egui::FontId::proportional(11.5),
                Color32::WHITE,
            );
            painter.text(
                false_alarm.center(),
                egui::Align2::CENTER_CENTER,
                "虚假",
                egui::FontId::proportional(11.5),
                Color32::WHITE,
            );
            if ui
                .interact(real, ui.id().with("feedback-real"), egui::Sense::click())
                .clicked()
            {
                self.emit_action("feedback_real", state.latest_alert_id.clone());
            }
            if ui
                .interact(
                    false_alarm,
                    ui.id().with("feedback-false"),
                    egui::Sense::click(),
                )
                .clicked()
            {
                self.emit_action("feedback_false", state.latest_alert_id.clone());
            }
        }
    }

    fn draw_call_details(
        &mut self,
        ui: &mut egui::Ui,
        panel: Rect,
        state: &DesktopPetRuntimeState,
    ) {
        let painter = ui.painter();
        painter.rect_filled(panel, 12.0, Self::alert_detail_background(state));
        let sender = state.incoming_call_sender.as_deref().unwrap_or("好友");
        let media = if state.incoming_call_media.as_deref() == Some("video") {
            "视频通话邀请"
        } else {
            "语音通话邀请"
        };
        painter.text(
            panel.left_top() + Vec2::new(12.0, 11.0),
            egui::Align2::LEFT_TOP,
            sender,
            egui::FontId::proportional(13.0),
            Color32::from_rgb(32, 38, 45),
        );
        painter.text(
            panel.left_top() + Vec2::new(12.0, 34.0),
            egui::Align2::LEFT_TOP,
            media,
            egui::FontId::proportional(11.0),
            Color32::from_rgb(116, 126, 138),
        );
        let accept = Rect::from_min_size(
            panel.left_bottom() + Vec2::new(12.0, -29.0),
            Vec2::new(74.0, 23.0),
        );
        let reject = Rect::from_min_size(
            panel.left_bottom() + Vec2::new(94.0, -29.0),
            Vec2::new(74.0, 23.0),
        );
        painter.rect_filled(accept, 7.0, Color32::from_rgb(24, 167, 105));
        painter.rect_filled(reject, 7.0, Color32::from_rgb(218, 65, 79));
        painter.text(
            accept.center(),
            egui::Align2::CENTER_CENTER,
            "接听",
            egui::FontId::proportional(11.0),
            Color32::WHITE,
        );
        painter.text(
            reject.center(),
            egui::Align2::CENTER_CENTER,
            "拒绝",
            egui::FontId::proportional(11.0),
            Color32::WHITE,
        );
        if ui
            .interact(accept, ui.id().with("call-accept"), egui::Sense::click())
            .clicked()
        {
            self.emit_action("accept_call", state.incoming_call_id.clone());
        }
        if ui
            .interact(reject, ui.id().with("call-reject"), egui::Sense::click())
            .clicked()
        {
            self.emit_action("reject_call", state.incoming_call_id.clone());
        }
    }

    fn draw_bold_temperature(painter: &egui::Painter, pos: Pos2, text: String, font: egui::FontId) {
        let color = Color32::from_rgb(228, 58, 68);
        painter.text(
            pos,
            egui::Align2::CENTER_CENTER,
            text.clone(),
            font.clone(),
            color,
        );
        painter.text(
            pos + Vec2::new(0.55, 0.0),
            egui::Align2::CENTER_CENTER,
            text,
            font,
            color,
        );
    }

    fn resize_from_scroll(&mut self, ctx: &egui::Context, delta: f32) {
        if delta.abs() < 0.01 || self.details_open {
            return;
        }
        let factor = if delta > 0.0 { 1.08 } else { 0.92 };
        let next = Vec2::new(
            (self.last_size.x * factor).clamp(96.0, 520.0),
            (self.last_size.y * factor).clamp(92.0, 380.0),
        );
        self.last_size = next;
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(next));
    }

    fn alert_visual_active(runtime_state: PetStateKind, state: &DesktopPetRuntimeState) -> bool {
        runtime_state == PetStateKind::Alert
            || state.flashing
            || state.pending_count > 0
            || state.disco
    }

    fn start_pointer_interaction(&mut self, ctx: &egui::Context) {
        self.runtime_machine.handle(PetEvent::PointerInteract);
        if self.runtime_machine.current() == PetStateKind::Interact {
            self.runtime_state_started = Instant::now();
            self.reset_active_sequence();
            ctx.request_repaint();
        }
    }

    fn run_single_pet_click(&mut self, ctx: &egui::Context, alert_active: bool) {
        if alert_active {
            native_pet_log("single pet click resolved as stop_visuals");
            self.emit_action("stop_visuals", None);
        } else {
            native_pet_log("single pet click resolved as interact_and_open_main_window");
            self.start_pointer_interaction(ctx);
            self.emit_action("open_main_window", None);
        }
    }

    fn finish_pending_single_click(&mut self, ctx: &egui::Context) {
        let Some(started_at) = self.pending_single_click_at else {
            return;
        };
        if started_at.elapsed() < PET_SINGLE_CLICK_DELAY {
            return;
        }
        let alert_active = self.pending_single_click_alert_active;
        self.pending_single_click_at = None;
        self.run_single_pet_click(ctx, alert_active);
    }

    fn ctrl_pressed(ctx: &egui::Context) -> bool {
        ctx.input(|input| input.modifiers.ctrl) || system_ctrl_pressed()
    }

    fn emit_pet_double_click_action(&mut self, ctx: &egui::Context) {
        if Self::ctrl_pressed(ctx) {
            native_pet_log("double pet click resolved as broadcast_disco_alert");
            self.emit_action("broadcast_disco_alert", None);
        } else {
            native_pet_log("double pet click resolved as quick_alert");
            self.emit_action("quick_alert", None);
        }
    }

    fn handle_primary_pet_click(&mut self, ctx: &egui::Context, pos: Pos2, alert_active: bool) {
        let now = Instant::now();
        let is_double_click = self
            .last_pet_click_at
            .zip(self.last_pet_click_pos)
            .map(|(last_at, last_pos)| {
                now.duration_since(last_at) <= PET_DOUBLE_CLICK_WINDOW
                    && (pos - last_pos).length_sq() <= PET_DOUBLE_CLICK_DISTANCE_SQ
            })
            .unwrap_or(false);

        self.last_pet_click_at = Some(now);
        self.last_pet_click_pos = Some(pos);

        if is_double_click {
            self.pending_single_click_at = None;
            self.emit_pet_double_click_action(ctx);
        } else {
            native_pet_log("primary pet click scheduled");
            self.pending_single_click_at = Some(now);
            self.pending_single_click_alert_active = alert_active;
        }
    }
}

fn install_cjk_fonts(ctx: &egui::Context) {
    let Some((font_name, font_bytes)) = load_cjk_font() else {
        native_pet_log("no cjk font found for desktop pet; using egui default fonts");
        return;
    };
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        font_name.clone(),
        egui::FontData::from_owned(font_bytes).into(),
    );
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, font_name.clone());
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .insert(0, font_name);
    ctx.set_fonts(fonts);
}

fn load_cjk_font() -> Option<(String, Vec<u8>)> {
    for path in cjk_font_candidates() {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        if !bytes.is_empty() {
            native_pet_log(&format!("desktop pet loaded cjk font: {}", path.display()));
            return Some((
                path.file_stem()
                    .and_then(|value| value.to_str())
                    .unwrap_or("lanchat-cjk")
                    .to_string(),
                bytes,
            ));
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn cjk_font_candidates() -> Vec<&'static Path> {
    vec![
        Path::new("C:/Windows/Fonts/msyh.ttc"),
        Path::new("C:/Windows/Fonts/simhei.ttf"),
        Path::new("C:/Windows/Fonts/simsun.ttc"),
    ]
}

#[cfg(target_os = "macos")]
fn cjk_font_candidates() -> Vec<&'static Path> {
    vec![
        Path::new("/System/Library/Fonts/PingFang.ttc"),
        Path::new("/System/Library/Fonts/STHeiti Light.ttc"),
        Path::new("/System/Library/Fonts/STHeiti Medium.ttc"),
        Path::new("/System/Library/Fonts/Hiragino Sans GB.ttc"),
        Path::new("/Library/Fonts/Arial Unicode.ttf"),
    ]
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
fn cjk_font_candidates() -> Vec<&'static Path> {
    vec![
        Path::new("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc"),
        Path::new("/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc"),
        Path::new("/usr/share/fonts/truetype/wqy/wqy-microhei.ttc"),
    ]
}

impl eframe::App for DesktopPetApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let state = self
            .state
            .lock()
            .map(|value| value.clone())
            .unwrap_or_default();
        let package = self.package.lock().ok().and_then(|value| value.clone());
        if state.disco && !self.was_disco {
            self.disco_origin = ctx.input(|input| input.viewport().outer_rect.map(|rect| rect.min));
        } else if should_restore_disco_origin(self.was_disco, state.disco) {
            if let Some(origin) = self.disco_origin.take() {
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(origin));
            }
        }
        self.was_disco = state.disco;
        if !state.enabled {
            self.transition_runtime(PetEvent::Disable);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            ctx.request_repaint_after(Duration::from_millis(300));
            return;
        }
        if !self.runtime_machine.is_enabled() {
            self.transition_runtime(PetEvent::Enable);
        }
        let runtime_state = self.update_runtime_state(&state);
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        let incoming_call = state.incoming_call_id.is_some();
        if incoming_call {
            self.details_open = true;
        } else if self.details_open && state.pending_count == 0 {
            self.details_open = false;
        }
        let detail_is_open = self.details_open && (state.pending_count > 0 || incoming_call);
        let detail_space = if detail_is_open {
            DETAIL_WIDTH + DETAIL_GAP
        } else {
            0.0
        };
        let detail_transition_space = DETAIL_WIDTH + DETAIL_GAP;
        let desired_size = Vec2::new(self.last_size.x + detail_space, self.last_size.y);
        if detail_is_open != self.detail_last_open {
            if let Some(outer_rect) = ctx.input(|input| input.viewport().outer_rect) {
                if detail_is_open {
                    self.detail_side = if outer_rect.min.x >= detail_transition_space {
                        -1
                    } else {
                        1
                    };
                    if self.detail_side < 0 {
                        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(Pos2::new(
                            outer_rect.min.x - detail_transition_space,
                            outer_rect.min.y,
                        )));
                    }
                } else if self.detail_side < 0 {
                    ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(Pos2::new(
                        outer_rect.min.x + detail_transition_space,
                        outer_rect.min.y,
                    )));
                }
            }
            self.detail_last_open = detail_is_open;
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(desired_size));
        if !self.initial_positioned {
            if let Some((monitor_size, outer_rect)) = ctx.input(|input| {
                let viewport = input.viewport();
                viewport.monitor_size.zip(viewport.outer_rect)
            }) {
                let window_size = outer_rect.size();
                let x = (monitor_size.x - window_size.x - 18.0).max(0.0).round();
                let y = (monitor_size.y - window_size.y - 48.0).max(0.0).round();
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(Pos2::new(x, y)));
                self.initial_positioned = true;
            }
        }
        let scroll = ctx.input(|input| input.raw_scroll_delta.y);
        self.resize_from_scroll(ctx, scroll);
        if state.flashing {
            ctx.request_repaint_after(Duration::from_millis(80));
        }
        ctx.request_repaint_after(Duration::from_millis(160));

        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(Color32::TRANSPARENT))
            .show(ctx, |ui| {
                let available = ui.available_size();
                let pet_available = self.last_size;
                let pet_origin_x = if detail_is_open && self.detail_side < 0 {
                    detail_space
                } else {
                    0.0
                };
                let base = Vec2::new(180.0, 160.0);
                let scale = (pet_available.x / base.x)
                    .min(pet_available.y / base.y)
                    .clamp(0.5, 2.8);
                let pet_margin = 62.0 * scale;
                let mut center = Pos2::new(pet_origin_x + pet_available.x * 0.5, 72.0 * scale);
                if state.disco {
                    let elapsed = self.started_at.elapsed().as_secs_f32();
                    let jump_movement = state.disco_movement_mode != "linear";
                    let (leg_index, x_phase, jump_arc) = if jump_movement {
                        let hop_position = elapsed / DISCO_HOP_SECONDS;
                        let hop_index = hop_position.floor() as i32;
                        let hop_progress = hop_position.fract();
                        let movement_progress = if hop_progress < DISCO_CROUCH_OUT_END {
                            0.0
                        } else if hop_progress > DISCO_LEAP_END {
                            1.0
                        } else {
                            let leap_t = (hop_progress - DISCO_CROUCH_OUT_END)
                                / (DISCO_LEAP_END - DISCO_CROUCH_OUT_END);
                            0.5 - (std::f32::consts::PI * leap_t).cos() * 0.5
                        };
                        let leap_t =
                            if (DISCO_CROUCH_OUT_END..=DISCO_LEAP_END).contains(&hop_progress) {
                                (hop_progress - DISCO_CROUCH_OUT_END)
                                    / (DISCO_LEAP_END - DISCO_CROUCH_OUT_END)
                            } else {
                                0.0
                            };
                        let leg_index = hop_index.div_euclid(DISCO_HOPS_PER_LEG);
                        let step_index = hop_index.rem_euclid(DISCO_HOPS_PER_LEG);
                        let moving_right = leg_index % 2 == 0;
                        self.move_direction = if moving_right { 1 } else { -1 };
                        let leg_progress =
                            (step_index as f32 + movement_progress) / DISCO_HOPS_PER_LEG as f32;
                        let x_phase = if moving_right {
                            leg_progress
                        } else {
                            1.0 - leg_progress
                        };
                        (
                            leg_index,
                            x_phase,
                            (std::f32::consts::PI * leap_t).sin().max(0.0),
                        )
                    } else {
                        let leg_seconds = DISCO_HOP_SECONDS * DISCO_HOPS_PER_LEG as f32;
                        let leg_position = elapsed / leg_seconds;
                        let leg_index = leg_position.floor() as i32;
                        let leg_progress = leg_position.fract();
                        let moving_right = leg_index % 2 == 0;
                        self.move_direction = if moving_right { 1 } else { -1 };
                        (
                            leg_index,
                            if moving_right {
                                leg_progress
                            } else {
                                1.0 - leg_progress
                            },
                            0.0,
                        )
                    };
                    let x_span = (pet_available.x - pet_margin * 2.0).max(0.0);
                    let y_min = pet_margin;
                    let y_max = (pet_available.y - pet_margin).max(y_min);
                    let y_base = y_min
                        + (y_max - y_min)
                            * ((elapsed * 0.36 + leg_index as f32 * 0.37).sin() * 0.5 + 0.5);
                    center = Pos2::new(
                        pet_origin_x + pet_margin + x_span * x_phase,
                        (y_base - 28.0 * scale * jump_arc).clamp(y_min * 0.65, y_max),
                    );
                    ctx.request_repaint_after(Duration::from_millis(35));

                    // 蹦迪时移动透明原生窗口本身，让当前桌宠覆盖整个屏幕范围移动。
                    if let Some((monitor_size, outer_rect)) = ctx.input(|input| {
                        let viewport = input.viewport();
                        viewport.monitor_size.zip(viewport.outer_rect)
                    }) {
                        let window_size = outer_rect.size();
                        let screen_x_span = (monitor_size.x - window_size.x).max(0.0);
                        let screen_y_span = (monitor_size.y - window_size.y).max(0.0);
                        let x = (screen_x_span * x_phase).round();
                        let y_base = screen_y_span
                            * ((elapsed * 0.41 + leg_index as f32 * 0.53).sin() * 0.5 + 0.5);
                        let y = (y_base - (screen_y_span * 0.10).min(92.0) * jump_arc)
                            .clamp(0.0, screen_y_span)
                            .round();
                        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(Pos2::new(
                            x, y,
                        )));
                    }
                } else {
                    self.move_direction = 0;
                }
                let pet_rect = Rect::from_center_size(
                    center + Vec2::new(0.0, 5.0 * scale),
                    Vec2::new(138.0 * scale, 138.0 * scale),
                );
                // Transparent margins do not receive pointer input reliably on every platform.
                // Give the rendered body a generous hit target so dragging works from the whole pet.
                let pet_hit_rect = pet_rect.expand(34.0 * scale);
                let pet_response = ui.interact(
                    pet_hit_rect,
                    ui.id().with("pet-body"),
                    egui::Sense::click_and_drag(),
                );
                let primary_pressed_in_pet = ctx.input(|input| {
                    input.pointer.button_pressed(egui::PointerButton::Primary)
                        && input
                            .pointer
                            .interact_pos()
                            .is_some_and(|pos| pet_hit_rect.contains(pos))
                });
                if primary_pressed_in_pet
                    || pet_response.drag_started_by(egui::PointerButton::Primary)
                {
                    self.pet_press_pos = pet_response
                        .interact_pointer_pos()
                        .or_else(|| ctx.input(|input| input.pointer.interact_pos()));
                    self.pet_press_dragged = false;
                }
                let manually_dragging = ctx.input(|input| {
                    input.pointer.button_down(egui::PointerButton::Primary)
                        && self
                            .pet_press_pos
                            .zip(input.pointer.interact_pos())
                            .is_some_and(|(press_pos, pointer_pos)| {
                                (pointer_pos - press_pos).length_sq() > PET_CLICK_DRAG_THRESHOLD_SQ
                            })
                });
                if manually_dragging && !self.pet_press_dragged {
                    self.pet_press_dragged = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                if manually_dragging && !state.disco {
                    let delta_x = pet_response.drag_delta().x;
                    if delta_x.abs() > 1.0 {
                        self.move_direction = if delta_x < 0.0 { -1 } else { 1 };
                    }
                }
                let alert_active = Self::alert_visual_active(runtime_state, &state);
                let primary_released_in_pet = ctx.input(|input| {
                    input.pointer.button_released(egui::PointerButton::Primary)
                        && input
                            .pointer
                            .interact_pos()
                            .is_some_and(|pos| pet_hit_rect.contains(pos))
                });
                if pet_response.secondary_clicked() {
                    self.pending_single_click_at = None;
                    self.pet_press_dragged = false;
                    self.emit_action("configure_pet", None);
                } else if pet_response.double_clicked_by(egui::PointerButton::Primary) {
                    self.pending_single_click_at = None;
                    self.pet_press_dragged = false;
                    self.emit_pet_double_click_action(ctx);
                } else if pet_response.clicked() || primary_released_in_pet {
                    let click_pos = pet_response
                        .interact_pointer_pos()
                        .or(self.pet_press_pos)
                        .unwrap_or(pet_rect.center());
                    if !self.pet_press_dragged {
                        self.handle_primary_pet_click(ctx, click_pos, alert_active);
                    }
                }
                self.finish_pending_single_click(ctx);
                let dynamic_state = if state.disco || manually_dragging {
                    PetStateKind::Move
                } else {
                    runtime_state
                };
                let direction = match self.move_direction {
                    value if value < 0 => Some("left"),
                    value if value > 0 => Some("right"),
                    _ => None,
                };
                if let Some(package) = package.as_ref() {
                    self.draw_package_frame(
                        ctx,
                        ui.painter(),
                        pet_rect,
                        package,
                        dynamic_state,
                        direction,
                    );
                }
                if detail_is_open {
                    let panel = Self::detail_panel_rect(available, pet_rect, self.detail_side);
                    if incoming_call {
                        self.draw_call_details(ui, panel, &state);
                    } else {
                        self.draw_alert_details(ui, panel, &state);
                    }
                }

                if state.pending_count > 0 || state.latest_sender.is_some() {
                    let mark_rect = Rect::from_center_size(
                        center + Vec2::new(64.0 * scale, -47.0 * scale),
                        Vec2::splat(28.0 * scale),
                    );
                    let mark =
                        ui.interact(mark_rect, ui.id().with("alert-mark"), egui::Sense::click());
                    if mark.clicked() {
                        self.details_open = !self.details_open;
                    }
                    let mark_color = if state.pending_count > 0 {
                        Color32::from_rgb(235, 48, 64)
                    } else {
                        Color32::from_rgb(150, 160, 170)
                    };
                    ui.painter()
                        .circle_filled(mark_rect.center(), 14.0 * scale, mark_color);
                    ui.painter().text(
                        mark_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        if state.pending_count > 0 {
                            state.pending_count.min(99).to_string()
                        } else {
                            "!".to_string()
                        },
                        egui::FontId::proportional(15.0 * scale),
                        Color32::WHITE,
                    );
                }

                if state.latest_sender.is_some() {
                    let temperature_pos = center - Vec2::new(0.0, 58.0 * scale);
                    Self::draw_bold_temperature(
                        ui.painter(),
                        temperature_pos,
                        format!("{}°C", state.temperature),
                        egui::FontId::proportional((14.0 * scale).max(8.0)),
                    );
                }
            });
    }
}
