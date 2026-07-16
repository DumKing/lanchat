use crate::desktop_pet::{DesktopPetPackage, PetEvent, PetFrame, PetStateKind, PetStateMachine};
use eframe::egui::{self, Color32, Pos2, Rect, TextureHandle, Vec2};
use image::{GenericImageView, RgbaImage};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use chrono::TimeZone;
use tauri::{AppHandle, Emitter};

const FROG_SHEET_BYTES: &[u8] = include_bytes!("../../public/pet-assets/frog-3d-actions-alpha.png");
const FROG_DISCO_SHEET_BYTES: &[u8] = include_bytes!("../../public/pet-assets/disco/frog-disco-side-hop-v2-alpha.png");
const FROG_ACTION_COLUMNS: u32 = 4;
const FROG_ACTION_ROWS: u32 = 4;
const DETAIL_WIDTH: f32 = 236.0;
const DETAIL_HEIGHT: f32 = 88.0;
const DETAIL_GAP: f32 = 10.0;
const DISCO_HOP_SECONDS: f32 = 0.72;
const DISCO_HOPS_PER_LEG: i32 = 8;
const DISCO_CROUCH_OUT_END: f32 = 0.25;
const DISCO_LEAP_END: f32 = 0.78;
const POSE_IDLE: usize = 0;
const POSE_BLINK: usize = 1;
const POSE_LOOK_RIGHT: usize = 2;
const POSE_LOOK_LEFT: usize = 3;
const POSE_LOOK_UP: usize = 4;
const POSE_SURPRISE: usize = 5;
const POSE_ANGRY: usize = 6;
const POSE_ALERT: usize = 7;
const POSE_LOOK_DOWN: usize = 8;
const POSE_BREATHE_A: usize = 9;
const POSE_BREATHE_B: usize = 10;
const POSE_BREATHE_C: usize = 11;
const POSE_SLOW_BLINK: usize = 12;
const POSE_HALF_BLINK: usize = 13;
const POSE_SWAY: usize = 14;
const ALERT_POSES: [usize; 3] = [POSE_SURPRISE, POSE_ANGRY, POSE_ALERT];

fn native_pet_log(message: &str) {
    let path = std::env::temp_dir().join("lanchat-native-frog.log");
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        use std::io::Write;
        let _ = writeln!(file, "{} {message}", chrono::Utc::now().to_rfc3339());
    }
}

fn run_native_frog_window(
    mut options: eframe::NativeOptions,
    state: Arc<Mutex<NativeFrogPetState>>,
    package: Arc<Mutex<Option<DesktopPetPackage>>>,
    repaint: Arc<Mutex<Option<egui::Context>>>,
    app: AppHandle,
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
        Box::new(move |cc| Ok(Box::new(NativeFrogPetApp::new(cc, state, package, repaint, app)))),
    );
    native_pet_log(&format!("eframe returned: {result:?}"));
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NativeFrogPetState {
    pub enabled: bool,
    pub pending_count: u32,
    pub temperature: u8,
    pub latest_alert_id: Option<String>,
    pub latest_sender: Option<String>,
    pub latest_sender_address: Option<String>,
    pub latest_content: Option<String>,
    pub latest_created_at: Option<i64>,
    pub feedbackable: bool,
    pub flashing: bool,
    pub disco: bool,
    pub theme_accent: Option<String>,
    #[serde(default = "default_true")]
    pub random_move_enabled: bool,
    #[serde(default = "default_true")]
    pub random_life_enabled: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize)]
struct NativeFrogAction {
    action: String,
    alert_id: Option<String>,
}

#[derive(Clone)]
pub struct NativeFrogPetController {
    state: Arc<Mutex<NativeFrogPetState>>,
    package: Arc<Mutex<Option<DesktopPetPackage>>>,
    repaint: Arc<Mutex<Option<egui::Context>>>,
}

impl NativeFrogPetController {
    pub fn start(app: AppHandle) -> Self {
        let state = Arc::new(Mutex::new(NativeFrogPetState {
            enabled: true,
            ..Default::default()
        }));
        let package = Arc::new(Mutex::new(None));
        let repaint = Arc::new(Mutex::new(None));
        let controller = Self {
            state: state.clone(),
            package: package.clone(),
            repaint: repaint.clone(),
        };
        native_pet_log("starting native frog thread");
        std::thread::Builder::new()
            .name("lanchat-native-frog-pet".to_string())
            .spawn(move || {
                native_pet_log("native frog thread entered");
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
                run_native_frog_window(options, state, package, repaint, app);
            })
            .expect("启动原生青蛙桌宠线程失败");
        controller
    }

