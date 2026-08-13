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
    pub faces: Vec<DetectedFace>,
}

/// One decoded YuNet face in the 640×640 detector input space.
#[derive(Debug, Clone)]
pub struct DetectedFace {
    pub x1: f32,
    pub y1: f32,
    pub w: f32,
    pub h: f32,
    /// 关键点顺序：右眼、左眼、鼻尖、右嘴角、左嘴角（被识者视角）。
    pub landmarks: [(f32, f32); 5],
    pub score: f32,
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
        let output_index: HashMap<String, usize> = session.outputs().iter().enumerate().map(|(index, output)| (output.name().to_string(), index)).collect();
        let outputs = session.run(ort::inputs![ort::value::TensorRef::from_array_view(&input)
            .map_err(|error| format!("构造检测输入失败：{error}"))?]).map_err(|error| format!("人脸检测推理失败：{error}"))?;
        let strides: [f32; 3] = [8.0, 16.0, 32.0];
        let mut decoded: [Option<(f32, [&[f32]; 4])>; 3] = [None, None, None];
        for ((slot, stride), grid) in decoded.iter_mut().zip(strides).zip([8usize, 16, 32]) {
            let names = [format!("cls_{grid}"), format!("obj_{grid}"), format!("bbox_{grid}"), format!("kps_{grid}")];
            let mut tensors: [&[f32]; 4] = [&[], &[], &[], &[]];
            let mut complete = true;
            for (name, tensor_slot) in names.iter().zip(tensors.iter_mut()) {
                let Some(&index) = output_index.get(name.as_str()) else { complete = false; break; };
                let Ok((_, tensor)) = outputs[index].try_extract_tensor::<f32>() else { complete = false; break; };
                *tensor_slot = tensor;
            }
            if !complete { decoded = [None, None, None]; break; }
            *slot = Some((stride, tensors));
        }
        if let [Some((s0, t0)), Some((s1, t1)), Some((s2, t2))] = decoded {
            let size = DETECTOR_SIZE as f32;
            let faces = decode_faces([(s0, t0[0], t0[1], t0[2], t0[3]), (s1, t1[0], t1[1], t1[2], t1[3]), (s2, t2[0], t2[1], t2[2], t2[3])], size, 0.60);
            let faces = nms_faces(faces, 0.35, 5);
            let best = faces.iter().map(|face| face.score).fold(0.0_f32, f32::max);
            if best < 0.60 { return Ok(None); }
            let detected_faces = faces.len().min(255) as u8;
            return Ok(Some(PresenceDetection { confidence: (best * 100.0).round().clamp(0.0, 100.0) as u8, detected_faces, faces }));
        }
        // Fallback for detectors without named stride outputs: keep presence-only behavior.
        let mut best = 0.0_f32;
        for (cls_index, object_index) in [(0usize, 3usize), (1, 4), (2, 5)] {
            let (_, cls) = outputs[cls_index].try_extract_tensor::<f32>().map_err(|error| format!("读取检测结果失败：{error}"))?;
            let (_, objectness) = outputs[object_index].try_extract_tensor::<f32>().map_err(|error| format!("读取检测结果失败：{error}"))?;
            for (&class_score, &object_score) in cls.iter().zip(objectness.iter()) {
                best = best.max(normalize_score(class_score) * normalize_score(object_score));
            }
        }
        if best < 0.60 { return Ok(None); }
        Ok(Some(PresenceDetection { confidence: (best * 100.0).round().clamp(0.0, 100.0) as u8, detected_faces: 1, faces: Vec::new() }))
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

/// YuNet 锚框解码：每个 stride 层按 grid cell 生成先验框，分数取 cls/obj 归一化后的几何均值。
fn decode_faces(strides: [(f32, &[f32], &[f32], &[f32], &[f32]); 3], size: f32, min_score: f32) -> Vec<DetectedFace> {
    let mut faces = Vec::new();
    for (stride, cls, obj, bbox, kps) in strides {
        let cols = (size / stride) as usize;
        let rows = cols;
        for r in 0..rows {
            for c in 0..cols {
                let idx = r * cols + c;
                if idx >= cls.len() || idx >= obj.len() || (idx + 1) * 4 > bbox.len() || (idx + 1) * 10 > kps.len() { continue; }
                let score = (normalize_score(cls[idx]) * normalize_score(obj[idx])).sqrt();
                if score < min_score { continue; }
                let cx = (c as f32 + bbox[idx * 4]) * stride;
                let cy = (r as f32 + bbox[idx * 4 + 1]) * stride;
                let w = bbox[idx * 4 + 2].exp() * stride;
                let h = bbox[idx * 4 + 3].exp() * stride;
                let mut landmarks = [(0.0_f32, 0.0_f32); 5];
                for (n, landmark) in landmarks.iter_mut().enumerate() {
                    *landmark = ((kps[idx * 10 + 2 * n] + c as f32) * stride, (kps[idx * 10 + 2 * n + 1] + r as f32) * stride);
                }
                faces.push(DetectedFace { x1: cx - w / 2.0, y1: cy - h / 2.0, w, h, landmarks, score });
            }
        }
    }
    faces
}

fn face_iou(a: &DetectedFace, b: &DetectedFace) -> f32 {
    let x1 = a.x1.max(b.x1);
    let y1 = a.y1.max(b.y1);
    let x2 = (a.x1 + a.w).min(b.x1 + b.w);
    let y2 = (a.y1 + a.h).min(b.y1 + b.h);
    let intersection = (x2 - x1).max(0.0) * (y2 - y1).max(0.0);
    let union = a.w * a.h + b.w * b.h - intersection;
    if union <= 0.0 { 0.0 } else { intersection / union }
}

/// 贪心 NMS：按分数降序保留，抑制与之 IoU 超阈值的锚框，至多保留 top_k 张脸。
fn nms_faces(mut faces: Vec<DetectedFace>, iou_threshold: f32, top_k: usize) -> Vec<DetectedFace> {
    faces.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    let mut kept = Vec::new();
    for face in faces {
        if kept.iter().any(|candidate: &DetectedFace| face_iou(candidate, &face) > iou_threshold) { continue; }
        kept.push(face);
        if kept.len() >= top_k { break; }
    }
    kept
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

    fn stride_layer<'a>(stride: f32, cls: &'a [f32], obj: &'a [f32], bbox: &'a [f32], kps: &'a [f32]) -> (f32, &'a [f32], &'a [f32], &'a [f32], &'a [f32]) {
        (stride, cls, obj, bbox, kps)
    }

