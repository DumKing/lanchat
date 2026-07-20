use image::ImageReader;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use uuid::Uuid;

pub const DESKTOP_PET_STATES: [PetStateKind; 5] = [
    PetStateKind::Idle,
    PetStateKind::Alert,
    PetStateKind::Move,
    PetStateKind::Interact,
    PetStateKind::Life,
];
pub const DEFAULT_DESKTOP_PET_ID: &str = "violet-tail-girl";
pub const FALLBACK_DESKTOP_PET_ID: &str = "frog-buddy";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PetStateKind {
    Idle,
    Alert,
    Move,
    Interact,
    Life,
}

impl PetStateKind {
    pub fn directory_name(self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::Alert => "Alert",
            Self::Move => "Move",
            Self::Interact => "Interact",
            Self::Life => "Life",
        }
    }

    pub fn from_directory_name(value: &str) -> Option<Self> {
        DESKTOP_PET_STATES
            .into_iter()
            .find(|state| state.directory_name() == value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PetPackageSource {
    BuiltIn,
    Portable,
    User,
}

impl PetPackageSource {
    fn priority(self) -> u8 {
        match self {
            Self::BuiltIn => 0,
            Self::Portable => 1,
            Self::User => 2,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PetResourceRoot {
    pub path: PathBuf,
    pub source: PetPackageSource,
}

impl PetResourceRoot {
    pub fn new(path: PathBuf, source: PetPackageSource) -> Self {
        Self { path, source }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PetClipConfig {
    #[serde(default)]
    pub fps: Option<f32>,
    #[serde(default, rename = "loop")]
    pub loop_mode: Option<String>,
    #[serde(default)]
    pub direction: Option<String>,
    #[serde(default)]
    pub weight: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PetManifest {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub description: String,
    pub resolution: u32,
    pub fps: f32,
    pub transparent: bool,
    pub default_state: String,
    pub states: Value,
    #[serde(default)]
    pub clips: HashMap<String, PetClipConfig>,
}

fn default_schema_version() -> u32 {
    1
}

impl PetManifest {
    fn has_state(&self, state: PetStateKind) -> bool {
        let name = state.directory_name();
        match &self.states {
            Value::Array(values) => values.iter().any(|value| value.as_str() == Some(name)),
            Value::Object(values) => values.contains_key(name),
            _ => false,
        }
    }

    fn state_loop_mode(&self, state: PetStateKind) -> String {
        self.states
            .as_object()
            .and_then(|states| states.get(state.directory_name()))
            .and_then(Value::as_object)
            .and_then(|config| config.get("loop"))
            .and_then(Value::as_str)
            .unwrap_or_else(|| match state {
                PetStateKind::Interact | PetStateKind::Life => "once",
                _ => "repeat",
            })
            .to_string()
    }

    fn state_playback_config(&self, state: PetStateKind) -> PetStatePlaybackConfig {
        let defaults = PetStatePlaybackConfig::defaults_for(state);
        let config = self
            .states
            .as_object()
            .and_then(|states| states.get(state.directory_name()))
            .and_then(Value::as_object);
        let number = |name: &str| {
            config
                .and_then(|value| value.get(name))
                .and_then(Value::as_u64)
                .map(|value| value.min(u32::MAX as u64) as u32)
        };
        PetStatePlaybackConfig {
            min_duration_ms: number("minDurationMs").unwrap_or(defaults.min_duration_ms),
            max_duration_ms: number("maxDurationMs").unwrap_or(defaults.max_duration_ms),
            min_action_count: number("minActionCount").unwrap_or(defaults.min_action_count),
            max_action_count: number("maxActionCount").unwrap_or(defaults.max_action_count),
            min_interval_ms: number("minIntervalMs").unwrap_or(defaults.min_interval_ms),
            max_interval_ms: number("maxIntervalMs").unwrap_or(defaults.max_interval_ms),
        }
        .normalized()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PetStatePlaybackConfig {
    pub min_duration_ms: u32,
    pub max_duration_ms: u32,
    pub min_action_count: u32,
    pub max_action_count: u32,
    pub min_interval_ms: u32,
    pub max_interval_ms: u32,
}

impl PetStatePlaybackConfig {
    pub fn defaults_for(state: PetStateKind) -> Self {
        match state {
            PetStateKind::Idle => Self::new(3000, 7000, 1, 2, 500, 1200),
            PetStateKind::Alert => Self::new(2000, 4000, 1, 2, 250, 700),
            PetStateKind::Move => Self::new(1200, 2400, 2, 4, 120, 420),
            PetStateKind::Interact => Self::new(0, 0, 1, 1, 0, 0),
            PetStateKind::Life => Self::new(0, 0, 2, 4, 800, 2000),
        }
    }

    const fn new(
        min_duration_ms: u32,
        max_duration_ms: u32,
        min_action_count: u32,
        max_action_count: u32,
        min_interval_ms: u32,
        max_interval_ms: u32,
    ) -> Self {
        Self {
            min_duration_ms,
            max_duration_ms,
            min_action_count,
            max_action_count,
            min_interval_ms,
            max_interval_ms,
        }
    }

    pub fn normalized(self) -> Self {
        let (min_duration_ms, max_duration_ms) =
            ordered_pair(self.min_duration_ms, self.max_duration_ms);
        let (min_action_count, max_action_count) = ordered_pair(
            self.min_action_count.clamp(1, 20),
            self.max_action_count.clamp(1, 20),
        );
        let (min_interval_ms, max_interval_ms) = ordered_pair(
            self.min_interval_ms.min(60_000),
            self.max_interval_ms.min(60_000),
        );
        Self {
            min_duration_ms: min_duration_ms.min(300_000),
            max_duration_ms: max_duration_ms.min(300_000),
            min_action_count,
            max_action_count,
            min_interval_ms,
            max_interval_ms,
        }
    }
}

fn ordered_pair(left: u32, right: u32) -> (u32, u32) {
    if left <= right {
        (left, right)
    } else {
        (right, left)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PetFrame {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PetClip {
    pub id: String,
    pub state: PetStateKind,
    pub frames: Vec<PetFrame>,
    pub fps: f32,
    pub loop_mode: String,
    pub direction: Option<String>,
    pub weight: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopPetPackage {
    pub manifest: PetManifest,
    pub source: PetPackageSource,
    pub root: PathBuf,
    pub preview_path: Option<PathBuf>,
    pub icon_path: Option<PathBuf>,
    pub states: HashMap<PetStateKind, Vec<PetClip>>,
    pub warnings: Vec<String>,
}

impl DesktopPetPackage {
    pub fn id(&self) -> &str {
        &self.manifest.id
    }

    pub fn state(&self, state: PetStateKind) -> Option<&Vec<PetClip>> {
        self.states.get(&state)
    }

    pub fn total_frames(&self, state: PetStateKind) -> usize {
        self.state(state)
            .map(|clips| clips.iter().map(|clip| clip.frames.len()).sum())
            .unwrap_or_default()
    }

    pub fn playback_config(&self, state: PetStateKind) -> PetStatePlaybackConfig {
        self.manifest.state_playback_config(state)
    }

    pub fn clip_candidates(&self, state: PetStateKind, direction: Option<&str>) -> Vec<&PetClip> {
        let Some(clips) = self
            .state(state)
            .filter(|clips| !clips.is_empty())
            .or_else(|| self.state(PetStateKind::Idle))
        else {
            return Vec::new();
        };
        if let Some(direction) = direction {
            let directed = clips
                .iter()
                .filter(|clip| clip.direction.as_deref() == Some(direction))
                .collect::<Vec<_>>();
            if !directed.is_empty() {
                return directed;
            }
        }
        clips.iter().collect()
    }

    pub fn clip_by_uniform_index(
        &self,
        state: PetStateKind,
        direction: Option<&str>,
        index: usize,
    ) -> Option<&PetClip> {
        let candidates = self.clip_candidates(state, direction);
        if candidates.is_empty() {
            return None;
        }
        candidates.get(index % candidates.len()).copied()
    }

    pub fn clip_cycle_seconds(clip: &PetClip) -> f32 {
        let frame_steps = match clip.loop_mode.as_str() {
            "ping-pong" if clip.frames.len() > 1 => clip.frames.len() * 2 - 2,
            _ => clip.frames.len(),
        };
        frame_steps.max(1) as f32 / clip.fps.max(0.1)
    }

    pub fn frame_in_clip(clip: &PetClip, elapsed_seconds: f32) -> Option<&PetFrame> {
        if clip.frames.is_empty() {
            return None;
        }
        let raw_index = (elapsed_seconds.max(0.0) * clip.fps.max(0.1)).floor() as usize;
        let index = match clip.loop_mode.as_str() {
            "once" => raw_index.min(clip.frames.len() - 1),
            "ping-pong" if clip.frames.len() > 1 => {
                let span = clip.frames.len() * 2 - 2;
                let offset = raw_index % span;
                if offset < clip.frames.len() {
                    offset
                } else {
                    span - offset
                }
            }
            _ => raw_index % clip.frames.len(),
        };
        clip.frames.get(index)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PetPackageIssue {
    pub root: PathBuf,
    pub source: PetPackageSource,
    pub error: String,
}

#[derive(Debug, Clone, Default)]
pub struct DesktopPetRegistry {
    packages: HashMap<String, DesktopPetPackage>,
    issues: Vec<PetPackageIssue>,
}

impl DesktopPetRegistry {
    pub fn scan_roots(roots: Vec<PetResourceRoot>) -> Self {
        let mut registry = Self::default();
        for root in roots {
            let Ok(entries) = fs::read_dir(&root.path) else {
                continue;
            };
            for entry in entries.flatten() {
                let package_root = entry.path();
                if !package_root.is_dir() || entry.file_name().to_string_lossy().starts_with('.') {
                    continue;
                }
                match load_package(&package_root, root.source) {
                    Ok(package) => {
                        let should_replace = registry
                            .packages
                            .get(package.id())
                            .map(|current| package.source.priority() >= current.source.priority())
                            .unwrap_or(true);
                        if should_replace {
                            registry.packages.insert(package.id().to_string(), package);
                        }
                    }
                    Err(error) => registry.issues.push(PetPackageIssue {
                        root: package_root,
                        source: root.source,
                        error,
                    }),
                }
            }
        }
        registry
    }

    pub fn package(&self, id: &str) -> Option<&DesktopPetPackage> {
        self.packages.get(id)
    }

    pub fn packages(&self) -> Vec<DesktopPetPackage> {
        let mut packages = self.packages.values().cloned().collect::<Vec<_>>();
        packages.sort_by(|left, right| left.manifest.name.cmp(&right.manifest.name));
        packages
    }

    pub fn issues(&self) -> &[PetPackageIssue] {
        &self.issues
    }
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|value| {
            value.is_ascii_lowercase() || value.is_ascii_digit() || value == '-' || value == '_'
        })
}

fn load_package(root: &Path, source: PetPackageSource) -> Result<DesktopPetPackage, String> {
    let manifest_path = root.join("manifest.json");
    let manifest_bytes =
        fs::read(&manifest_path).map_err(|error| format!("读取 manifest.json 失败：{error}"))?;
    let manifest: PetManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| format!("解析 manifest.json 失败：{error}"))?;
    if manifest.schema_version > 2 {
        return Err(format!("不支持 schemaVersion {}", manifest.schema_version));
    }
    if !valid_id(&manifest.id) {
        return Err("桌宠 id 只允许小写字母、数字、短横线和下划线".to_string());
    }
    if root.file_name().and_then(|value| value.to_str()) != Some(manifest.id.as_str()) {
        return Err("资源包目录名必须与 manifest id 一致".to_string());
    }
    if manifest.name.trim().is_empty() {
        return Err("桌宠名称不能为空".to_string());
    }
    if manifest.resolution == 0 || !manifest.fps.is_finite() || manifest.fps <= 0.0 {
        return Err("resolution 和 fps 必须大于 0".to_string());
    }
    if !manifest.transparent {
        return Err("桌宠资源必须声明 transparent=true".to_string());
    }
    if manifest.default_state != "Idle" || !manifest.has_state(PetStateKind::Idle) {
        return Err("defaultState 必须为 Idle，且 states 必须包含 Idle".to_string());
    }

    let mut warnings = Vec::new();
    let mut states = HashMap::new();
    for state in DESKTOP_PET_STATES {
        if !manifest.has_state(state) {
            warnings.push(format!("manifest states 未声明 {}", state.directory_name()));
        }
        match load_state_clips(root, state, &manifest, &mut warnings) {
            Ok(clips) if !clips.is_empty() => {
                states.insert(state, clips);
            }
            Ok(_) if state == PetStateKind::Idle => {
                return Err("Idle 目录至少需要一张有效 PNG".to_string());
            }
            Ok(_) => warnings.push(format!(
                "{} 目录没有有效图片，将回退 Idle",
                state.directory_name()
            )),
            Err(error) if state == PetStateKind::Idle => return Err(error),
            Err(error) => warnings.push(error),
        }
    }

    let preview_path = validated_optional_image(root.join("preview.png"), &mut warnings);
    let icon_path = validated_optional_image(root.join("icon.png"), &mut warnings);
    Ok(DesktopPetPackage {
        preview_path,
        icon_path,
        manifest,
        source,
        root: root.to_path_buf(),
        states,
        warnings,
    })
}

fn validated_optional_image(path: PathBuf, warnings: &mut Vec<String>) -> Option<PathBuf> {
    if !path.is_file() {
        return None;
    }
    match ImageReader::open(&path)
        .and_then(|reader| reader.with_guessed_format())
        .and_then(|reader| reader.decode().map_err(std::io::Error::other))
    {
        Ok(_) => Some(path),
        Err(error) => {
            warnings.push(format!("忽略无法解码的图片 {}：{error}", path.display()));
            None
        }
    }
}

fn load_state_clips(
    root: &Path,
    state: PetStateKind,
    manifest: &PetManifest,
    warnings: &mut Vec<String>,
) -> Result<Vec<PetClip>, String> {
    let state_root = root.join(state.directory_name());
    if !state_root.is_dir() {
        return Ok(Vec::new());
    }

    let mut clips = Vec::new();
    let direct_frames = collect_frames(&state_root, manifest.resolution, warnings)?;
    if !direct_frames.is_empty() {
        clips.push(build_clip("default", state, direct_frames, manifest));
    }
    let mut directories = fs::read_dir(&state_root)
        .map_err(|error| format!("读取 {} 失败：{error}", state_root.display()))?
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .collect::<Vec<_>>();
    directories.sort_by(|left, right| {
        natural_cmp(
            &left.file_name().to_string_lossy(),
            &right.file_name().to_string_lossy(),
        )
    });
    for directory in directories {
        let clip_id = directory.file_name().to_string_lossy().to_string();
        let frames = collect_frames(&directory.path(), manifest.resolution, warnings)?;
        if frames.is_empty() {
            warnings.push(format!(
                "{}/{} 没有有效 PNG",
                state.directory_name(),
                clip_id
            ));
        } else {
            clips.push(build_clip(&clip_id, state, frames, manifest));
        }
    }
    Ok(clips)
}

fn build_clip(
    id: &str,
    state: PetStateKind,
    frames: Vec<PetFrame>,
    manifest: &PetManifest,
) -> PetClip {
    let key = format!("{}/{}", state.directory_name(), id);
    let config = manifest.clips.get(&key).cloned().unwrap_or_default();
    PetClip {
        id: id.to_string(),
        state,
        frames,
        fps: config.fps.unwrap_or(manifest.fps).max(0.1),
        loop_mode: config
            .loop_mode
            .unwrap_or_else(|| manifest.state_loop_mode(state)),
        direction: config.direction,
        weight: config.weight.unwrap_or(1).max(1),
    }
}

fn collect_frames(
    directory: &Path,
    resolution: u32,
    warnings: &mut Vec<String>,
) -> Result<Vec<PetFrame>, String> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| format!("读取 {} 失败：{error}", directory.display()))?
        .flatten()
        .filter(|entry| {
            entry.path().is_file()
                && entry
                    .path()
                    .extension()
                    .and_then(|value| value.to_str())
                    .map(|value| value.eq_ignore_ascii_case("png"))
                    .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        natural_cmp(
            &left.file_name().to_string_lossy(),
            &right.file_name().to_string_lossy(),
        )
    });

    let mut frames = Vec::new();
    for entry in entries {
        let path = entry.path();
        let reader = match ImageReader::open(&path).and_then(|reader| reader.with_guessed_format())
        {
            Ok(reader) => reader,
            Err(error) => {
                warnings.push(format!("忽略无法读取的图片 {}：{error}", path.display()));
                continue;
            }
        };
        let image = match reader.decode() {
            Ok(image) => image,
            Err(error) => {
                warnings.push(format!("忽略无法解码的图片 {}：{error}", path.display()));
                continue;
            }
        };
        if image.width() != resolution || image.height() != resolution {
            warnings.push(format!(
                "{} 尺寸为 {}x{}，manifest 声明为 {}x{}",
                path.display(),
                image.width(),
                image.height(),
                resolution,
                resolution
            ));
        }
        if !image.color().has_alpha() {
            warnings.push(format!("{} 没有 Alpha 通道", path.display()));
        }
        frames.push(PetFrame {
            path,
            width: image.width(),
            height: image.height(),
        });
    }
    Ok(frames)
}

fn natural_cmp(left: &str, right: &str) -> Ordering {
    let mut left_chars = left.chars().peekable();
    let mut right_chars = right.chars().peekable();
    loop {
        match (left_chars.peek(), right_chars.peek()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(left_char), Some(right_char))
                if left_char.is_ascii_digit() && right_char.is_ascii_digit() =>
            {
                let left_number = take_number(&mut left_chars);
                let right_number = take_number(&mut right_chars);
                let order = left_number.cmp(&right_number);
                if order != Ordering::Equal {
                    return order;
                }
            }
            (Some(_), Some(_)) => {
                let left_char = left_chars.next().unwrap().to_ascii_lowercase();
                let right_char = right_chars.next().unwrap().to_ascii_lowercase();
                let order = left_char.cmp(&right_char);
                if order != Ordering::Equal {
                    return order;
                }
            }
        }
    }
}

fn take_number<I>(chars: &mut std::iter::Peekable<I>) -> u64
where
    I: Iterator<Item = char>,
{
    let mut value = 0u64;
    while let Some(character) = chars.peek().copied() {
        if !character.is_ascii_digit() {
            break;
        }
        chars.next();
        value = value
            .saturating_mul(10)
            .saturating_add(character.to_digit(10).unwrap_or_default() as u64);
    }
    value
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PetEvent {
    Enable,
    Disable,
    LifeTimer,
    MoveTimer,
    PointerInteract,
    InteractionFinished,
    MovementFinished,
    LifeFinished,
    AlertRaised,
    AlertCleared,
    PackageChanged,
}

#[derive(Debug, Clone)]
pub struct PetStateMachine {
    enabled: bool,
    current: PetStateKind,
}

impl Default for PetStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

impl PetStateMachine {
    pub fn new() -> Self {
        Self {
            enabled: true,
            current: PetStateKind::Idle,
        }
    }

    pub fn current(&self) -> PetStateKind {
        self.current
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn handle(&mut self, event: PetEvent) {
        match event {
            PetEvent::Disable => self.enabled = false,
            PetEvent::Enable => {
                self.enabled = true;
                self.current = PetStateKind::Idle;
            }
            _ if !self.enabled => {}
            PetEvent::AlertRaised => self.current = PetStateKind::Alert,
            PetEvent::AlertCleared | PetEvent::PackageChanged => self.current = PetStateKind::Idle,
            PetEvent::PointerInteract if self.current != PetStateKind::Alert => {
                self.current = PetStateKind::Interact;
            }
            PetEvent::LifeTimer if self.current == PetStateKind::Idle => {
                self.current = PetStateKind::Life;
            }
            PetEvent::MoveTimer if self.current == PetStateKind::Idle => {
                self.current = PetStateKind::Move;
            }
            PetEvent::InteractionFinished | PetEvent::MovementFinished | PetEvent::LifeFinished => {
                self.current = PetStateKind::Idle
            }
            _ => {}
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopPetSettings {
    pub enabled: bool,
    pub selected_pet_id: Option<String>,
    pub scale: f32,
    pub position_x: Option<f32>,
    pub position_y: Option<f32>,
    pub monitor_id: Option<String>,
    pub alert_mode: String,
    pub stop_hotkey: String,
    pub random_move_enabled: bool,
    pub random_life_enabled: bool,
    #[serde(default = "default_disco_movement_mode")]
    pub disco_movement_mode: String,
}

fn default_disco_movement_mode() -> String {
    "jump".to_string()
}

impl Default for DesktopPetSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            selected_pet_id: None,
            scale: 1.0,
            position_x: None,
            position_y: None,
            monitor_id: None,
            alert_mode: "normal".to_string(),
            stop_hotkey: "Ctrl+Alt+G".to_string(),
            random_move_enabled: true,
            random_life_enabled: true,
            disco_movement_mode: default_disco_movement_mode(),
        }
    }
}

impl DesktopPetSettings {
    pub fn load(path: &Path) -> Self {
        fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| format!("创建桌宠配置目录失败：{error}"))?;
        }
        let temporary = path.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(self)
            .map_err(|error| format!("序列化桌宠配置失败：{error}"))?;
        fs::write(&temporary, bytes).map_err(|error| format!("写入桌宠配置失败：{error}"))?;
        fs::rename(&temporary, path).map_err(|error| format!("保存桌宠配置失败：{error}"))
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DesktopPetRegistrySnapshot {
    pub packages: Vec<DesktopPetPackage>,
    pub issues: Vec<PetPackageIssue>,
}

#[derive(Clone)]
pub struct DesktopPetManager {
    roots: Arc<Vec<PetResourceRoot>>,
    user_root: PathBuf,
    settings_path: PathBuf,
    registry: Arc<RwLock<DesktopPetRegistry>>,
    settings: Arc<Mutex<DesktopPetSettings>>,
    signature: Arc<Mutex<String>>,
    disk_signature: Arc<Mutex<u64>>,
}

impl DesktopPetManager {
    pub fn new(roots: Vec<PetResourceRoot>, user_root: PathBuf, settings_path: PathBuf) -> Self {
        let _ = fs::create_dir_all(&user_root);
        let registry = DesktopPetRegistry::scan_roots(roots.clone());
        let signature = registry_signature(&registry);
        let disk_signature = resource_roots_signature(&roots);
        let mut settings = DesktopPetSettings::load(&settings_path);
        let selection_is_valid = settings
            .selected_pet_id
            .as_deref()
            .is_some_and(|id| registry.package(id).is_some());
        if !selection_is_valid {
            settings.selected_pet_id = registry
                .package(DEFAULT_DESKTOP_PET_ID)
                .or_else(|| registry.package(FALLBACK_DESKTOP_PET_ID))
                .or_else(|| registry.packages.values().next())
                .map(|package| package.id().to_string());
            let _ = settings.save(&settings_path);
        }
        Self {
            roots: Arc::new(roots),
            user_root,
            settings_path: settings_path.clone(),
            registry: Arc::new(RwLock::new(registry)),
            settings: Arc::new(Mutex::new(settings)),
            signature: Arc::new(Mutex::new(signature)),
            disk_signature: Arc::new(Mutex::new(disk_signature)),
        }
    }

    pub fn snapshot(&self) -> DesktopPetRegistrySnapshot {
        let registry = self
            .registry
            .read()
            .unwrap_or_else(|error| error.into_inner());
        DesktopPetRegistrySnapshot {
            packages: registry.packages(),
            issues: registry.issues().to_vec(),
        }
    }

    pub fn refresh(&self) -> bool {
        let next = DesktopPetRegistry::scan_roots(self.roots.as_ref().clone());
        let next_signature = registry_signature(&next);
        let changed = {
            let mut signature = self
                .signature
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let changed = *signature != next_signature;
            *signature = next_signature;
            changed
        };
        *self
            .registry
            .write()
            .unwrap_or_else(|error| error.into_inner()) = next;
        *self
            .disk_signature
            .lock()
            .unwrap_or_else(|error| error.into_inner()) =
            resource_roots_signature(self.roots.as_ref());
        changed
    }

    pub fn refresh_if_changed(&self) -> bool {
        let next = resource_roots_signature(self.roots.as_ref());
        let previous = *self
            .disk_signature
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if next == previous {
            return false;
        }
        self.refresh()
    }

    pub fn settings(&self) -> DesktopPetSettings {
        self.settings
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    pub fn update_settings(
        &self,
        mut settings: DesktopPetSettings,
    ) -> Result<DesktopPetSettings, String> {
        if !settings.scale.is_finite() {
            return Err("桌宠缩放比例无效".to_string());
        }
        settings.scale = settings.scale.clamp(0.3, 3.0);
        settings.disco_movement_mode = match settings.disco_movement_mode.as_str() {
            "linear" => "linear",
            _ => "jump",
        }
        .to_string();
        if let Some(id) = settings.selected_pet_id.as_deref() {
            if self
                .registry
                .read()
                .unwrap_or_else(|error| error.into_inner())
                .package(id)
                .is_none()
            {
                return Err("选择的桌宠资源不存在或校验失败".to_string());
            }
        }
        settings.save(&self.settings_path)?;
        *self
            .settings
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = settings.clone();
        Ok(settings)
    }

    pub fn select(&self, id: &str) -> Result<DesktopPetSettings, String> {
        let id = id.trim();
        if self
            .registry
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .package(id)
            .is_none()
        {
            return Err("桌宠资源不存在或校验失败".to_string());
        }
        let mut settings = self.settings();
        settings.selected_pet_id = Some(id.to_string());
        self.update_settings(settings)
    }

    pub fn selected_package(&self) -> Option<DesktopPetPackage> {
        let settings = self.settings();
        let id = settings.selected_pet_id?;
        self.registry
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .package(&id)
            .cloned()
    }

    pub fn import_package(&self, source: &Path) -> Result<DesktopPetPackage, String> {
        let source = source
            .canonicalize()
            .map_err(|error| format!("读取桌宠资源目录失败：{error}"))?;
        let package = load_package(&source, PetPackageSource::User)?;
        fs::create_dir_all(&self.user_root)
            .map_err(|error| format!("创建桌宠资源目录失败：{error}"))?;
        let staging_root = self.user_root.join(format!(".staging-{}", Uuid::new_v4()));
        let staging_package = staging_root.join(package.id());
        copy_directory(&source, &staging_package)?;
        load_package(&staging_package, PetPackageSource::User).or_else(|error| {
            let _ = fs::remove_dir_all(&staging_root);
            Err(error)
        })?;

        let destination = self.user_root.join(package.id());
        if destination.exists() {
            fs::remove_dir_all(&destination)
                .map_err(|error| format!("替换旧桌宠资源失败：{error}"))?;
        }
        fs::rename(&staging_package, &destination)
            .map_err(|error| format!("安装桌宠资源失败：{error}"))?;
        let _ = fs::remove_dir_all(&staging_root);
        self.refresh();
        self.registry
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .package(package.id())
            .cloned()
            .ok_or_else(|| "桌宠导入后未通过注册校验".to_string())
    }

    pub fn update_playback_configs(
        &self,
        id: &str,
        configs: HashMap<String, PetStatePlaybackConfig>,
    ) -> Result<DesktopPetPackage, String> {
        let id = id.trim();
        let package = self
            .registry
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .package(id)
            .cloned()
            .ok_or_else(|| "桌宠资源不存在或校验失败".to_string())?;
        let editable_root = if package.source == PetPackageSource::User {
            package.root.clone()
        } else {
            fs::create_dir_all(&self.user_root)
                .map_err(|error| format!("创建桌宠资源目录失败：{error}"))?;
            let destination = self.user_root.join(id);
            if destination.exists() {
                fs::remove_dir_all(&destination)
                    .map_err(|error| format!("清理桌宠用户覆盖包失败：{error}"))?;
            }
            copy_directory(&package.root, &destination)?;
            destination
        };

        let manifest_path = editable_root.join("manifest.json");
        let mut manifest: Value = serde_json::from_slice(
            &fs::read(&manifest_path)
                .map_err(|error| format!("读取 manifest.json 失败：{error}"))?,
        )
        .map_err(|error| format!("解析 manifest.json 失败：{error}"))?;
        let states = manifest
            .get_mut("states")
            .and_then(Value::as_object_mut)
            .ok_or_else(|| "manifest states 必须是对象才能编辑动作配置".to_string())?;
        for (state_name, config) in configs {
            let Some(state) = PetStateKind::from_directory_name(&state_name) else {
                return Err(format!("未知桌宠状态：{state_name}"));
            };
            let value = states
                .entry(state.directory_name().to_string())
                .or_insert_with(|| Value::Object(Default::default()));
            let object = value
                .as_object_mut()
                .ok_or_else(|| format!("{} 状态配置必须是对象", state.directory_name()))?;
            let config = config.normalized();
            object.insert(
                "minDurationMs".to_string(),
                Value::from(config.min_duration_ms),
            );
            object.insert(
                "maxDurationMs".to_string(),
                Value::from(config.max_duration_ms),
            );
            object.insert(
                "minActionCount".to_string(),
                Value::from(config.min_action_count),
            );
            object.insert(
                "maxActionCount".to_string(),
                Value::from(config.max_action_count),
            );
            object.insert(
                "minIntervalMs".to_string(),
                Value::from(config.min_interval_ms),
            );
            object.insert(
                "maxIntervalMs".to_string(),
                Value::from(config.max_interval_ms),
            );
        }
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest)
                .map_err(|error| format!("序列化 manifest.json 失败：{error}"))?,
        )
        .map_err(|error| format!("保存 manifest.json 失败：{error}"))?;
        load_package(&editable_root, PetPackageSource::User)?;
        self.refresh();
        self.registry
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .package(id)
            .cloned()
            .ok_or_else(|| "桌宠配置保存后未通过资源校验".to_string())
    }

    pub fn remove_user_package(&self, id: &str) -> Result<(), String> {
        let package = self
            .registry
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .package(id)
            .cloned()
            .ok_or_else(|| "桌宠资源不存在".to_string())?;
        if package.source != PetPackageSource::User {
            return Err("只能删除用户导入的桌宠".to_string());
        }
        let destination = self.user_root.join(package.id());
        if destination.exists() {
            fs::remove_dir_all(&destination)
                .map_err(|error| format!("删除桌宠资源失败：{error}"))?;
        }
        let mut settings = self.settings();
        let removed_selected_package = settings.selected_pet_id.as_deref() == Some(package.id());
        self.refresh();
        if removed_selected_package {
            let registry = self
                .registry
                .read()
                .unwrap_or_else(|error| error.into_inner());
            settings.selected_pet_id = if registry.package(package.id()).is_some() {
                Some(package.id().to_string())
            } else if registry.package(DEFAULT_DESKTOP_PET_ID).is_some() {
                Some(DEFAULT_DESKTOP_PET_ID.to_string())
            } else if registry.package(FALLBACK_DESKTOP_PET_ID).is_some() {
                Some(FALLBACK_DESKTOP_PET_ID.to_string())
            } else {
                registry
                    .packages()
                    .first()
                    .map(|package| package.id().to_string())
            };
            drop(registry);
            settings.save(&self.settings_path)?;
            *self
                .settings
                .lock()
                .unwrap_or_else(|error| error.into_inner()) = settings;
        }
        Ok(())
    }

    pub fn user_root(&self) -> &Path {
        &self.user_root
    }
}

fn registry_signature(registry: &DesktopPetRegistry) -> String {
    let mut values = registry
        .packages()
        .into_iter()
        .map(|package| {
            let counts = DESKTOP_PET_STATES
                .into_iter()
                .map(|state| package.total_frames(state).to_string())
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "{}:{:?}:{}:{}",
                package.id(),
                package.source,
                package.manifest.version,
                counts
            )
        })
        .collect::<Vec<_>>();
    values.extend(
        registry
            .issues()
            .iter()
            .map(|issue| format!("issue:{}:{}", issue.root.display(), issue.error)),
    );
    values.sort();
    values.join("|")
}

fn resource_roots_signature(roots: &[PetResourceRoot]) -> u64 {
    let mut hasher = DefaultHasher::new();
    for root in roots {
        root.path.hash(&mut hasher);
        root.source.priority().hash(&mut hasher);
        hash_directory(&root.path, 0, &mut hasher);
    }
    hasher.finish()
}

fn hash_directory(path: &Path, depth: usize, hasher: &mut DefaultHasher) {
    if depth > 4 {
        return;
    }
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    let mut entries = entries.flatten().collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        entry.file_name().hash(hasher);
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        metadata.len().hash(hasher);
        metadata
            .modified()
            .ok()
            .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|value| value.as_millis())
            .hash(hasher);
        if metadata.is_dir() {
            hash_directory(&entry.path(), depth + 1, hasher);
        }
    }
}

fn copy_directory(source: &Path, destination: &Path) -> Result<(), String> {
    if destination.exists() {
        fs::remove_dir_all(destination)
            .map_err(|error| format!("清理临时导入目录失败：{error}"))?;
    }
    fs::create_dir_all(destination).map_err(|error| format!("创建临时导入目录失败：{error}"))?;
    for entry in fs::read_dir(source).map_err(|error| format!("读取资源包失败：{error}"))? {
        let entry = entry.map_err(|error| format!("读取资源项失败：{error}"))?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("读取资源类型失败：{error}"))?;
        if file_type.is_symlink() {
            return Err("桌宠资源包不能包含符号链接".to_string());
        }
        let target = destination.join(entry.file_name());
        if file_type.is_dir() {
            copy_directory(&entry.path(), &target)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), target).map_err(|error| format!("复制桌宠资源失败：{error}"))?;
        }
    }
    Ok(())
}