    pub fn update(&self, next: NativeFrogPetState) {
        let enabled = next.enabled;
        if let Ok(mut state) = self.state.lock() {
            *state = next;
        }
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
        if let Ok(context) = self.repaint.lock() {
            if let Some(context) = context.as_ref() {
                context.send_viewport_cmd(egui::ViewportCommand::Visible(enabled));
                context.request_repaint();
            }
        }
    }

    pub fn set_package(&self, package: Option<DesktopPetPackage>) {
        if let Ok(mut current) = self.package.lock() {
            *current = package;
        }
        if let Ok(context) = self.repaint.lock() {
            if let Some(context) = context.as_ref() {
                context.request_repaint();
            }
        }
    }
}

struct NativeFrogPetApp {
    state: Arc<Mutex<NativeFrogPetState>>,
    package: Arc<Mutex<Option<DesktopPetPackage>>>,
    app: AppHandle,
    details_open: bool,
    detail_last_open: bool,
    detail_side: i8,
    last_size: Vec2,
    started_at: Instant,
    initial_positioned: bool,
    frog_textures: Vec<TextureHandle>,
    disco_textures: Vec<TextureHandle>,
    disco_direction: i8,
    dynamic_package_key: Option<String>,
    dynamic_textures: HashMap<PathBuf, TextureHandle>,
    dynamic_texture_order: VecDeque<PathBuf>,
    runtime_machine: PetStateMachine,
    runtime_state_started: Instant,
    next_idle_action_move: bool,
}

