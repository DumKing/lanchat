use image::imageops::FilterType;
use ndarray::Array4;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;

const DETECTOR_SIZE: u32 = 640;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FaceMonitorLocalSettings {
    pub enabled: bool,
    pub device_id: Option<String>,
    pub pause_during_call: bool,
    pub sample_fps: u8,
}

impl Default for FaceMonitorLocalSettings {
    fn default() -> Self {
        Self { enabled: false, device_id: None, pause_during_call: false, sample_fps: 2 }
    }
}

impl FaceMonitorLocalSettings {
    pub fn normalized(mut self) -> Self {
        self.device_id = self.device_id.map(|value| value.trim().to_string()).filter(|value| !value.is_empty());
        self.sample_fps = self.sample_fps.clamp(1, 5);
        self
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FaceMonitorStatus {
    pub supported: bool,
    pub enabled: bool,
    pub model_assets_ready: bool,
    pub model_ready: bool,
    pub recognizer_ready: bool,
    pub queue_busy: bool,
    pub accepted_frames: u64,
    pub dropped_frames: u64,
    pub model_version: Option<String>,
    pub last_detection_score: Option<u8>,
    pub detected_faces: u8,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PresenceDetection {
    pub confidence: u8,
    pub detected_faces: u8,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FaceModelManifest {
    schema_version: u8,
    model_version: String,
    detector: FaceModelAsset,
    #[serde(default)]
    recognizer: Option<FaceModelAsset>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FaceModelAsset { file: String, sha256: String }

#[derive(Debug, Clone, Default)]
struct FaceModelState {
    ready: bool,
    version: Option<String>,
    error: Option<String>,
    detector_path: Option<PathBuf>,
    recognizer_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Default)]
struct HitGateState { consecutive_hits: u8, last_alert_at: i64 }

/// Local camera face-presence detector. It only answers whether a face is present
/// in the current frame; it neither identifies people nor stores frames/photos.
pub struct FaceMonitorRuntime {
    settings: Mutex<FaceMonitorLocalSettings>,
    busy: AtomicBool,
    accepted_frames: AtomicU64,
    dropped_frames: AtomicU64,
    hit_state: Mutex<HashMap<String, HitGateState>>,
    model_state: FaceModelState,
    detector: Option<Mutex<ort::session::Session>>,
    recognizer: Option<Mutex<ort::session::Session>>,
    last_detection: Mutex<Option<PresenceDetection>>,
    runtime_error: Mutex<Option<String>>,
}

impl Default for FaceMonitorRuntime {
    fn default() -> Self {
        Self {
            settings: Mutex::new(FaceMonitorLocalSettings::default()), busy: AtomicBool::new(false),
            accepted_frames: AtomicU64::new(0), dropped_frames: AtomicU64::new(0),
            hit_state: Mutex::new(HashMap::new()), detector: None, recognizer: None, last_detection: Mutex::new(None),
            runtime_error: Mutex::new(None),
            model_state: FaceModelState { error: Some("人脸检测模型尚未安装".to_string()), ..Default::default() },
        }
    }
}

impl FaceMonitorRuntime {
    pub fn from_resource_dirs(resource_dir: Option<PathBuf>) -> Self {
        let mut candidates = resource_dir.into_iter().flat_map(|path| {
            vec![path.join("object-models"), path.join("resources").join("object-models")]
        }).collect::<Vec<_>>();
        candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources").join("object-models"));

        let mut reasons = Vec::new();
        let mut model_state = None;
        for path in &candidates {
            match model_state_from_dir(path) { Ok(state) => { model_state = Some(state); break; }, Err(error) => reasons.push(error) }
        }
        let mut runtime = Self {
            model_state: model_state.unwrap_or_else(|| FaceModelState {
                error: reasons.into_iter().next().or_else(|| Some("未找到人脸检测模型资源目录".to_string())), ..Default::default()
            }),
            ..Self::default()
        };
        if let Some(path) = runtime.model_state.detector_path.clone() {
            match ort::session::Session::builder().and_then(|mut builder| builder.commit_from_file(path)) {
                Ok(session) => runtime.detector = Some(Mutex::new(session)),
                Err(error) => {
                    runtime.model_state.ready = false;
                    runtime.model_state.error = Some(format!("人脸检测模型加载失败：{error}"));
                }
            }
        }
        // 识别模型加载失败不影响存在检测，仅使识别能力不可用。
        if let Some(path) = runtime.model_state.recognizer_path.clone() {
            match ort::session::Session::builder().and_then(|mut builder| builder.commit_from_file(path)) {
                Ok(session) => runtime.recognizer = Some(Mutex::new(session)),
                Err(error) => {
                    runtime.recognizer = None;
                    if runtime.model_state.error.is_none() {
                        runtime.model_state.error = Some(format!("人脸识别模型加载失败：{error}"));
                    }
                }
            }
        }
        runtime
    }

    pub fn settings(&self) -> FaceMonitorLocalSettings { self.settings.lock().map(|value| value.clone()).unwrap_or_default() }

    pub fn update_settings(&self, settings: FaceMonitorLocalSettings) -> FaceMonitorLocalSettings {
        let settings = settings.normalized();
        if let Ok(mut current) = self.settings.lock() { *current = settings.clone(); }
        settings
    }

    pub fn status(&self) -> FaceMonitorStatus {
        let last_detection = self.last_detection.lock().ok().and_then(|value| value.clone());
        let runtime_error = self.runtime_error.lock().ok().and_then(|value| value.clone());
        FaceMonitorStatus {
            supported: cfg!(target_os = "windows"), enabled: self.settings().enabled,
            model_assets_ready: cfg!(target_os = "windows") && self.model_state.ready,
            model_ready: cfg!(target_os = "windows") && self.detector.is_some(),
            recognizer_ready: cfg!(target_os = "windows") && self.recognizer.is_some(),
            queue_busy: self.busy.load(Ordering::Relaxed), accepted_frames: self.accepted_frames.load(Ordering::Relaxed),
            dropped_frames: self.dropped_frames.load(Ordering::Relaxed), model_version: self.model_state.version.clone(),
            last_detection_score: last_detection.as_ref().map(|value| value.confidence),
            detected_faces: last_detection.map(|value| value.detected_faces).unwrap_or(0),
            last_error: if cfg!(target_os = "windows") { runtime_error.or_else(|| self.model_state.error.clone()) }
                else { Some("摄像头人脸出现检测第一期仅支持 Windows".to_string()) },
        }
    }

    pub fn analyze_frame(&self, bytes: &[u8], width: u32, height: u32) -> Result<Option<PresenceDetection>, String> {
        if !self.settings().enabled || self.detector.is_none() || bytes.is_empty() || width == 0 || height == 0 { return Ok(None); }
        if self.busy.swap(true, Ordering::AcqRel) {
            self.dropped_frames.fetch_add(1, Ordering::Relaxed);
            return Ok(None);
        }
        self.accepted_frames.fetch_add(1, Ordering::Relaxed);
        let result = self.detect_presence(bytes);
        self.busy.store(false, Ordering::Release);
        match result {
            Ok(detection) => {
                if let Ok(mut last) = self.last_detection.lock() { *last = detection.clone(); }
                if let Ok(mut error) = self.runtime_error.lock() { *error = None; }
                Ok(detection)
            }
            Err(error) => {
                if let Ok(mut last_error) = self.runtime_error.lock() { *last_error = Some(error.clone()); }
                Err(error)
            }
        }
    }

    fn detect_presence(&self, bytes: &[u8]) -> Result<Option<PresenceDetection>, String> {
        let image = image::load_from_memory(bytes).map_err(|error| format!("摄像头采样帧无法解码：{error}"))?.to_rgb8();
        let resized = image::imageops::resize(&image, DETECTOR_SIZE, DETECTOR_SIZE, FilterType::Triangle);
        let mut input = Array4::<f32>::zeros((1, 3, DETECTOR_SIZE as usize, DETECTOR_SIZE as usize));
        for y in 0..DETECTOR_SIZE as usize {
            for x in 0..DETECTOR_SIZE as usize {
                let pixel = resized.get_pixel(x as u32, y as u32);
                // YuNet follows OpenCV DNN BGR input order and uses 0..255 values.
                input[[0, 0, y, x]] = f32::from(pixel[2]);
                input[[0, 1, y, x]] = f32::from(pixel[1]);
                input[[0, 2, y, x]] = f32::from(pixel[0]);
            }
        }
        let detector = self.detector.as_ref().ok_or_else(|| "人脸检测模型未就绪".to_string())?;
        let mut session = detector.lock().map_err(|_| "人脸检测器被占用".to_string())?;
        let outputs = session.run(ort::inputs![ort::value::TensorRef::from_array_view(&input)
            .map_err(|error| format!("构造检测输入失败：{error}"))?]).map_err(|error| format!("人脸检测推理失败：{error}"))?;
        let mut best = 0.0_f32;
        for (cls_index, object_index) in [(0usize, 3usize), (1, 4), (2, 5)] {
            let (_, cls) = outputs[cls_index].try_extract_tensor::<f32>().map_err(|error| format!("读取检测结果失败：{error}"))?;
            let (_, objectness) = outputs[object_index].try_extract_tensor::<f32>().map_err(|error| format!("读取检测结果失败：{error}"))?;
            for (&class_score, &object_score) in cls.iter().zip(objectness.iter()) {
                best = best.max(normalize_score(class_score) * normalize_score(object_score));
            }
        }
        // YuNet emits several anchors for one face. Presence detection only needs the strongest one.
        if best < 0.60 { return Ok(None); }
        Ok(Some(PresenceDetection { confidence: (best * 100.0).round().clamp(0.0, 100.0) as u8, detected_faces: 1 }))
    }

    pub fn accept_match(&self, key: &str, confidence: u8, min_confidence: u8, required_hits: u8, cooldown_seconds: u32, now: i64) -> bool {
        if confidence < min_confidence.min(100) || key.trim().is_empty() { return false; }
        let Ok(mut states) = self.hit_state.lock() else { return false; };
        let state = states.entry(key.to_string()).or_default();
        state.consecutive_hits = state.consecutive_hits.saturating_add(1);
        if state.consecutive_hits < required_hits.clamp(1, 20) { return false; }
        if state.last_alert_at > 0 && now.saturating_sub(state.last_alert_at) < i64::from(cooldown_seconds.clamp(5, 86_400)) * 1000 { return false; }
        state.last_alert_at = now;
        state.consecutive_hits = 0;
        true
    }
}

fn normalize_score(value: f32) -> f32 {
    if (0.0..=1.0).contains(&value) { value } else { 1.0 / (1.0 + (-value).exp()) }
}

fn model_state_from_dir(dir: &Path) -> Result<FaceModelState, String> {
    let manifest_path = dir.join("manifest.json");
    let manifest_text = fs::read_to_string(&manifest_path).map_err(|_| format!("未找到模型清单：{}", manifest_path.display()))?;
    let manifest: FaceModelManifest = serde_json::from_str(&manifest_text).map_err(|error| format!("模型清单格式无效：{error}"))?;
    if manifest.schema_version < 1 || manifest.schema_version > 2 { return Err(format!("不支持的人脸检测模型清单版本：{}", manifest.schema_version)); }
    if manifest.model_version.trim().is_empty() || manifest.model_version == "uninstalled" { return Err("人脸检测模型尚未安装".to_string()); }
    let detector_path = validate_model_asset(dir, "检测", &manifest.detector)?;
    let recognizer_path = match manifest.recognizer {
        Some(asset) => Some(validate_model_asset(dir, "识别", &asset)?),
        None => None,
    };
    Ok(FaceModelState { ready: true, version: Some(manifest.model_version), error: None, detector_path: Some(detector_path), recognizer_path })
}

fn validate_model_asset(dir: &Path, label: &str, asset: &FaceModelAsset) -> Result<PathBuf, String> {
    if asset.file.trim().is_empty() || asset.sha256.len() != 64 { return Err(format!("{label}模型清单不完整")); }
    let path = dir.join(&asset.file);
    let bytes = fs::read(&path).map_err(|_| format!("缺少{label}模型：{}", path.display()))?;
    if !hex::encode(Sha256::digest(&bytes)).eq_ignore_ascii_case(&asset.sha256) { return Err(format!("{label}模型校验失败")); }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_are_clamped_to_safe_sampling_range() {
        let settings = FaceMonitorLocalSettings { enabled: true, device_id: Some("  camera-1  ".to_string()), pause_during_call: false, sample_fps: 99 }.normalized();
        assert_eq!(settings.sample_fps, 5);
        assert_eq!(settings.device_id.as_deref(), Some("camera-1"));
    }

    #[test]
    fn disabled_runtime_rejects_frames_without_retaining_them() {
        let runtime = FaceMonitorRuntime::default();
        assert!(runtime.analyze_frame(&[1, 2, 3], 16, 16).unwrap().is_none());
        assert_eq!(runtime.status().accepted_frames, 0);
    }

    #[test]
    fn match_gate_requires_threshold_hits_and_cooldown() {
        let runtime = FaceMonitorRuntime::default();
        assert!(!runtime.accept_match("presence", 79, 80, 2, 60, 1_000));
        assert!(!runtime.accept_match("presence", 90, 80, 2, 60, 2_000));
        assert!(runtime.accept_match("presence", 90, 80, 2, 60, 3_000));
        assert!(!runtime.accept_match("presence", 90, 80, 1, 60, 4_000));
        assert!(runtime.accept_match("presence", 90, 80, 1, 60, 64_000));
    }

    #[test]
    fn model_manifest_requires_one_verified_detector_asset() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("detector.onnx"), b"detector").unwrap();
        let detector = hex::encode(Sha256::digest(b"detector"));
        fs::write(temp.path().join("manifest.json"), format!(r#"{{"schemaVersion":1,"modelVersion":"test-1","detector":{{"file":"detector.onnx","sha256":"{detector}"}}}}"#)).unwrap();
        let state = model_state_from_dir(temp.path()).unwrap();
        assert!(state.ready);
        assert_eq!(state.version.as_deref(), Some("test-1"));
        assert!(state.recognizer_path.is_none());
    }

    #[test]
    fn model_manifest_v2_includes_recognizer_asset() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("detector.onnx"), b"detector").unwrap();
        fs::write(temp.path().join("recognizer.onnx"), b"recognizer").unwrap();
        let detector = hex::encode(Sha256::digest(b"detector"));
        let recognizer = hex::encode(Sha256::digest(b"recognizer"));
        fs::write(temp.path().join("manifest.json"), format!(
            r#"{{"schemaVersion":2,"modelVersion":"test-2","detector":{{"file":"detector.onnx","sha256":"{detector}"}},"recognizer":{{"file":"recognizer.onnx","sha256":"{recognizer}"}}}}"#
        )).unwrap();
        let state = model_state_from_dir(temp.path()).unwrap();
        assert!(state.ready);
        assert!(state.recognizer_path.is_some());
    }

    #[test]
    fn model_manifest_v2_rejects_bad_recognizer_hash() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("detector.onnx"), b"detector").unwrap();
        fs::write(temp.path().join("recognizer.onnx"), b"recognizer").unwrap();
        let detector = hex::encode(Sha256::digest(b"detector"));
        let wrong = hex::encode(Sha256::digest(b"other"));
        fs::write(temp.path().join("manifest.json"), format!(
            r#"{{"schemaVersion":2,"modelVersion":"test-2","detector":{{"file":"detector.onnx","sha256":"{detector}"}},"recognizer":{{"file":"recognizer.onnx","sha256":"{wrong}"}}}}"#
        )).unwrap();
        assert!(model_state_from_dir(temp.path()).is_err());
    }

    #[test]
    fn bundled_onnx_detector_can_be_opened_by_onnx_runtime() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/object-models");
        let mut detector = ort::session::Session::builder().unwrap().commit_from_file(root.join("presence-detector.onnx")).unwrap();
        assert_eq!(detector.inputs().len(), 1);
        assert!(detector.outputs().len() >= 6);
        let input = Array4::<f32>::zeros((1, 3, 640, 640));
        let outputs = detector.run(ort::inputs![ort::value::TensorRef::from_array_view(&input).unwrap()]).unwrap();
        let (_, cls8) = outputs[0].try_extract_tensor::<f32>().unwrap();
        assert_eq!(cls8.len(), 6400);
    }
}