    #[test]
    fn decode_faces_decodes_single_anchor_by_formula() {
        // 2×2 网格（size=16，stride=8），仅 idx=3 的锚框得分 0.9。
        let cls = [0.0, 0.0, 0.0, 0.81];
        let obj = [0.0, 0.0, 0.0, 1.0];
        let mut bbox = vec![0.0_f32; 16];
        bbox[12..16].copy_from_slice(&[0.5, 0.25, 0.0, 0.6931472]);
        let mut kps = vec![0.0_f32; 40];
        for n in 0..5 {
            kps[30 + 2 * n] = n as f32 * 0.1;
            kps[31 + 2 * n] = 0.5;
        }
        let layers = [stride_layer(8.0, &cls, &obj, &bbox, &kps), stride_layer(16.0, &[], &[], &[], &[]), stride_layer(32.0, &[], &[], &[], &[])];
        let faces = decode_faces(layers, 16.0, 0.6);
        assert_eq!(faces.len(), 1);
        let face = &faces[0];
        assert!((face.score - 0.9).abs() < 1e-6);
        // cx=(1+0.5)*8=12, cy=(1+0.25)*8=10, w=8, h=exp(0.6931472)*8≈16
        assert!((face.x1 - 8.0).abs() < 1e-3);
        assert!((face.y1 - 2.0).abs() < 1e-3);
        assert!((face.w - 8.0).abs() < 1e-3);
        assert!((face.h - 16.0).abs() < 1e-2);
        assert!((face.landmarks[0].0 - 8.0).abs() < 1e-3);
        assert!((face.landmarks[0].1 - 12.0).abs() < 1e-3);
        assert!((face.landmarks[4].0 - 11.2).abs() < 1e-3);
    }

    #[test]
    fn decode_faces_filters_low_score_anchors() {
        let cls = [0.25];
        let obj = [0.25];
        let bbox = [0.0, 0.0, 0.0, 0.0];
        let kps = [0.0; 10];
        let layers = [stride_layer(8.0, &cls, &obj, &bbox, &kps), stride_layer(16.0, &[], &[], &[], &[]), stride_layer(32.0, &[], &[], &[], &[])];
        assert!(decode_faces(layers, 8.0, 0.6).is_empty());
    }

    #[test]
    fn nms_faces_suppresses_overlapping_keeps_separated() {
        let face = |x1: f32, y1: f32, score: f32| DetectedFace { x1, y1, w: 10.0, h: 10.0, landmarks: [(0.0, 0.0); 5], score };
        let overlapping = nms_faces(vec![face(0.0, 0.0, 0.9), face(1.0, 1.0, 0.8)], 0.35, 5);
        assert_eq!(overlapping.len(), 1);
        assert!((overlapping[0].score - 0.9).abs() < 1e-6);
        let separated = nms_faces(vec![face(0.0, 0.0, 0.7), face(50.0, 50.0, 0.9)], 0.35, 5);
        assert_eq!(separated.len(), 2);
        assert!((separated[0].score - 0.9).abs() < 1e-6);
    }

    #[test]
    fn nms_faces_respects_top_k() {
        let face = |offset: f32, score: f32| DetectedFace { x1: offset, y1: offset, w: 10.0, h: 10.0, landmarks: [(0.0, 0.0); 5], score };
        let faces = (0..8).map(|i| face(i as f32 * 40.0, 0.5 + i as f32 * 0.01)).collect();
        assert_eq!(nms_faces(faces, 0.35, 5).len(), 5);
    }
}