impl NativeFrogPetApp {
    fn new(
        _cc: &eframe::CreationContext<'_>,
        state: Arc<Mutex<NativeFrogPetState>>,
        package: Arc<Mutex<Option<DesktopPetPackage>>>,
        repaint: Arc<Mutex<Option<egui::Context>>>,
        app: AppHandle,
    ) -> Self {
        #[cfg(target_os = "windows")]
        {
            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert(
                "microsoft-yahei".to_owned(),
                egui::FontData::from_static(include_bytes!("C:/Windows/Fonts/msyh.ttc")),
            );
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "microsoft-yahei".to_owned());
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .insert(0, "microsoft-yahei".to_owned());
            _cc.egui_ctx.set_fonts(fonts);
        }
        if let Ok(mut context) = repaint.lock() {
            *context = Some(_cc.egui_ctx.clone());
        }
        let frog_textures = Self::load_frog_textures(&_cc.egui_ctx);
        let disco_textures = Self::load_frog_sheet_textures(
            &_cc.egui_ctx,
            FROG_DISCO_SHEET_BYTES,
            4,
            1,
            "lanchat-3d-frog-disco",
        );
        Self {
            state,
            package,
            app,
            details_open: false,
            detail_last_open: false,
            detail_side: -1,
            last_size: Vec2::new(180.0, 160.0),
            started_at: Instant::now(),
            initial_positioned: false,
            frog_textures,
            disco_textures,
            disco_direction: 0,
            dynamic_package_key: None,
            dynamic_textures: HashMap::new(),
            dynamic_texture_order: VecDeque::new(),
            runtime_machine: PetStateMachine::new(),
            runtime_state_started: Instant::now(),
            next_idle_action_move: false,
        }
    }

    fn emit_action(&self, action: &str, alert_id: Option<String>) {
        let payload = NativeFrogAction {
            action: action.to_string(),
            alert_id,
        };
        let _ = self.app.emit("desktop_pet_action", &payload);
        let _ = self.app.emit("native_frog_action", payload);
    }

    fn load_frog_textures(ctx: &egui::Context) -> Vec<TextureHandle> {
        Self::load_frog_sheet_textures(ctx, FROG_SHEET_BYTES, FROG_ACTION_COLUMNS, FROG_ACTION_ROWS, "lanchat-3d-frog-action")
    }

    fn load_frog_sheet_textures(
        ctx: &egui::Context,
        bytes: &[u8],
        columns: u32,
        rows: u32,
        name: &str,
    ) -> Vec<TextureHandle> {
        let Ok(sheet) = image::load_from_memory(bytes) else {
            native_pet_log(&format!("failed to decode embedded sheet: {name}"));
            return Vec::new();
        };
        let (sheet_width, sheet_height) = sheet.dimensions();
        let cell_width = sheet_width / columns;
        let cell_height = sheet_height / rows;
        let mut textures = Vec::new();
        for row in 0..rows {
            for col in 0..columns {
                let cell = sheet.crop_imm(col * cell_width, row * cell_height, cell_width, cell_height).to_rgba8();
                let cropped = Self::crop_transparent_padding(&cell, 18);
                let size = [cropped.width() as usize, cropped.height() as usize];
                let image = egui::ColorImage::from_rgba_unmultiplied(size, cropped.as_raw());
                textures.push(ctx.load_texture(
                    format!("{name}-{row}-{col}"),
                    image,
                    egui::TextureOptions::LINEAR,
                ));
            }
        }
        textures
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
        }
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
        self.dynamic_textures.insert(frame.path.clone(), texture.clone());
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
        elapsed: f32,
    ) -> bool {
        self.sync_dynamic_package(package);
        let Some(frame) = package.frame_at(state, direction, elapsed) else {
            return false;
        };
        let Some(texture) = self.dynamic_texture(ctx, frame) else {
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

    fn transition_runtime(&mut self, event: PetEvent) {
        let previous = self.runtime_machine.current();
        self.runtime_machine.handle(event);
        if previous != self.runtime_machine.current() {
            self.runtime_state_started = Instant::now();
        }
    }

    fn update_runtime_state(&mut self, state: &NativeFrogPetState) -> PetStateKind {
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
            PetStateKind::Interact if elapsed >= Duration::from_secs(2) => {
                self.transition_runtime(PetEvent::InteractionFinished);
            }
            PetStateKind::Move if elapsed >= Duration::from_secs(5) => {
                self.transition_runtime(PetEvent::MovementFinished);
            }
            PetStateKind::Life if elapsed >= Duration::from_secs(5) => {
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

    fn frog_pose_index(&self, state: &NativeFrogPetState, elapsed: f32) -> usize {
        if state.flashing {
            return ALERT_POSES[((elapsed / 0.34).floor() as usize) % ALERT_POSES.len()];
        }
        let cycle = elapsed % 24.0;
        if cycle < 2.4 {
            return match ((cycle / 0.8).floor() as usize) % 3 {
                0 => POSE_BREATHE_A,
                1 => POSE_BREATHE_B,
                _ => POSE_BREATHE_C,
            };
        }
        if (2.4..2.9).contains(&cycle) {
            return POSE_BLINK;
        }
        if (5.0..6.3).contains(&cycle) {
            return POSE_LOOK_RIGHT;
        }
        if (8.0..9.3).contains(&cycle) {
            return POSE_LOOK_LEFT;
        }
        if (11.0..11.8).contains(&cycle) {
            return POSE_SLOW_BLINK;
        }
        if (13.0..13.8).contains(&cycle) {
            return POSE_HALF_BLINK;
        }
        if (17.0..18.0).contains(&cycle) {
            return POSE_LOOK_UP;
        }
        if (18.5..19.5).contains(&cycle) {
            return POSE_LOOK_DOWN;
        }
        if (21.0..22.0).contains(&cycle) {
            return POSE_SWAY;
        }
        POSE_IDLE
    }

    fn fitted_texture_rect(texture: &TextureHandle, rect: Rect) -> Rect {
        let size = texture.size_vec2();
        if size.x <= 0.0 || size.y <= 0.0 {
            return rect;
        }
        let scale = (rect.width() / size.x).min(rect.height() / size.y);
        Rect::from_center_size(rect.center(), size * scale)
    }

    fn draw_frog_image(&self, painter: &egui::Painter, rect: Rect, pose_index: usize) {
        if let Some(texture) = self.frog_textures.get(pose_index).or_else(|| self.frog_textures.first()) {
            painter.image(
                texture.id(),
                Self::fitted_texture_rect(texture, rect),
                Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
    }

    fn draw_disco_frog_image(&self, painter: &egui::Painter, rect: Rect, hop_progress: f32) {
        let crouching = hop_progress < DISCO_CROUCH_OUT_END || hop_progress > DISCO_LEAP_END;
        let index = match self.disco_direction {
            direction if direction < 0 => if crouching { 0 } else { 1 },
            direction if direction > 0 => if crouching { 2 } else { 3 },
            _ => 2,
        };
        let draw_rect = if crouching {
            rect
        } else {
            Rect::from_center_size(rect.center() - Vec2::new(0.0, rect.height() * 0.04), rect.size())
        };
        if let Some(texture) = self.disco_textures.get(index).or_else(|| self.disco_textures.get(2)) {
            painter.image(
                texture.id(),
                Self::fitted_texture_rect(texture, draw_rect),
                Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        } else {
            self.draw_frog_image(painter, draw_rect, POSE_ALERT);
        }
    }

    fn format_alert_time(value: Option<i64>) -> String {
        let Some(value) = value else {
            return String::new();
        };
        let millis = if value.abs() > 10_000_000_000 { value } else { value * 1000 };
        chrono::Local
            .timestamp_millis_opt(millis)
            .single()
            .map(|time| time.format("%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| value.to_string())
    }

    fn parse_hex_color(value: Option<&str>, fallback: Color32) -> Color32 {
        let Some(value) = value.map(str::trim).and_then(|value| value.strip_prefix('#')) else {
            return fallback;
        };
        if value.len() != 6 {
            return fallback;
        }
        let Ok(red) = u8::from_str_radix(&value[0..2], 16) else { return fallback; };
        let Ok(green) = u8::from_str_radix(&value[2..4], 16) else { return fallback; };
        let Ok(blue) = u8::from_str_radix(&value[4..6], 16) else { return fallback; };
        Color32::from_rgb(red, green, blue)
    }

    fn alert_detail_background(state: &NativeFrogPetState) -> Color32 {
        let accent = Self::parse_hex_color(state.theme_accent.as_deref(), Color32::from_rgb(22, 119, 255));
        Color32::from_rgba_unmultiplied(
            ((accent.r() as u16 + 255 * 5) / 6) as u8,
            ((accent.g() as u16 + 255 * 5) / 6) as u8,
            ((accent.b() as u16 + 255 * 5) / 6) as u8,
            232,
        )
    }

    fn detail_panel_rect(available: Vec2, frog_rect: Rect, side: i8) -> Rect {
        let y = (frog_rect.center().y - DETAIL_HEIGHT * 0.5)
            .clamp(6.0, (available.y - DETAIL_HEIGHT - 6.0).max(6.0));
        let x = if side < 0 {
            (frog_rect.left() - DETAIL_GAP - DETAIL_WIDTH).max(6.0)
        } else {
            (frog_rect.right() + DETAIL_GAP).min((available.x - DETAIL_WIDTH - 6.0).max(6.0))
        };
        Rect::from_min_size(Pos2::new(x, y), Vec2::new(DETAIL_WIDTH, DETAIL_HEIGHT))
    }

    fn draw_alert_details(&mut self, ui: &mut egui::Ui, panel: Rect, state: &NativeFrogPetState) {
        let painter = ui.painter();
        painter.rect_filled(panel, 12.0, Self::alert_detail_background(state));
        let text_color = Color32::from_rgb(32, 38, 45);
        let muted_color = Color32::from_rgb(116, 126, 138);
        let sender = state.latest_sender.as_deref().unwrap_or("告警");
        let sender_address = state.latest_sender_address.as_deref().unwrap_or("未知 IP");
        let sender_line = format!("{}：{}", sender, sender_address);
        let created_at = Self::format_alert_time(state.latest_created_at);
        let title = state.latest_content.as_deref().unwrap_or("");

        painter.text(panel.left_top() + Vec2::new(12.0, 10.0), egui::Align2::LEFT_TOP, sender_line, egui::FontId::proportional(11.0), muted_color);
        painter.text(panel.left_top() + Vec2::new(12.0, 34.0), egui::Align2::LEFT_TOP, title, egui::FontId::proportional(13.0), text_color);
        painter.text(panel.right_bottom() + Vec2::new(-12.0, -18.0), egui::Align2::RIGHT_CENTER, created_at, egui::FontId::proportional(10.5), muted_color);

        if state.feedbackable {
            let real = Rect::from_min_size(panel.left_bottom() + Vec2::new(12.0, -30.0), Vec2::new(46.0, 22.0));
            let false_alarm = Rect::from_min_size(panel.left_bottom() + Vec2::new(66.0, -30.0), Vec2::new(46.0, 22.0));
            painter.rect_filled(real, 7.0, Color32::from_rgb(28, 170, 106));
            painter.rect_filled(false_alarm, 7.0, Color32::from_rgb(210, 68, 75));
            painter.text(real.center(), egui::Align2::CENTER_CENTER, "真实", egui::FontId::proportional(11.5), Color32::WHITE);
            painter.text(false_alarm.center(), egui::Align2::CENTER_CENTER, "虚假", egui::FontId::proportional(11.5), Color32::WHITE);
            if ui.interact(real, ui.id().with("feedback-real"), egui::Sense::click()).clicked() {
                self.emit_action("feedback_real", state.latest_alert_id.clone());
            }
            if ui.interact(false_alarm, ui.id().with("feedback-false"), egui::Sense::click()).clicked() {
                self.emit_action("feedback_false", state.latest_alert_id.clone());
            }
        }
    }

    fn draw_bold_temperature(painter: &egui::Painter, pos: Pos2, text: String, font: egui::FontId) {
        let color = Color32::from_rgb(228, 58, 68);
        painter.text(pos, egui::Align2::CENTER_CENTER, text.clone(), font.clone(), color);
        painter.text(pos + Vec2::new(0.55, 0.0), egui::Align2::CENTER_CENTER, text, font, color);
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
}

impl eframe::App for NativeFrogPetApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let state = self.state.lock().map(|value| value.clone()).unwrap_or_default();
        let package = self.package.lock().ok().and_then(|value| value.clone());
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
        if self.details_open && state.pending_count == 0 {
            self.details_open = false;
        }
        let detail_is_open = self.details_open && state.pending_count > 0;
        let detail_space = if detail_is_open { DETAIL_WIDTH + DETAIL_GAP } else { 0.0 };
        let detail_transition_space = DETAIL_WIDTH + DETAIL_GAP;
        let desired_size = Vec2::new(self.last_size.x + detail_space, self.last_size.y);
        if detail_is_open != self.detail_last_open {
            if let Some(outer_rect) = ctx.input(|input| input.viewport().outer_rect) {
                if detail_is_open {
                    self.detail_side = if outer_rect.min.x >= detail_transition_space { -1 } else { 1 };
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
                let pet_origin_x = if detail_is_open && self.detail_side < 0 { detail_space } else { 0.0 };
                let base = Vec2::new(180.0, 160.0);
                let scale = (pet_available.x / base.x).min(pet_available.y / base.y).clamp(0.5, 2.8);
                let frog_margin = 62.0 * scale;
                let mut center = Pos2::new(pet_origin_x + pet_available.x * 0.5, 72.0 * scale);
                let mut disco_hop_progress = 0.0;
                if state.disco {
                    let elapsed = self.started_at.elapsed().as_secs_f32();
                    let hop_position = elapsed / DISCO_HOP_SECONDS;
                    let hop_index = hop_position.floor() as i32;
                    let hop_progress = hop_position.fract();
                    disco_hop_progress = hop_progress;
                    let movement_progress = if hop_progress < DISCO_CROUCH_OUT_END {
                        0.0
                    } else if hop_progress > DISCO_LEAP_END {
                        1.0
                    } else {
                        let leap_t = (hop_progress - DISCO_CROUCH_OUT_END) / (DISCO_LEAP_END - DISCO_CROUCH_OUT_END);
                        0.5 - (std::f32::consts::PI * leap_t).cos() * 0.5
                    };
                    let leap_t = if (DISCO_CROUCH_OUT_END..=DISCO_LEAP_END).contains(&hop_progress) {
                        (hop_progress - DISCO_CROUCH_OUT_END) / (DISCO_LEAP_END - DISCO_CROUCH_OUT_END)
                    } else {
                        0.0
                    };
                    let leg_index = hop_index.div_euclid(DISCO_HOPS_PER_LEG);
                    let step_index = hop_index.rem_euclid(DISCO_HOPS_PER_LEG);
                    let moving_right = leg_index % 2 == 0;
                    self.disco_direction = if moving_right { 1 } else { -1 };
                    let leg_progress = (step_index as f32 + movement_progress) / DISCO_HOPS_PER_LEG as f32;
                    let x_phase = if moving_right { leg_progress } else { 1.0 - leg_progress };
                    let jump_arc = (std::f32::consts::PI * leap_t).sin().max(0.0);
                    let x_span = (pet_available.x - frog_margin * 2.0).max(0.0);
                    let y_min = frog_margin;
                    let y_max = (pet_available.y - frog_margin).max(y_min);
                    let y_base = y_min + (y_max - y_min) * ((elapsed * 0.36 + leg_index as f32 * 0.37).sin() * 0.5 + 0.5);
                    center = Pos2::new(
                        pet_origin_x + frog_margin + x_span * x_phase,
                        (y_base - 28.0 * scale * jump_arc).clamp(y_min * 0.65, y_max),
                    );
                    ctx.request_repaint_after(Duration::from_millis(35));

                    // 蹦迪时移动透明原生窗口本身，青蛙才能覆盖整个屏幕范围移动。
                    if let Some((monitor_size, outer_rect)) = ctx.input(|input| {
                        let viewport = input.viewport();
                        viewport.monitor_size.zip(viewport.outer_rect)
                    }) {
                        let window_size = outer_rect.size();
                        let screen_x_span = (monitor_size.x - window_size.x).max(0.0);
                        let screen_y_span = (monitor_size.y - window_size.y).max(0.0);
                        let x = (screen_x_span * x_phase).round();
                        let y_base = screen_y_span * ((elapsed * 0.41 + leg_index as f32 * 0.53).sin() * 0.5 + 0.5);
                        let y = (y_base - (screen_y_span * 0.10).min(92.0) * jump_arc)
                            .clamp(0.0, screen_y_span)
                            .round();
                        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(Pos2::new(x, y)));
                    }
                } else {
                    self.disco_direction = 0;
                }
                let elapsed = self.started_at.elapsed().as_secs_f32();
                let pose_index = self.frog_pose_index(&state, elapsed);
                let frog_rect = Rect::from_center_size(
                    center + Vec2::new(0.0, 5.0 * scale),
                    Vec2::new(138.0 * scale, 138.0 * scale),
                );
                let frog_response = ui.interact(frog_rect, ui.id().with("frog-body"), egui::Sense::click_and_drag());
                if frog_response.drag_started_by(egui::PointerButton::Primary) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                if frog_response.double_clicked() {
                    let ctrl_pressed = ctx.input(|input| input.modifiers.ctrl);
                    if ctrl_pressed {
                        self.emit_action("broadcast_disco_alert", None);
                    } else {
                        self.emit_action("quick_alert", None);
                    }
                } else if frog_response.clicked() {
                    if runtime_state != PetStateKind::Alert {
                        self.transition_runtime(PetEvent::PointerInteract);
                    }
                    self.emit_action("stop_visuals", None);
                }
                let dynamic_state = if state.disco {
                    PetStateKind::Move
                } else {
                    runtime_state
                };
                let direction = match self.disco_direction {
                    value if value < 0 => Some("left"),
                    value if value > 0 => Some("right"),
                    _ => None,
                };
                let package_elapsed = self.runtime_state_started.elapsed().as_secs_f32();
                let package_drawn = package.as_ref().is_some_and(|package| {
                    self.draw_package_frame(
                        ctx,
                        ui.painter(),
                        frog_rect,
                        package,
                        dynamic_state,
                        direction,
                        package_elapsed,
                    )
                });
                if !package_drawn {
                    if state.disco {
                        self.draw_disco_frog_image(ui.painter(), frog_rect, disco_hop_progress);
                    } else {
                        self.draw_frog_image(ui.painter(), frog_rect, pose_index);
                    }
                }
                if detail_is_open {
                    let panel = Self::detail_panel_rect(available, frog_rect, self.detail_side);
                    self.draw_alert_details(ui, panel, &state);
                }

                if state.pending_count > 0 || state.latest_sender.is_some() {
                    let mark_rect = Rect::from_center_size(
                        center + Vec2::new(64.0 * scale, -47.0 * scale),
                        Vec2::splat(28.0 * scale),
                    );
                    let mark = ui.interact(mark_rect, ui.id().with("alert-mark"), egui::Sense::click());
                    if mark.clicked() {
                        self.details_open = !self.details_open;
                    }
                    let mark_color = if state.pending_count > 0 { Color32::from_rgb(235, 48, 64) } else { Color32::from_rgb(150, 160, 170) };
                    ui.painter().circle_filled(mark_rect.center(), 14.0 * scale, mark_color);
                    ui.painter().text(mark_rect.center(), egui::Align2::CENTER_CENTER, if state.pending_count > 0 { state.pending_count.min(99).to_string() } else { "!".to_string() }, egui::FontId::proportional(15.0 * scale), Color32::WHITE);
                }

                if state.latest_sender.is_some() {
                    let temperature_pos = center - Vec2::new(0.0, 58.0 * scale);
                    Self::draw_bold_temperature(ui.painter(), temperature_pos, format!("{}°C", state.temperature), egui::FontId::proportional((14.0 * scale).max(8.0)));
                }
            });

    }
}
