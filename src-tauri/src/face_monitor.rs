use image::imageops::FilterType;
use ndarray::Array4;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;

use crate::vision::profile::manifest_v4::{
    validate_manifest_v4, ComponentDescriptor as V4ComponentDescriptor, VisionManifestV4,
};
use crate::vision::profile::pipeline::resolve_pipeline;
use crate::vision::{
    alert::fuse_track_evidence,
    fusion::{FusionEvidence, FusionPolicy, TemporalIdentityFusion},
    tracking::{BoundingBox, Detection, TrackStore},
    types::{IdentityDecision, VisionModality},
};
use crate::vision::{backend::RuntimeBackend, openvino_runtime};

const DETECTOR_SIZE: u32 = 640;
const RECOGNIZER_SIZE: u32 = 112;
const PERSON_DETECTOR_SIZE: u32 = 640;
const PERSON_REID_WIDTH: u32 = 128;
const PERSON_REID_HEIGHT: u32 = 256;
pub const PERSON_REID_DIM: usize = 768;
const BODY_MATCH_MIN_MARGIN: f32 = 0.06;
const MAX_CONSECUTIVE_HIT_GAP_MS: i64 = 2_500;

/// arcface 112×112 对齐模板（左眼、右眼、鼻尖、左嘴角、右嘴角）。
const ALIGN_TEMPLATE: [(f32, f32); 5] = [
    (38.2946, 51.6963),
    (73.5318, 51.5014),
    (56.0252, 71.7366),
    (41.5493, 92.3655),
    (70.7299, 92.2041),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FaceMonitorLocalSettings {
    pub enabled: bool,
    pub face_recognition_enabled: bool,
    pub body_recognition_enabled: bool,
    pub device_id: Option<String>,
    pub pause_during_call: bool,
    pub sample_fps: u8,
    pub face_min_confidence: u8,
    pub body_min_confidence: u8,
    pub consecutive_hits: u8,
    pub face_cooldown_seconds: u32,
    pub body_cooldown_seconds: u32,
    pub applied_policy_version: i64,
}

impl Default for FaceMonitorLocalSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            face_recognition_enabled: true,
            body_recognition_enabled: true,
            device_id: None,
            pause_during_call: false,
            sample_fps: 2,
            face_min_confidence: 60,
            body_min_confidence: 68,
            consecutive_hits: 1,
            face_cooldown_seconds: 60,
            body_cooldown_seconds: 300,
            applied_policy_version: 0,
        }
    }
}

impl FaceMonitorLocalSettings {
    pub fn normalized(mut self) -> Self {
        self.device_id = self
            .device_id
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        self.sample_fps = self.sample_fps.clamp(1, 5);
        self.face_min_confidence = self.face_min_confidence.clamp(1, 100);
        self.body_min_confidence = self.body_min_confidence.clamp(1, 100);
        self.consecutive_hits = self.consecutive_hits.clamp(1, 20);
        self.face_cooldown_seconds = self.face_cooldown_seconds.clamp(5, 86_400);
        self.body_cooldown_seconds = self.body_cooldown_seconds.clamp(5, 86_400);
        self.applied_policy_version = self.applied_policy_version.max(0);
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
    pub person_detector_ready: bool,
    pub person_recognizer_ready: bool,
    pub queue_busy: bool,
    pub accepted_frames: u64,
    pub dropped_frames: u64,
    pub model_version: Option<String>,
    pub model_profile_id: Option<String>,
    pub face_embedding_space_id: Option<String>,
    pub body_embedding_space_id: Option<String>,
    pub face_embedding_dimension: usize,
    pub body_embedding_dimension: usize,
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

/// 已录入人员的本机特征模板，仅在监控设备内存中使用。
#[derive(Debug, Clone)]
pub struct PersonTemplate {
    pub person_id: String,
    pub display_name: String,
    pub face_embeddings: Vec<Vec<f32>>,
    pub body_embeddings: Vec<Vec<f32>>,
}

#[derive(Debug, Clone)]
pub struct FaceMatch {
    pub person_id: String,
    pub display_name: String,
    pub confidence: u8,
    pub recognition_level: String,
    pub face_confidence: Option<u8>,
    pub body_confidence: Option<u8>,
}

#[derive(Debug, Clone)]
pub struct FaceRecognitionFrame {
    pub matches: Vec<FaceMatch>,
}

#[derive(Debug, Default)]
struct TrackEvidence {
    face: Option<(FaceMatch, f32)>,
    body: Option<(FaceMatch, f32)>,
}

/// 录入人员参考图时的本地质量结论。它只包含质量分，不包含图像或特征。
#[derive(Debug, Clone, Copy)]
pub struct ReferencePhotoAnalysis {
    pub face_quality_score: Option<f32>,
    pub body_quality_score: Option<f32>,
}

/// 前端用于在多人参考照片中选择目标人员的归一化裁剪区域。
/// 坐标始终基于原始图片的 0.0..=1.0 空间，避免前端缩放预览时产生偏差。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferencePhotoCandidate {
    pub candidate_id: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub score: f32,
    pub uses_person_crop: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferencePhotoCandidateAnalysis {
    pub candidates: Vec<ReferencePhotoCandidate>,
    pub requires_selection: bool,
}

#[derive(Debug, Clone)]
struct DetectedPerson {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    score: f32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FaceModelManifest {
    schema_version: u8,
    model_version: String,
    detector: FaceModelAsset,
    #[serde(default)]
    recognizer: Option<FaceModelAsset>,
    #[serde(default)]
    person_detector: Option<FaceModelAsset>,
    #[serde(default)]
    person_recognizer: Option<FaceModelAsset>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FaceModelAsset {
    file: String,
    sha256: String,
}

#[derive(Debug, Clone)]
struct FaceModelState {
    ready: bool,
    version: Option<String>,
    profile_id: Option<String>,
    face_embedding_space_id: Option<String>,
    body_embedding_space_id: Option<String>,
    error: Option<String>,
    detector_path: Option<PathBuf>,
    recognizer_path: Option<PathBuf>,
    person_detector_path: Option<PathBuf>,
    person_recognizer_path: Option<PathBuf>,
    runtime_backend: RuntimeBackend,
    face_recognizer_spec: FaceRecognizerSpec,
    person_reid_spec: PersonReIdSpec,
}

impl Default for FaceModelState {
    fn default() -> Self {
        Self {
            ready: false,
            version: None,
            profile_id: None,
            face_embedding_space_id: None,
            body_embedding_space_id: None,
            error: None,
            detector_path: None,
            recognizer_path: None,
            person_detector_path: None,
            person_recognizer_path: None,
            runtime_backend: RuntimeBackend::OnnxRuntime,
            face_recognizer_spec: FaceRecognizerSpec::default(),
            person_reid_spec: PersonReIdSpec::default(),
        }
    }
}

enum InferenceSession {
    Onnx(ort::session::Session),
    #[cfg(feature = "openvino-runtime")]
    OpenVino(openvino_runtime::OpenVinoSession),
}

fn load_inference_session(
    path: &Path,
    backend: RuntimeBackend,
) -> Result<InferenceSession, String> {
    match backend {
        RuntimeBackend::OnnxRuntime => ort::session::Session::builder()
            .and_then(|mut builder| builder.commit_from_file(path))
            .map(InferenceSession::Onnx)
            .map_err(|error| format!("ONNX 会话加载失败：{error}")),
        RuntimeBackend::OpenVino => {
            #[cfg(feature = "openvino-runtime")]
            {
                openvino_runtime::OpenVinoSession::load_cpu(path).map(InferenceSession::OpenVino)
            }
            #[cfg(not(feature = "openvino-runtime"))]
            {
                let _ = path;
                Err("VISION_OPENVINO_RUNTIME_REQUIRED".to_string())
            }
        }
    }
}

#[derive(Debug, Clone)]
struct FaceRecognizerSpec {
    width: u32,
    height: u32,
    output_dimension: usize,
    color_order: String,
    normalization: String,
}

impl Default for FaceRecognizerSpec {
    fn default() -> Self {
        Self {
            width: RECOGNIZER_SIZE,
            height: RECOGNIZER_SIZE,
            output_dimension: 128,
            color_order: "BGR".to_string(),
            normalization: "sface_127_5".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
struct PersonReIdSpec {
    width: u32,
    height: u32,
    output_dimension: usize,
    color_order: String,
    normalization: String,
}

impl Default for PersonReIdSpec {
    fn default() -> Self {
        Self {
            width: PERSON_REID_WIDTH,
            height: PERSON_REID_HEIGHT,
            output_dimension: PERSON_REID_DIM,
            color_order: "RGB".to_string(),
            normalization: "imagenet".to_string(),
        }
    }
}

#[derive(Debug, Clone, Default)]
struct HitGateState {
    consecutive_hits: u8,
    last_alert_at: i64,
    last_hit_at: i64,
}

/// Local camera face-presence detector. It only answers whether a face is present
/// in the current frame; it neither identifies people nor stores frames/photos.
pub struct FaceMonitorRuntime {
    settings: Mutex<FaceMonitorLocalSettings>,
    busy: AtomicBool,
    accepted_frames: AtomicU64,
    dropped_frames: AtomicU64,
    recognition_frame_id: AtomicU64,
    hit_state: Mutex<HashMap<String, HitGateState>>,
    track_store: Mutex<TrackStore>,
    temporal_fusion: Mutex<TemporalIdentityFusion>,
    model_state: FaceModelState,
    detector: Option<Mutex<InferenceSession>>,
    recognizer: Option<Mutex<InferenceSession>>,
    person_detector: Option<Mutex<InferenceSession>>,
    person_recognizer: Option<Mutex<InferenceSession>>,
    last_detection: Mutex<Option<PresenceDetection>>,
    runtime_error: Mutex<Option<String>>,
}

impl Default for FaceMonitorRuntime {
    fn default() -> Self {
        Self {
            settings: Mutex::new(FaceMonitorLocalSettings::default()),
            busy: AtomicBool::new(false),
            accepted_frames: AtomicU64::new(0),
            dropped_frames: AtomicU64::new(0),
            recognition_frame_id: AtomicU64::new(0),
            hit_state: Mutex::new(HashMap::new()),
            track_store: Mutex::new(TrackStore::new(2_500)),
            temporal_fusion: Mutex::new(TemporalIdentityFusion::new(FusionPolicy::default())),
            detector: None,
            recognizer: None,
            person_detector: None,
            person_recognizer: None,
            last_detection: Mutex::new(None),
            runtime_error: Mutex::new(None),
            model_state: FaceModelState {
                runtime_backend: RuntimeBackend::OnnxRuntime,
                error: Some("人脸检测模型尚未安装".to_string()),
                ..Default::default()
            },
        }
    }
}

impl FaceMonitorRuntime {
    pub fn from_resource_dirs(resource_dir: Option<PathBuf>) -> Self {
        Self::from_candidate_dirs(resource_dir.into_iter().collect())
    }

    /// 候选目录按顺序尝试；受控下载模型失败时自然回退内置资源。
    pub fn from_candidate_dirs(resource_dirs: Vec<PathBuf>) -> Self {
        let mut candidates = resource_dirs
            .into_iter()
            .flat_map(|path| {
                vec![
                    path.join("object-models"),
                    path.join("resources").join("object-models"),
                ]
            })
            .collect::<Vec<_>>();
        candidates.push(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("resources")
                .join("object-models"),
        );

        let mut reasons = Vec::new();
        let mut model_state = None;
        for path in &candidates {
            match model_state_from_dir(path) {
                Ok(state) => {
                    model_state = Some(state);
                    break;
                }
                Err(error) => reasons.push(error),
            }
        }
        let mut runtime = Self {
            model_state: model_state.unwrap_or_else(|| FaceModelState {
                error: reasons
                    .into_iter()
                    .next()
                    .or_else(|| Some("未找到人脸检测模型资源目录".to_string())),
                ..Default::default()
            }),
            ..Self::default()
        };
        if let Some(path) = runtime.model_state.detector_path.clone() {
            match load_inference_session(&path, runtime.model_state.runtime_backend) {
                Ok(session) => runtime.detector = Some(Mutex::new(session)),
                Err(error) => {
                    runtime.model_state.ready = false;
                    runtime.model_state.error = Some(format!("人脸检测模型加载失败：{error}"));
                }
            }
        }
        // 识别模型加载失败不影响存在检测，仅使识别能力不可用。
        if let Some(path) = runtime.model_state.recognizer_path.clone() {
            match load_inference_session(&path, runtime.model_state.runtime_backend) {
                Ok(session) => runtime.recognizer = Some(Mutex::new(session)),
                Err(error) => {
                    runtime.recognizer = None;
                    if runtime.model_state.error.is_none() {
                        runtime.model_state.error = Some(format!("人脸识别模型加载失败：{error}"));
                    }
                }
            }
        }
        if let Some(path) = runtime.model_state.person_detector_path.clone() {
            match load_inference_session(&path, runtime.model_state.runtime_backend) {
                Ok(session) => runtime.person_detector = Some(Mutex::new(session)),
                Err(error) => {
                    runtime.model_state.error = Some(format!("人体检测模型加载失败：{error}"))
                }
            }
        }
        if let Some(path) = runtime.model_state.person_recognizer_path.clone() {
            match load_inference_session(&path, runtime.model_state.runtime_backend) {
                Ok(session) => runtime.person_recognizer = Some(Mutex::new(session)),
                Err(error) => {
                    runtime.model_state.error = Some(format!("人体识别模型加载失败：{error}"))
                }
            }
        }
        runtime
    }

    /// Profile 激活前构建一次短生命周期候选会话。这里不替换正在服务的
    /// Runtime，只验证清单、摘要、适配器组合和 ONNX Session 是否都可打开。
    pub fn validate_candidate_model_dir(model_dir: &Path) -> Result<(), String> {
        let state = model_state_from_dir(model_dir)?;
        if !state.ready {
            return Err("VISION_CANDIDATE_MODEL_NOT_READY".to_string());
        }
        let validate_session = |label: &str, path: Option<&PathBuf>, required: bool| {
            let Some(path) = path else {
                return if required {
                    Err(format!("VISION_CANDIDATE_COMPONENT_MISSING:{label}"))
                } else {
                    Ok(())
                };
            };
            load_inference_session(path, state.runtime_backend)
                .map(|_| ())
                .map_err(|error| format!("VISION_CANDIDATE_SESSION_INVALID:{label}:{error}"))
        };
        let v4_profile = state.profile_id.as_deref().is_some_and(|id| id != "legacy");
        validate_session("face-detector", state.detector_path.as_ref(), true)?;
        validate_session(
            "face-recognizer",
            state.recognizer_path.as_ref(),
            v4_profile,
        )?;
        validate_session(
            "person-detector",
            state.person_detector_path.as_ref(),
            v4_profile,
        )?;
        validate_session(
            "person-reid",
            state.person_recognizer_path.as_ref(),
            v4_profile,
        )?;
        Ok(())
    }

    pub fn settings(&self) -> FaceMonitorLocalSettings {
        self.settings
            .lock()
            .map(|value| value.clone())
            .unwrap_or_default()
    }

    pub fn person_reid_embedding_dimension(&self) -> usize {
        self.model_state.person_reid_spec.output_dimension
    }

    pub fn face_embedding_dimension(&self) -> usize {
        self.model_state.face_recognizer_spec.output_dimension
    }

    /// 特征缓存必须按模型空间而非展示版号复用。不同模型恰好都叫 1.0.0 时，
    /// 也不能把 SFace/Youtu 或 OSNet 的向量混在一起。
    pub fn face_embedding_space_id(&self) -> String {
        self.model_state
            .face_embedding_space_id
            .clone()
            .unwrap_or_else(|| "legacy.face.unknown".to_string())
    }

    pub fn body_embedding_space_id(&self) -> String {
        self.model_state
            .body_embedding_space_id
            .clone()
            .unwrap_or_else(|| "legacy.body.unknown".to_string())
    }

    pub fn update_settings(&self, settings: FaceMonitorLocalSettings) -> FaceMonitorLocalSettings {
        let settings = settings.normalized();
        if let Ok(mut current) = self.settings.lock() {
            *current = settings.clone();
        }
        settings
    }

    pub fn status(&self) -> FaceMonitorStatus {
        let last_detection = self
            .last_detection
            .lock()
            .ok()
            .and_then(|value| value.clone());
        let runtime_error = self
            .runtime_error
            .lock()
            .ok()
            .and_then(|value| value.clone());
        FaceMonitorStatus {
            supported: cfg!(target_os = "windows"),
            enabled: self.settings().enabled,
            model_assets_ready: cfg!(target_os = "windows") && self.model_state.ready,
            model_ready: cfg!(target_os = "windows")
                && ((self.detector.is_some() && self.recognizer.is_some())
                    || (self.person_detector.is_some() && self.person_recognizer.is_some())),
            recognizer_ready: cfg!(target_os = "windows") && self.recognizer.is_some(),
            person_detector_ready: cfg!(target_os = "windows") && self.person_detector.is_some(),
            person_recognizer_ready: cfg!(target_os = "windows")
                && self.person_recognizer.is_some(),
            queue_busy: self.busy.load(Ordering::Relaxed),
            accepted_frames: self.accepted_frames.load(Ordering::Relaxed),
            dropped_frames: self.dropped_frames.load(Ordering::Relaxed),
            model_version: self.model_state.version.clone(),
            model_profile_id: self.model_state.profile_id.clone(),
            face_embedding_space_id: self.model_state.face_embedding_space_id.clone(),
            body_embedding_space_id: self.model_state.body_embedding_space_id.clone(),
            face_embedding_dimension: self.model_state.face_recognizer_spec.output_dimension,
            body_embedding_dimension: self.model_state.person_reid_spec.output_dimension,
            last_detection_score: last_detection.as_ref().map(|value| value.confidence),
            detected_faces: last_detection
                .map(|value| value.detected_faces)
                .unwrap_or(0),
            last_error: if cfg!(target_os = "windows") {
                runtime_error.or_else(|| self.model_state.error.clone())
            } else {
                Some("摄像头人脸出现检测第一期仅支持 Windows".to_string())
            },
        }
    }

    fn detect_in_rgb(&self, image: &image::RgbImage) -> Result<Option<PresenceDetection>, String> {
        if self.model_state.runtime_backend == RuntimeBackend::OpenVino {
            return self.detect_openvino_faces(image);
        }
        let resized =
            image::imageops::resize(image, DETECTOR_SIZE, DETECTOR_SIZE, FilterType::Triangle);
        let mut input =
            Array4::<f32>::zeros((1, 3, DETECTOR_SIZE as usize, DETECTOR_SIZE as usize));
        for y in 0..DETECTOR_SIZE as usize {
            for x in 0..DETECTOR_SIZE as usize {
                let pixel = resized.get_pixel(x as u32, y as u32);
                // YuNet follows OpenCV DNN BGR input order and uses 0..255 values.
                input[[0, 0, y, x]] = f32::from(pixel[2]);
                input[[0, 1, y, x]] = f32::from(pixel[1]);
                input[[0, 2, y, x]] = f32::from(pixel[0]);
            }
        }
        let detector = self
            .detector
            .as_ref()
            .ok_or_else(|| "人脸检测模型未就绪".to_string())?;
        let mut session = detector
            .lock()
            .map_err(|_| "人脸检测器被占用".to_string())?;
        #[cfg(feature = "openvino-runtime")]
        let InferenceSession::Onnx(session) = &mut *session
        else {
            return Err("人脸检测模型后端不匹配".to_string());
        };
        #[cfg(not(feature = "openvino-runtime"))]
        let InferenceSession::Onnx(session) = &mut *session;
        let output_index: HashMap<String, usize> = session
            .outputs()
            .iter()
            .enumerate()
            .map(|(index, output)| (output.name().to_string(), index))
            .collect();
        let outputs = session
            .run(ort::inputs![ort::value::TensorRef::from_array_view(&input)
                .map_err(|error| format!("构造检测输入失败：{error}"))?])
            .map_err(|error| format!("人脸检测推理失败：{error}"))?;
        let strides: [f32; 3] = [8.0, 16.0, 32.0];
        let mut decoded: [Option<(f32, [&[f32]; 4])>; 3] = [None, None, None];
        for ((slot, stride), grid) in decoded.iter_mut().zip(strides).zip([8usize, 16, 32]) {
            let names = [
                format!("cls_{grid}"),
                format!("obj_{grid}"),
                format!("bbox_{grid}"),
                format!("kps_{grid}"),
            ];
            let mut tensors: [&[f32]; 4] = [&[], &[], &[], &[]];
            let mut complete = true;
            for (name, tensor_slot) in names.iter().zip(tensors.iter_mut()) {
                let Some(&index) = output_index.get(name.as_str()) else {
                    complete = false;
                    break;
                };
                let Ok((_, tensor)) = outputs[index].try_extract_tensor::<f32>() else {
                    complete = false;
                    break;
                };
                *tensor_slot = tensor;
            }
            if !complete {
                decoded = [None, None, None];
                break;
            }
            *slot = Some((stride, tensors));
        }
        if let [Some((s0, t0)), Some((s1, t1)), Some((s2, t2))] = decoded {
            let size = DETECTOR_SIZE as f32;
            let faces = decode_faces(
                [
                    (s0, t0[0], t0[1], t0[2], t0[3]),
                    (s1, t1[0], t1[1], t1[2], t1[3]),
                    (s2, t2[0], t2[1], t2[2], t2[3]),
                ],
                size,
                0.60,
            );
            let faces = nms_faces(faces, 0.35, 5);
            let best = faces.iter().map(|face| face.score).fold(0.0_f32, f32::max);
            if best < 0.60 {
                return Ok(None);
            }
            let detected_faces = faces.len().min(255) as u8;
            return Ok(Some(PresenceDetection {
                confidence: (best * 100.0).round().clamp(0.0, 100.0) as u8,
                detected_faces,
                faces,
            }));
        }
        // Fallback for detectors without named stride outputs: keep presence-only behavior.
        let mut best = 0.0_f32;
        for (cls_index, object_index) in [(0usize, 3usize), (1, 4), (2, 5)] {
            let (_, cls) = outputs[cls_index]
                .try_extract_tensor::<f32>()
                .map_err(|error| format!("读取检测结果失败：{error}"))?;
            let (_, objectness) = outputs[object_index]
                .try_extract_tensor::<f32>()
                .map_err(|error| format!("读取检测结果失败：{error}"))?;
            for (&class_score, &object_score) in cls.iter().zip(objectness.iter()) {
                best = best.max(normalize_score(class_score) * normalize_score(object_score));
            }
        }
        if best < 0.60 {
            return Ok(None);
        }
        Ok(Some(PresenceDetection {
            confidence: (best * 100.0).round().clamp(0.0, 100.0) as u8,
            detected_faces: 1,
            faces: Vec::new(),
        }))
    }

    #[cfg(feature = "openvino-runtime")]
    fn detect_openvino_faces(
        &self,
        image: &image::RgbImage,
    ) -> Result<Option<PresenceDetection>, String> {
        const OMZ_FACE_DETECTOR_SIZE: u32 = 300;
        let resized = image::imageops::resize(
            image,
            OMZ_FACE_DETECTOR_SIZE,
            OMZ_FACE_DETECTOR_SIZE,
            FilterType::Triangle,
        );
        let input = openvino_bgr_input(&resized, OMZ_FACE_DETECTOR_SIZE, OMZ_FACE_DETECTOR_SIZE);
        let detector = self
            .detector
            .as_ref()
            .ok_or_else(|| "人脸检测模型未就绪".to_string())?;
        let mut session = detector
            .lock()
            .map_err(|_| "人脸检测器被占用".to_string())?;
        let InferenceSession::OpenVino(session) = &mut *session else {
            return Err("人脸检测模型后端不匹配".to_string());
        };
        let output = session.infer_f32(
            &[
                1,
                3,
                i64::from(OMZ_FACE_DETECTOR_SIZE),
                i64::from(OMZ_FACE_DETECTOR_SIZE),
            ],
            &input,
        )?;
        let faces =
            decode_openvino_faces(&output, DETECTOR_SIZE as f32, DETECTOR_SIZE as f32, 0.60);
        let best = faces.iter().map(|face| face.score).fold(0.0_f32, f32::max);
        if best < 0.60 {
            return Ok(None);
        }
        Ok(Some(PresenceDetection {
            confidence: (best * 100.0).round().clamp(0.0, 100.0) as u8,
            detected_faces: faces.len().min(255) as u8,
            faces: nms_faces(faces, 0.35, 5),
        }))
    }

    #[cfg(not(feature = "openvino-runtime"))]
    fn detect_openvino_faces(
        &self,
        _image: &image::RgbImage,
    ) -> Result<Option<PresenceDetection>, String> {
        Err("VISION_OPENVINO_RUNTIME_REQUIRED".to_string())
    }

    /// 识别模式入口：检测→逐脸提取特征→与人员模板比对。
    /// 单脸特征提取失败只跳过该脸，不导致整帧失败。
    pub fn recognize_frame(
        &self,
        bytes: &[u8],
        people: &[PersonTemplate],
    ) -> Result<Option<FaceRecognitionFrame>, String> {
        let settings = self.settings();
        let face_ready = settings.face_recognition_enabled
            && self.detector.is_some()
            && self.recognizer.is_some();
        let body_ready = settings.body_recognition_enabled
            && self.person_detector.is_some()
            && self.person_recognizer.is_some();
        if !settings.enabled || (!face_ready && !body_ready) || bytes.is_empty() {
            return Ok(None);
        }
        if self.busy.swap(true, Ordering::AcqRel) {
            self.dropped_frames.fetch_add(1, Ordering::Relaxed);
            return Ok(None);
        }
        self.accepted_frames.fetch_add(1, Ordering::Relaxed);
        let result = self.recognize_bytes(bytes, people, &settings);
        self.busy.store(false, Ordering::Release);
        match result {
            Ok(frame) => {
                if let Ok(mut error) = self.runtime_error.lock() {
                    *error = None;
                }
                Ok(frame)
            }
            Err(error) => {
                if let Ok(mut last_error) = self.runtime_error.lock() {
                    *last_error = Some(error.clone());
                }
                Err(error)
            }
        }
    }

    fn recognize_bytes(
        &self,
        bytes: &[u8],
        people: &[PersonTemplate],
        settings: &FaceMonitorLocalSettings,
    ) -> Result<Option<FaceRecognitionFrame>, String> {
        let image = image::load_from_memory(bytes)
            .map_err(|error| format!("摄像头采样帧无法解码：{error}"))?
            .to_rgb8();
        let detection = if settings.face_recognition_enabled
            && self.detector.is_some()
            && self.recognizer.is_some()
        {
            self.detect_in_rgb(&image)?
        } else {
            None
        };
        if let Some(value) = detection.as_ref() {
            if let Ok(mut last) = self.last_detection.lock() {
                *last = Some(value.clone());
            }
        }
        let frame_id = self.recognition_frame_id.fetch_add(1, Ordering::Relaxed) + 1;
        let observed_at = chrono::Utc::now().timestamp_millis();
        let mut tracked = BTreeMap::<String, TrackEvidence>::new();
        if let Some(detection) = detection.filter(|value| !value.faces.is_empty()) {
            let scale_x = image.width() as f32 / DETECTOR_SIZE as f32;
            let scale_y = image.height() as f32 / DETECTOR_SIZE as f32;
            for face in &detection.faces {
                let landmarks: [(f32, f32); 5] =
                    face.landmarks.map(|(x, y)| (x * scale_x, y * scale_y));
                let Ok(embedding) = self.extract_embedding(&image, landmarks) else {
                    continue;
                };
                let Some(matched) = best_face_match(&embedding, people) else {
                    continue;
                };
                let track_id = self.observe_track(
                    VisionModality::Face,
                    BoundingBox::new(
                        (face.x1 * scale_x / image.width() as f32).clamp(0.0, 1.0),
                        (face.y1 * scale_y / image.height() as f32).clamp(0.0, 1.0),
                        (face.w * scale_x / image.width() as f32).clamp(0.0, 1.0),
                        (face.h * scale_y / image.height() as f32).clamp(0.0, 1.0),
                    ),
                    observed_at,
                );
                let entry = tracked.entry(track_id).or_default();
                if entry
                    .face
                    .as_ref()
                    .is_none_or(|(current, _)| current.confidence < matched.confidence)
                {
                    entry.face = Some((matched, face.score));
                }
            }
        }
        if settings.body_recognition_enabled
            && self.person_detector.is_some()
            && self.person_recognizer.is_some()
        {
            for person in self.detect_people(&image)?.into_iter().take(5) {
                let crop = crop_person(&image, &person);
                let Ok(embedding) = self.extract_body_embedding(&crop) else {
                    continue;
                };
                let Some(matched) = best_body_match(&embedding, people) else {
                    continue;
                };
                let track_id = self.observe_track(
                    VisionModality::Body,
                    BoundingBox::new(
                        (person.x / image.width() as f32).clamp(0.0, 1.0),
                        (person.y / image.height() as f32).clamp(0.0, 1.0),
                        (person.w / image.width() as f32).clamp(0.0, 1.0),
                        (person.h / image.height() as f32).clamp(0.0, 1.0),
                    ),
                    observed_at,
                );
                let entry = tracked.entry(track_id).or_default();
                if entry
                    .body
                    .as_ref()
                    .is_none_or(|(current, _)| current.confidence < matched.confidence)
                {
                    entry.body = Some((matched, person.score));
                }
            }
        }
        let mut matches = tracked
            .into_iter()
            .filter_map(|(track_id, evidence)| {
                self.fuse_track_matches(&track_id, frame_id, evidence)
            })
            .collect::<Vec<_>>();
        matches.sort_by(|left, right| right.confidence.cmp(&left.confidence));
        matches.dedup_by(|left, right| {
            left.person_id == right.person_id && left.recognition_level == right.recognition_level
        });
        if matches.is_empty() {
            Ok(None)
        } else {
            Ok(Some(FaceRecognitionFrame { matches }))
        }
    }

    fn observe_track(
        &self,
        modality: VisionModality,
        bounds: BoundingBox,
        observed_at: i64,
    ) -> String {
        self.track_store
            .lock()
            .map(|mut tracks| tracks.observe(Detection::new(modality, bounds), observed_at))
            .unwrap_or_else(|_| "track-unavailable".to_string())
    }

    /// 将同一短时轨迹中的人脸/人体结果收敛为一个候选。时间融合只负责提高
    /// 决策可信度；既有的连续命中、冷却和告警策略仍由 `accept_match` 统一管理。
    fn fuse_track_matches(
        &self,
        track_id: &str,
        frame_id: u64,
        evidence: TrackEvidence,
    ) -> Option<FaceMatch> {
        let face = evidence.face;
        let body = evidence.body;
        let face_evidence = face.as_ref().map(|(matched, quality)| {
            FusionEvidence::face(&matched.person_id, f32::from(matched.confidence), *quality)
        });
        let body_evidence = body.as_ref().map(|(matched, quality)| {
            FusionEvidence::body(&matched.person_id, f32::from(matched.confidence), *quality)
        });
        let temporal = self.temporal_fusion.lock().ok().and_then(|mut fusion| {
            fusion.observe(track_id, frame_id, face_evidence, body_evidence)
        });
        let immediate = fuse_track_evidence(
            face.as_ref()
                .map(|(matched, _)| (matched.person_id.as_str(), matched.confidence)),
            body.as_ref()
                .map(|(matched, _)| (matched.person_id.as_str(), matched.confidence)),
        );
        let decision = temporal
            .as_ref()
            .map(|value| value.decision)
            .unwrap_or(immediate.decision);
        let person_id = temporal
            .and_then(|value| value.person_id)
            .or(immediate.person_id)?;
        let selected = match decision {
            IdentityDecision::ConfirmedFusion | IdentityDecision::ConfirmedFace => face
                .as_ref()
                .filter(|(matched, _)| matched.person_id == person_id)
                .map(|(matched, _)| matched.clone())
                .or_else(|| body.as_ref().map(|(matched, _)| matched.clone())),
            IdentityDecision::ProbableBody => body
                .as_ref()
                .filter(|(matched, _)| matched.person_id == person_id)
                .map(|(matched, _)| matched.clone()),
            IdentityDecision::Unknown => None,
        }?;
        Some(FaceMatch {
            recognition_level: if matches!(decision, IdentityDecision::ProbableBody) {
                "suspected".to_string()
            } else {
                "confirmed".to_string()
            },
            face_confidence: face
                .as_ref()
                .filter(|(matched, _)| matched.person_id == person_id)
                .map(|(matched, _)| matched.confidence),
            body_confidence: body
                .as_ref()
                .filter(|(matched, _)| matched.person_id == person_id)
                .map(|(matched, _)| matched.confidence),
            ..selected
        })
    }

    pub fn accept_match(
        &self,
        key: &str,
        confidence: u8,
        min_confidence: u8,
        required_hits: u8,
        cooldown_seconds: u32,
        now: i64,
    ) -> bool {
        if confidence < min_confidence.min(100) || key.trim().is_empty() {
            return false;
        }
        let Ok(mut states) = self.hit_state.lock() else {
            return false;
        };
        let state = states.entry(key.to_string()).or_default();
        if state.last_hit_at > 0
            && now.saturating_sub(state.last_hit_at) > MAX_CONSECUTIVE_HIT_GAP_MS
        {
            state.consecutive_hits = 0;
        }
        state.last_hit_at = now;
        state.consecutive_hits = state.consecutive_hits.saturating_add(1);
        if state.consecutive_hits < required_hits.clamp(1, 20) {
            return false;
        }
        if state.last_alert_at > 0
            && now.saturating_sub(state.last_alert_at)
                < i64::from(cooldown_seconds.clamp(5, 86_400)) * 1000
        {
            return false;
        }
        state.last_alert_at = now;
        state.consecutive_hits = 0;
        true
    }

    pub fn prepare_match_frame(&self, candidate_keys: &[String]) {
        let Ok(mut states) = self.hit_state.lock() else {
            return;
        };
        for (key, state) in states.iter_mut() {
            if key.starts_with("camera-person:") && !candidate_keys.iter().any(|item| item == key) {
                state.consecutive_hits = 0;
                state.last_hit_at = 0;
            }
        }
        if candidate_keys.is_empty() {
            if let Ok(mut tracks) = self.track_store.lock() {
                tracks.reset();
            }
            if let Ok(mut fusion) = self.temporal_fusion.lock() {
                fusion.reset();
            }
        }
    }

    /// 对已对齐到原图坐标空间的关键点提取当前人脸引擎的特征（L2 归一化）。
    pub fn extract_embedding(
        &self,
        image: &image::RgbImage,
        landmarks: [(f32, f32); 5],
    ) -> Result<Vec<f32>, String> {
        if self.model_state.runtime_backend == RuntimeBackend::OpenVino {
            return self.extract_openvino_face_embedding(image, landmarks);
        }
        let recognizer = self
            .recognizer
            .as_ref()
            .ok_or_else(|| "识别模型未就绪".to_string())?;
        let spec = &self.model_state.face_recognizer_spec;
        let aligned = align_face_112(image, landmarks);
        let aligned = if aligned.width() == spec.width && aligned.height() == spec.height {
            aligned
        } else {
            image::imageops::resize(&aligned, spec.width, spec.height, FilterType::Triangle)
        };
        let mut input = Array4::<f32>::zeros((1, 3, spec.height as usize, spec.width as usize));
        for y in 0..spec.height as usize {
            for x in 0..spec.width as usize {
                let pixel = aligned.get_pixel(x as u32, y as u32);
                let rgb = [
                    f32::from(pixel[0]),
                    f32::from(pixel[1]),
                    f32::from(pixel[2]),
                ];
                let channels = match spec.color_order.as_str() {
                    "RGB" => rgb,
                    "BGR" => [rgb[2], rgb[1], rgb[0]],
                    _ => return Err("人脸识别模型颜色顺序不受支持".to_string()),
                };
                for channel in 0..3 {
                    input[[0, channel, y, x]] = match spec.normalization.as_str() {
                        // 保留现有 SFace 路径，避免内置模型的预处理行为变化。
                        "sface_127_5" => channels[channel],
                        "arcface_127_5" => channels[channel] / 127.5 - 1.0,
                        _ => return Err("人脸识别模型预处理方式不受支持".to_string()),
                    };
                }
            }
        }
        let mut session = recognizer
            .lock()
            .map_err(|_| "人脸识别器被占用".to_string())?;
        #[cfg(feature = "openvino-runtime")]
        let InferenceSession::Onnx(session) = &mut *session
        else {
            return Err("人脸识别模型后端不匹配".to_string());
        };
        #[cfg(not(feature = "openvino-runtime"))]
        let InferenceSession::Onnx(session) = &mut *session;
        let outputs = session
            .run(ort::inputs![ort::value::TensorRef::from_array_view(&input)
                .map_err(|error| format!("构造识别输入失败：{error}"))?])
            .map_err(|error| format!("人脸识别推理失败：{error}"))?;
        let (_, tensor) = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|error| format!("读取识别结果失败：{error}"))?;
        if tensor.len() != spec.output_dimension {
            return Err(format!("识别模型输出维数异常：{}", tensor.len()));
        }
        let mut embedding = tensor.to_vec();
        normalize_embedding(&mut embedding);
        Ok(embedding)
    }

    #[cfg(feature = "openvino-runtime")]
    fn extract_openvino_face_embedding(
        &self,
        image: &image::RgbImage,
        landmarks: [(f32, f32); 5],
    ) -> Result<Vec<f32>, String> {
        let spec = &self.model_state.face_recognizer_spec;
        let aligned = align_face_112(image, landmarks);
        let aligned =
            image::imageops::resize(&aligned, spec.width, spec.height, FilterType::Triangle);
        let input = openvino_bgr_input(&aligned, spec.width, spec.height);
        let recognizer = self
            .recognizer
            .as_ref()
            .ok_or_else(|| "人脸识别模型未就绪".to_string())?;
        let mut session = recognizer
            .lock()
            .map_err(|_| "人脸识别器被占用".to_string())?;
        let InferenceSession::OpenVino(session) = &mut *session else {
            return Err("人脸识别模型后端不匹配".to_string());
        };
        let mut embedding = session.infer_f32(
            &[1, 3, i64::from(spec.height), i64::from(spec.width)],
            &input,
        )?;
        if embedding.len() != spec.output_dimension {
            return Err(format!("识别模型输出维数异常：{}", embedding.len()));
        }
        normalize_embedding(&mut embedding);
        Ok(embedding)
    }

    #[cfg(not(feature = "openvino-runtime"))]
    fn extract_openvino_face_embedding(
        &self,
        _image: &image::RgbImage,
        _landmarks: [(f32, f32); 5],
    ) -> Result<Vec<f32>, String> {
        Err("VISION_OPENVINO_RUNTIME_REQUIRED".to_string())
    }

    /// 人员参考照片入口：解码图片→检测人脸→取最高分脸→提取特征。
    pub fn embedding_from_photo_bytes(&self, bytes: &[u8]) -> Result<Vec<f32>, String> {
        if self.detector.is_none() {
            return Err("人脸检测模型未就绪".to_string());
        }
        let photo = image::load_from_memory(bytes)
            .map_err(|error| format!("参考照片无法解码：{error}"))?
            .to_rgb8();
        let (origin_width, origin_height) = (photo.width() as f32, photo.height() as f32);
        let detection = self
            .detect_in_rgb(&photo)?
            .filter(|value| !value.faces.is_empty())
            .ok_or_else(|| "参考照片中未检测到人脸".to_string())?;
        let face = detection
            .faces
            .iter()
            .max_by(|a, b| {
                a.score
                    .partial_cmp(&b.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .ok_or_else(|| "参考照片中未检测到人脸".to_string())?;
        // 检测在 640×640 拉伸空间进行，关键点按各轴比例换算回原图坐标。
        let scale_x = origin_width / DETECTOR_SIZE as f32;
        let scale_y = origin_height / DETECTOR_SIZE as f32;
        let landmarks: [(f32, f32); 5] = face.landmarks.map(|(x, y)| (x * scale_x, y * scale_y));
        self.extract_embedding(&photo, landmarks)
    }

    /// 在保存原图前做一次本地参考图检查。人脸和人体检测器都可用时，取两者
    /// 中更大的主体数，避免把多人合照意外编码为某一个人的模板。
    pub fn analyze_reference_photo(&self, bytes: &[u8]) -> Result<ReferencePhotoAnalysis, String> {
        let photo = image::load_from_memory(bytes)
            .map_err(|error| format!("参考照片无法解码：{error}"))?
            .to_rgb8();
        let mut face_quality_score = None;
        let mut body_quality_score = None;

        if self.detector.is_some() {
            if let Some(detection) = self.detect_in_rgb(&photo)? {
                face_quality_score = Some(
                    detection
                        .faces
                        .iter()
                        .map(|face| face.score)
                        .fold(f32::from(detection.confidence) / 100.0, f32::max),
                );
            }
        }
        if self.person_detector.is_some() {
            let people = self.detect_people(&photo)?;
            body_quality_score = people
                .iter()
                .map(|person| person.score)
                .max_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
        }

        Ok(ReferencePhotoAnalysis {
            face_quality_score,
            body_quality_score,
        })
    }

    /// 参考图候选项以可用人脸为锚点，并优先扩展为包含该人脸的人体检测框。
    /// 因此多人合照只会在用户选择后保存目标人物的局部图片，不会把整张合照入库。
    pub fn analyze_reference_photo_candidates(
        &self,
        bytes: &[u8],
    ) -> Result<ReferencePhotoCandidateAnalysis, String> {
        let photo = image::load_from_memory(bytes)
            .map_err(|error| format!("参考照片无法解码：{error}"))?
            .to_rgb8();
        let candidates = self.reference_photo_candidates_from_rgb(&photo)?;
        if candidates.is_empty() {
            return Err("参考照片中未检测到可用人脸，请选择人脸清晰的照片".to_string());
        }
        Ok(ReferencePhotoCandidateAnalysis {
            requires_selection: candidates.len() > 1,
            candidates,
        })
    }

    /// 将选中的候选人裁剪并编码为 JPEG。调用方只需持久化返回字节，后续特征提取
    /// 会自然只看到被选中的单人样本。
    pub fn crop_reference_photo_candidate(
        &self,
        bytes: &[u8],
        candidate_id: Option<&str>,
    ) -> Result<Vec<u8>, String> {
        let photo = image::load_from_memory(bytes)
            .map_err(|error| format!("参考照片无法解码：{error}"))?
            .to_rgb8();
        let candidates = self.reference_photo_candidates_from_rgb(&photo)?;
        let candidate = select_reference_photo_candidate(&candidates, candidate_id)?;
        let crop = crop_normalized_rect(
            &photo,
            candidate.x,
            candidate.y,
            candidate.width,
            candidate.height,
        );
        let mut encoded = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut encoded, 92)
            .encode_image(&image::DynamicImage::ImageRgb8(crop))
            .map_err(|error| format!("保存目标人员裁剪图失败：{error}"))?;
        Ok(encoded)
    }

    /// 参考照片的人体外观特征。只接受检测到的有效人体，避免把背景编码进人员模板。
    pub fn body_embedding_from_photo_bytes(&self, bytes: &[u8]) -> Result<Vec<f32>, String> {
        let photo = image::load_from_memory(bytes)
            .map_err(|error| format!("参考照片无法解码：{error}"))?
            .to_rgb8();
        let detected_people = self.detect_people(&photo)?;
        let person = select_reference_person(&detected_people, &photo)
            .ok_or_else(|| "参考照片中未检测到清晰完整的人体".to_string())?;
        let crop = crop_person(&photo, person);
        self.extract_body_embedding(&crop)
    }

    fn detect_people(&self, image: &image::RgbImage) -> Result<Vec<DetectedPerson>, String> {
        if self.model_state.runtime_backend == RuntimeBackend::OpenVino {
            return self.detect_openvino_people(image);
        }
        let ratio = (PERSON_DETECTOR_SIZE as f32 / image.width() as f32)
            .min(PERSON_DETECTOR_SIZE as f32 / image.height() as f32);
        let target_w = (image.width() as f32 * ratio).round().max(1.0) as u32;
        let target_h = (image.height() as f32 * ratio).round().max(1.0) as u32;
        let resized = image::imageops::resize(image, target_w, target_h, FilterType::Triangle);
        let mut input = Array4::<f32>::from_elem(
            (
                1,
                3,
                PERSON_DETECTOR_SIZE as usize,
                PERSON_DETECTOR_SIZE as usize,
            ),
            114.0,
        );
        for y in 0..target_h as usize {
            for x in 0..target_w as usize {
                let pixel = resized.get_pixel(x as u32, y as u32);
                for channel in 0..3 {
                    input[[0, channel, y, x]] = f32::from(pixel[channel]);
                }
            }
        }
        let detector = self
            .person_detector
            .as_ref()
            .ok_or_else(|| "人体检测模型未就绪".to_string())?;
        let mut session = detector
            .lock()
            .map_err(|_| "人体检测器被占用".to_string())?;
        #[cfg(feature = "openvino-runtime")]
        let InferenceSession::Onnx(session) = &mut *session
        else {
            return Err("人体检测模型后端不匹配".to_string());
        };
        #[cfg(not(feature = "openvino-runtime"))]
        let InferenceSession::Onnx(session) = &mut *session;
        let outputs = session
            .run(ort::inputs![ort::value::TensorRef::from_array_view(&input)
                .map_err(|error| format!("构造人体检测输入失败：{error}"))?])
            .map_err(|error| format!("人体检测推理失败：{error}"))?;
        let (_, tensor) = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|error| format!("读取人体检测结果失败：{error}"))?;
        // 运行帧适当降低人体检测门限以保留远处小目标，后续由 ReID 差距和多帧门控过滤。
        let people = decode_yolox_people(tensor, ratio, image.width(), image.height(), 0.35);
        Ok(nms_people(people, 0.5, 5))
    }

    #[cfg(feature = "openvino-runtime")]
    fn detect_openvino_people(
        &self,
        image: &image::RgbImage,
    ) -> Result<Vec<DetectedPerson>, String> {
        let detector = self
            .person_detector
            .as_ref()
            .ok_or_else(|| "人体检测模型未就绪".to_string())?;
        let mut session = detector
            .lock()
            .map_err(|_| "人体检测器被占用".to_string())?;
        let InferenceSession::OpenVino(session) = &mut *session else {
            return Err("人体检测模型后端不匹配".to_string());
        };
        let input_shape = session.input_shape().to_vec();
        let (height, width) = (
            u32::try_from(input_shape[2]).map_err(|_| "人体检测模型输入高度无效".to_string())?,
            u32::try_from(input_shape[3]).map_err(|_| "人体检测模型输入宽度无效".to_string())?,
        );
        let resized = image::imageops::resize(image, width, height, FilterType::Triangle);
        let input = openvino_bgr_input(&resized, width, height);
        let output = session.infer_f32(&input_shape, &input)?;
        let people =
            decode_openvino_people(&output, image.width() as f32, image.height() as f32, 0.35);
        Ok(nms_people(people, 0.5, 5))
    }

    #[cfg(not(feature = "openvino-runtime"))]
    fn detect_openvino_people(
        &self,
        _image: &image::RgbImage,
    ) -> Result<Vec<DetectedPerson>, String> {
        Err("VISION_OPENVINO_RUNTIME_REQUIRED".to_string())
    }

    fn reference_photo_candidates_from_rgb(
        &self,
        photo: &image::RgbImage,
    ) -> Result<Vec<ReferencePhotoCandidate>, String> {
        let detection = self
            .detect_in_rgb(photo)?
            .filter(|value| !value.faces.is_empty())
            .ok_or_else(|| "参考照片中未检测到可用人脸，请选择人脸清晰的照片".to_string())?;
        let people = if self.person_detector.is_some() {
            self.detect_people(photo)?
        } else {
            Vec::new()
        };
        let width = photo.width().max(1) as f32;
        let height = photo.height().max(1) as f32;
        let scale_x = width / DETECTOR_SIZE as f32;
        let scale_y = height / DETECTOR_SIZE as f32;
        let mut faces = detection.faces;
        // 以阅读顺序生成稳定编号，重新分析同一张图时不会因模型分数微小抖动选错人。
        faces.sort_by(|left, right| {
            (left.y1 * scale_y)
                .total_cmp(&(right.y1 * scale_y))
                .then_with(|| (left.x1 * scale_x).total_cmp(&(right.x1 * scale_x)))
        });
        Ok(faces
            .iter()
            .enumerate()
            .map(|(index, face)| {
                let face_x = face.x1 * scale_x;
                let face_y = face.y1 * scale_y;
                let face_w = face.w * scale_x;
                let face_h = face.h * scale_y;
                let (x, y, w, h, uses_person_crop) = people
                    .iter()
                    .filter(|person| person_contains_face(person, face_x, face_y, face_w, face_h))
                    .min_by(|left, right| (left.w * left.h).total_cmp(&(right.w * right.h)))
                    .map(|person| (person.x, person.y, person.w, person.h, true))
                    .unwrap_or_else(|| {
                        expanded_face_reference_crop(face_x, face_y, face_w, face_h, width, height)
                    });
                ReferencePhotoCandidate {
                    candidate_id: format!("face-{index}"),
                    x: (x / width).clamp(0.0, 1.0),
                    y: (y / height).clamp(0.0, 1.0),
                    width: (w / width).clamp(0.0, 1.0),
                    height: (h / height).clamp(0.0, 1.0),
                    score: face.score,
                    uses_person_crop,
                }
            })
            .collect())
    }

    fn extract_body_embedding(&self, image: &image::RgbImage) -> Result<Vec<f32>, String> {
        if self.model_state.runtime_backend == RuntimeBackend::OpenVino {
            return self.extract_openvino_body_embedding(image);
        }
        let spec = &self.model_state.person_reid_spec;
        let resized = image::imageops::resize(image, spec.width, spec.height, FilterType::Triangle);
        if spec.normalization != "imagenet" {
            return Err("人体识别模型预处理方式不受支持".to_string());
        }
        let mean = [0.485_f32, 0.456, 0.406];
        let std = [0.229_f32, 0.224, 0.225];
        let mut input = Array4::<f32>::zeros((1, 3, spec.height as usize, spec.width as usize));
        for y in 0..spec.height as usize {
            for x in 0..spec.width as usize {
                let pixel = resized.get_pixel(x as u32, y as u32);
                for channel in 0..3 {
                    let source_channel = if spec.color_order == "BGR" {
                        2 - channel
                    } else {
                        channel
                    };
                    input[[0, channel, y, x]] =
                        (f32::from(pixel[source_channel]) / 255.0 - mean[channel]) / std[channel];
                }
            }
        }
        let recognizer = self
            .person_recognizer
            .as_ref()
            .ok_or_else(|| "人体识别模型未就绪".to_string())?;
        let mut session = recognizer
            .lock()
            .map_err(|_| "人体识别器被占用".to_string())?;
        #[cfg(feature = "openvino-runtime")]
        let InferenceSession::Onnx(session) = &mut *session
        else {
            return Err("人体识别模型后端不匹配".to_string());
        };
        #[cfg(not(feature = "openvino-runtime"))]
        let InferenceSession::Onnx(session) = &mut *session;
        let outputs = session
            .run(ort::inputs![ort::value::TensorRef::from_array_view(&input)
                .map_err(|error| format!("构造人体识别输入失败：{error}"))?])
            .map_err(|error| format!("人体识别推理失败：{error}"))?;
        let (_, tensor) = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|error| format!("读取人体识别结果失败：{error}"))?;
        if tensor.len() != spec.output_dimension {
            return Err(format!("人体识别模型输出维数异常：{}", tensor.len()));
        }
        let mut embedding = tensor.to_vec();
        normalize_embedding(&mut embedding);
        Ok(embedding)
    }

    #[cfg(feature = "openvino-runtime")]
    fn extract_openvino_body_embedding(&self, image: &image::RgbImage) -> Result<Vec<f32>, String> {
        let spec = &self.model_state.person_reid_spec;
        let resized = image::imageops::resize(image, spec.width, spec.height, FilterType::Triangle);
        let input = openvino_bgr_input(&resized, spec.width, spec.height);
        let recognizer = self
            .person_recognizer
            .as_ref()
            .ok_or_else(|| "人体识别模型未就绪".to_string())?;
        let mut session = recognizer
            .lock()
            .map_err(|_| "人体识别器被占用".to_string())?;
        let InferenceSession::OpenVino(session) = &mut *session else {
            return Err("人体识别模型后端不匹配".to_string());
        };
        let mut embedding = session.infer_f32(
            &[1, 3, i64::from(spec.height), i64::from(spec.width)],
            &input,
        )?;
        if embedding.len() != spec.output_dimension {
            return Err(format!("人体识别模型输出维数异常：{}", embedding.len()));
        }
        normalize_embedding(&mut embedding);
        Ok(embedding)
    }

    #[cfg(not(feature = "openvino-runtime"))]
    fn extract_openvino_body_embedding(
        &self,
        _image: &image::RgbImage,
    ) -> Result<Vec<f32>, String> {
        Err("VISION_OPENVINO_RUNTIME_REQUIRED".to_string())
    }
}

/// OMZ Retail 模型按 NCHW 提供 BGR 的 0..255 浮点数据；模型 IR 自身承载
/// 对应的预处理图，不能套用 OSNet 的 ImageNet 标准化。
#[cfg(feature = "openvino-runtime")]
fn openvino_bgr_input(image: &image::RgbImage, width: u32, height: u32) -> Vec<f32> {
    debug_assert_eq!(image.width(), width);
    debug_assert_eq!(image.height(), height);
    let plane = width as usize * height as usize;
    let mut input = vec![0.0_f32; plane * 3];
    for y in 0..height as usize {
        for x in 0..width as usize {
            let pixel = image.get_pixel(x as u32, y as u32);
            let index = y * width as usize + x;
            input[index] = f32::from(pixel[2]);
            input[plane + index] = f32::from(pixel[1]);
            input[plane * 2 + index] = f32::from(pixel[0]);
        }
    }
    input
}

/// OpenVINO DetectionOutput 的每项均为
/// `[batch_id, class_id, confidence, x1, y1, x2, y2]`，坐标是归一化值。
fn decode_openvino_detection_output(
    output: &[f32],
    width: f32,
    height: f32,
    min_score: f32,
) -> Vec<(f32, f32, f32, f32, f32)> {
    output
        .chunks_exact(7)
        .take_while(|detection| detection[0] >= 0.0)
        .filter_map(|detection| {
            let score = detection[2];
            if score < min_score {
                return None;
            }
            let x1 = (detection[3] * width).clamp(0.0, width);
            let y1 = (detection[4] * height).clamp(0.0, height);
            let x2 = (detection[5] * width).clamp(0.0, width);
            let y2 = (detection[6] * height).clamp(0.0, height);
            let box_width = (x2 - x1).max(0.0);
            let box_height = (y2 - y1).max(0.0);
            (box_width >= 2.0 && box_height >= 2.0)
                .then_some((x1, y1, box_width, box_height, score))
        })
        .collect()
}

#[cfg(feature = "openvino-runtime")]
fn decode_openvino_faces(
    output: &[f32],
    width: f32,
    height: f32,
    min_score: f32,
) -> Vec<DetectedFace> {
    decode_openvino_detection_output(output, width, height, min_score)
        .into_iter()
        .map(|(x1, y1, w, h, score)| DetectedFace {
            x1,
            y1,
            w,
            h,
            // Retail detector不输出关键点。使用稳定的相对点位，让现有对齐流程
            // 能把检测框裁切成近似正脸区域供 0095 提特征。
            landmarks: [
                (x1 + w * 0.32, y1 + h * 0.38),
                (x1 + w * 0.68, y1 + h * 0.38),
                (x1 + w * 0.50, y1 + h * 0.56),
                (x1 + w * 0.37, y1 + h * 0.74),
                (x1 + w * 0.63, y1 + h * 0.74),
            ],
            score,
        })
        .collect()
}

#[cfg(feature = "openvino-runtime")]
fn decode_openvino_people(
    output: &[f32],
    width: f32,
    height: f32,
    min_score: f32,
) -> Vec<DetectedPerson> {
    decode_openvino_detection_output(output, width, height, min_score)
        .into_iter()
        .map(|(x, y, w, h, score)| DetectedPerson { x, y, w, h, score })
        .collect()
}

fn normalize_score(value: f32) -> f32 {
    if (0.0..=1.0).contains(&value) {
        value
    } else {
        1.0 / (1.0 + (-value).exp())
    }
}

/// YuNet 锚框解码：每个 stride 层按 grid cell 生成先验框，分数取 cls/obj 归一化后的几何均值。
fn decode_faces(
    strides: [(f32, &[f32], &[f32], &[f32], &[f32]); 3],
    size: f32,
    min_score: f32,
) -> Vec<DetectedFace> {
    let mut faces = Vec::new();
    for (stride, cls, obj, bbox, kps) in strides {
        let cols = (size / stride) as usize;
        let rows = cols;
        for r in 0..rows {
            for c in 0..cols {
                let idx = r * cols + c;
                if idx >= cls.len()
                    || idx >= obj.len()
                    || (idx + 1) * 4 > bbox.len()
                    || (idx + 1) * 10 > kps.len()
                {
                    continue;
                }
                let score = (normalize_score(cls[idx]) * normalize_score(obj[idx])).sqrt();
                if score < min_score {
                    continue;
                }
                let cx = (c as f32 + bbox[idx * 4]) * stride;
                let cy = (r as f32 + bbox[idx * 4 + 1]) * stride;
                let w = bbox[idx * 4 + 2].exp() * stride;
                let h = bbox[idx * 4 + 3].exp() * stride;
                let mut landmarks = [(0.0_f32, 0.0_f32); 5];
                for (n, landmark) in landmarks.iter_mut().enumerate() {
                    *landmark = (
                        (kps[idx * 10 + 2 * n] + c as f32) * stride,
                        (kps[idx * 10 + 2 * n + 1] + r as f32) * stride,
                    );
                }
                faces.push(DetectedFace {
                    x1: cx - w / 2.0,
                    y1: cy - h / 2.0,
                    w,
                    h,
                    landmarks,
                    score,
                });
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
    if union <= 0.0 {
        0.0
    } else {
        intersection / union
    }
}

/// 贪心 NMS：按分数降序保留，抑制与之 IoU 超阈值的锚框，至多保留 top_k 张脸。
fn nms_faces(mut faces: Vec<DetectedFace>, iou_threshold: f32, top_k: usize) -> Vec<DetectedFace> {
    faces.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut kept = Vec::new();
    for face in faces {
        if kept
            .iter()
            .any(|candidate: &DetectedFace| face_iou(candidate, &face) > iou_threshold)
        {
            continue;
        }
        kept.push(face);
        if kept.len() >= top_k {
            break;
        }
    }
    kept
}

/// 按 5 关键点把人脸区域相似变换对齐到 112×112 模板。
/// YuNet 关键点顺序为被识者视角（右眼、左眼、鼻尖、右嘴角、左嘴角），
/// 被识者右侧对应画面小 x（左侧），与模板同下标点一一配对。
fn align_face_112(image: &image::RgbImage, landmarks: [(f32, f32); 5]) -> image::RgbImage {
    let count = landmarks.len() as f32;
    let (mean_x, mean_y) = landmarks
        .iter()
        .fold((0.0_f32, 0.0_f32), |(sx, sy), (x, y)| (sx + x, sy + y));
    let (mean_x, mean_y) = (mean_x / count, mean_y / count);
    let (mean_tx, mean_ty) = ALIGN_TEMPLATE
        .iter()
        .fold((0.0_f32, 0.0_f32), |(sx, sy), (x, y)| (sx + x, sy + y));
    let (mean_tx, mean_ty) = (mean_tx / count, mean_ty / count);
    let mut denom = 0.0_f32;
    let mut sum_a = 0.0_f32;
    let mut sum_b = 0.0_f32;
    for ((x, y), (tx, ty)) in landmarks.iter().zip(ALIGN_TEMPLATE.iter()) {
        let (dx, dy) = (x - mean_x, y - mean_y);
        let (dtx, dty) = (tx - mean_tx, ty - mean_ty);
        denom += dx * dx + dy * dy;
        sum_a += dx * dtx + dy * dty;
        sum_b += dy * dtx - dx * dty;
    }
    // 退化关键点（全部重合）时直接拉伸整图作为降级对齐。
    if denom < 1e-6 {
        return image::imageops::resize(
            image,
            RECOGNIZER_SIZE,
            RECOGNIZER_SIZE,
            FilterType::Triangle,
        );
    }
    let (a, b) = (sum_a / denom, sum_b / denom);
    let norm = a * a + b * b;
    if norm < 1e-9 {
        return image::imageops::resize(
            image,
            RECOGNIZER_SIZE,
            RECOGNIZER_SIZE,
            FilterType::Triangle,
        );
    }
    let (tx, ty) = (
        mean_tx - a * mean_x + b * mean_y,
        mean_ty - b * mean_x - a * mean_y,
    );
    let inv = 1.0 / norm;
    let mut output = image::RgbImage::new(RECOGNIZER_SIZE, RECOGNIZER_SIZE);
    for v in 0..RECOGNIZER_SIZE {
        for u in 0..RECOGNIZER_SIZE {
            let du = u as f32 - tx;
            let dv = v as f32 - ty;
            let x = (a * du + b * dv) * inv;
            let y = (a * dv - b * du) * inv;
            output.put_pixel(u, v, image::Rgb(sample_bilinear_rgb(image, x, y)));
        }
    }
    output
}

fn sample_bilinear_rgb(image: &image::RgbImage, x: f32, y: f32) -> [u8; 3] {
    let x0 = x.floor() as i64;
    let y0 = y.floor() as i64;
    let fx = x - x.floor();
    let fy = y - y.floor();
    let pixel = |px: i64, py: i64| -> [f32; 3] {
        if px < 0 || py < 0 || px >= i64::from(image.width()) || py >= i64::from(image.height()) {
            return [0.0, 0.0, 0.0];
        }
        let value = image.get_pixel(px as u32, py as u32);
        [
            f32::from(value[0]),
            f32::from(value[1]),
            f32::from(value[2]),
        ]
    };
    let top_left = pixel(x0, y0);
    let top_right = pixel(x0 + 1, y0);
    let bottom_left = pixel(x0, y0 + 1);
    let bottom_right = pixel(x0 + 1, y0 + 1);
    let mut result = [0_u8; 3];
    for channel in 0..3 {
        let top = top_left[channel] * (1.0 - fx) + top_right[channel] * fx;
        let bottom = bottom_left[channel] * (1.0 - fx) + bottom_right[channel] * fx;
        result[channel] = (top * (1.0 - fy) + bottom * fy).round().clamp(0.0, 255.0) as u8;
    }
    result
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let mut dot = 0.0_f32;
    let mut norm_a = 0.0_f32;
    let mut norm_b = 0.0_f32;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }
    if norm_a <= 0.0 || norm_b <= 0.0 {
        return 0.0;
    }
    dot / (norm_a.sqrt() * norm_b.sqrt())
}

pub fn normalize_embedding(embedding: &mut [f32]) {
    let norm = embedding
        .iter()
        .map(|value| value * value)
        .sum::<f32>()
        .sqrt();
    if norm > 0.0 {
        for value in embedding.iter_mut() {
            *value /= norm;
        }
    }
}

/// 将同一人员最接近的三条样本按衰减权重融合，避免单张样本偶然高分造成误识别。
/// 少于三张样本时，已使用权重会自动归一化，兼容原有单样本人员。
fn weighted_similarity<T: AsRef<[f32]>>(embedding: &[f32], samples: &[T]) -> Option<f32> {
    const TOP_K_WEIGHTS: [f32; 3] = [0.60, 0.28, 0.12];
    let mut scores: Vec<f32> = samples
        .iter()
        .map(|sample| cosine_similarity(embedding, sample.as_ref()))
        .filter(|score| score.is_finite())
        .collect();
    scores.sort_by(|left, right| right.total_cmp(left));
    let selected = scores.iter().take(TOP_K_WEIGHTS.len()).collect::<Vec<_>>();
    if selected.is_empty() {
        return None;
    }
    let weight_sum: f32 = TOP_K_WEIGHTS.iter().take(selected.len()).sum();
    Some(
        selected
            .iter()
            .enumerate()
            .map(|(index, score)| **score * TOP_K_WEIGHTS[index] / weight_sum)
            .sum(),
    )
}

fn weighted_person_similarity(embedding: &[f32], samples: &[Vec<f32>]) -> Option<f32> {
    weighted_similarity(embedding, samples)
}

fn decode_yolox_people(
    output: &[f32],
    ratio: f32,
    image_width: u32,
    image_height: u32,
    min_score: f32,
) -> Vec<DetectedPerson> {
    const ROW_WIDTH: usize = 85;
    let mut people = Vec::new();
    let mut row_index = 0usize;
    for stride in [8usize, 16, 32] {
        let grid = PERSON_DETECTOR_SIZE as usize / stride;
        for y in 0..grid {
            for x in 0..grid {
                let offset = row_index * ROW_WIDTH;
                row_index += 1;
                if offset + ROW_WIDTH > output.len() {
                    return people;
                }
                let objectness = normalize_score(output[offset + 4]);
                let person_score = objectness * normalize_score(output[offset + 5]);
                if person_score < min_score {
                    continue;
                }
                let cx = (output[offset] + x as f32) * stride as f32;
                let cy = (output[offset + 1] + y as f32) * stride as f32;
                let w = output[offset + 2].clamp(-20.0, 20.0).exp() * stride as f32;
                let h = output[offset + 3].clamp(-20.0, 20.0).exp() * stride as f32;
                let x1 = ((cx - w * 0.5) / ratio).clamp(0.0, image_width.saturating_sub(1) as f32);
                let y1 = ((cy - h * 0.5) / ratio).clamp(0.0, image_height.saturating_sub(1) as f32);
                let x2 = ((cx + w * 0.5) / ratio).clamp(x1 + 1.0, image_width as f32);
                let y2 = ((cy + h * 0.5) / ratio).clamp(y1 + 1.0, image_height as f32);
                people.push(DetectedPerson {
                    x: x1,
                    y: y1,
                    w: x2 - x1,
                    h: y2 - y1,
                    score: person_score,
                });
            }
        }
    }
    people
}

fn nms_people(
    mut people: Vec<DetectedPerson>,
    iou_threshold: f32,
    top_k: usize,
) -> Vec<DetectedPerson> {
    people.sort_by(|left, right| right.score.total_cmp(&left.score));
    let mut kept = Vec::new();
    while let Some(candidate) = people.first().cloned() {
        people.remove(0);
        people.retain(|other| person_iou(&candidate, other) < iou_threshold);
        kept.push(candidate);
        if kept.len() >= top_k {
            break;
        }
    }
    kept
}

fn person_iou(left: &DetectedPerson, right: &DetectedPerson) -> f32 {
    let x1 = left.x.max(right.x);
    let y1 = left.y.max(right.y);
    let x2 = (left.x + left.w).min(right.x + right.w);
    let y2 = (left.y + left.h).min(right.y + right.h);
    let intersection = (x2 - x1).max(0.0) * (y2 - y1).max(0.0);
    let union = left.w * left.h + right.w * right.h - intersection;
    if union <= 0.0 {
        0.0
    } else {
        intersection / union
    }
}

fn crop_person(image: &image::RgbImage, person: &DetectedPerson) -> image::RgbImage {
    let x = person.x.floor().max(0.0) as u32;
    let y = person.y.floor().max(0.0) as u32;
    let width = person.w.ceil().max(1.0) as u32;
    let height = person.h.ceil().max(1.0) as u32;
    image::imageops::crop_imm(
        image,
        x.min(image.width() - 1),
        y.min(image.height() - 1),
        width.min(image.width() - x.min(image.width() - 1)),
        height.min(image.height() - y.min(image.height() - 1)),
    )
    .to_image()
}

fn crop_normalized_rect(
    image: &image::RgbImage,
    normalized_x: f32,
    normalized_y: f32,
    normalized_width: f32,
    normalized_height: f32,
) -> image::RgbImage {
    let image_width = image.width().max(1);
    let image_height = image.height().max(1);
    let x = (normalized_x.clamp(0.0, 1.0) * image_width as f32).floor() as u32;
    let y = (normalized_y.clamp(0.0, 1.0) * image_height as f32).floor() as u32;
    let width = (normalized_width.clamp(0.0, 1.0) * image_width as f32)
        .ceil()
        .max(1.0) as u32;
    let height = (normalized_height.clamp(0.0, 1.0) * image_height as f32)
        .ceil()
        .max(1.0) as u32;
    image::imageops::crop_imm(
        image,
        x.min(image_width - 1),
        y.min(image_height - 1),
        width.min(image_width - x.min(image_width - 1)),
        height.min(image_height - y.min(image_height - 1)),
    )
    .to_image()
}

fn person_contains_face(
    person: &DetectedPerson,
    face_x: f32,
    face_y: f32,
    face_width: f32,
    face_height: f32,
) -> bool {
    let center_x = face_x + face_width * 0.5;
    let center_y = face_y + face_height * 0.5;
    center_x >= person.x
        && center_x <= person.x + person.w
        && center_y >= person.y
        && center_y <= person.y + person.h
}

fn expanded_face_reference_crop(
    face_x: f32,
    face_y: f32,
    face_width: f32,
    face_height: f32,
    image_width: f32,
    image_height: f32,
) -> (f32, f32, f32, f32, bool) {
    let target_width = (face_width * 3.0).min(image_width).max(1.0);
    let target_height = (face_height * 4.0).min(image_height).max(1.0);
    let x = (face_x + face_width * 0.5 - target_width * 0.5)
        .clamp(0.0, (image_width - target_width).max(0.0));
    let y = (face_y - face_height * 0.55).clamp(0.0, (image_height - target_height).max(0.0));
    (x, y, target_width, target_height, false)
}

fn select_reference_photo_candidate<'a>(
    candidates: &'a [ReferencePhotoCandidate],
    candidate_id: Option<&str>,
) -> Result<&'a ReferencePhotoCandidate, String> {
    match (
        candidates.len(),
        candidate_id.map(str::trim).filter(|id| !id.is_empty()),
    ) {
        (0, _) => Err("参考照片中未检测到可用人脸，请选择人脸清晰的照片".to_string()),
        (1, None) => Ok(&candidates[0]),
        (_, Some(candidate_id)) => candidates
            .iter()
            .find(|candidate| candidate.candidate_id == candidate_id)
            .ok_or_else(|| "所选目标人员已失效，请重新选择照片中的人员".to_string()),
        _ => Err("参考照片中检测到多个人，请选择要录入的目标人员".to_string()),
    }
}

fn select_reference_person<'a>(
    people: &'a [DetectedPerson],
    image: &image::RgbImage,
) -> Option<&'a DetectedPerson> {
    let min_height = (image.height() as f32 * 0.20).max(72.0);
    people
        .iter()
        .filter(|person| {
            let aspect = person.w / person.h.max(1.0);
            person.score >= 0.45
                && person.h >= min_height
                && person.w >= 28.0
                && (0.18..=1.15).contains(&aspect)
        })
        .max_by(|left, right| {
            let left_quality = left.score * left.w * left.h;
            let right_quality = right.score * right.w * right.h;
            left_quality.total_cmp(&right_quality)
        })
}

/// 在人员模板中取多样本加权相似度最高者；匹配置信度 = 相似度×100（负值 clamp 到 0），
/// 是否达到告警门限由 accept_match 的 min_confidence 把关。
fn best_face_match(embedding: &[f32], people: &[PersonTemplate]) -> Option<FaceMatch> {
    let mut best: Option<(f32, &PersonTemplate)> = None;
    for person in people {
        let Some(similarity) = weighted_person_similarity(embedding, &person.face_embeddings)
        else {
            continue;
        };
        if best.as_ref().map_or(true, |(score, _)| similarity > *score) {
            best = Some((similarity, person));
        }
    }
    best.map(|(similarity, person)| FaceMatch {
        person_id: person.person_id.clone(),
        display_name: person.display_name.clone(),
        confidence: (similarity * 100.0).round().clamp(0.0, 100.0) as u8,
        recognition_level: "confirmed".to_string(),
        face_confidence: Some((similarity * 100.0).round().clamp(0.0, 100.0) as u8),
        body_confidence: None,
    })
}

fn best_body_match(embedding: &[f32], people: &[PersonTemplate]) -> Option<FaceMatch> {
    let mut candidates = Vec::new();
    for person in people {
        let Some(similarity) = weighted_similarity(embedding, &person.body_embeddings) else {
            continue;
        };
        candidates.push((similarity, person));
    }
    candidates.sort_by(|left, right| right.0.total_cmp(&left.0));
    let (similarity, person) = candidates.first().copied()?;
    if candidates
        .get(1)
        .is_some_and(|(runner_up, _)| similarity - *runner_up < BODY_MATCH_MIN_MARGIN)
    {
        return None;
    }
    Some({
        let confidence = body_similarity_to_confidence(similarity);
        FaceMatch {
            person_id: person.person_id.clone(),
            display_name: person.display_name.clone(),
            confidence,
            recognition_level: "suspected".to_string(),
            face_confidence: None,
            body_confidence: Some(confidence),
        }
    })
}

fn body_similarity_to_confidence(similarity: f32) -> u8 {
    // ReID cosine scores typically occupy a narrower positive band than SFace.
    // 0.35 maps to 0 and 0.85 maps to 100, leaving policy thresholds intuitive.
    (((similarity - 0.35) / 0.50) * 100.0)
        .round()
        .clamp(0.0, 100.0) as u8
}

/// 特征落库序列化：小端 f32。向量维度由对应 Embedding Space 的模型语义决定。
pub fn embedding_bytes(embedding: &[f32]) -> Vec<u8> {
    embedding
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

pub fn embedding_from_bytes(bytes: &[u8]) -> Result<[f32; 128], String> {
    if bytes.len() != 128 * 4 {
        return Err(format!("人脸特征长度无效：{}", bytes.len()));
    }
    let mut embedding = [0.0_f32; 128];
    for (slot, chunk) in embedding.iter_mut().zip(bytes.chunks_exact(4)) {
        *slot = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
    }
    Ok(embedding)
}

pub fn dynamic_embedding_bytes(embedding: &[f32]) -> Vec<u8> {
    embedding
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

pub fn dynamic_embedding_from_bytes(bytes: &[u8], dimensions: usize) -> Result<Vec<f32>, String> {
    if bytes.len() != dimensions * 4 {
        return Err(format!("人物特征长度无效：{}", bytes.len()));
    }
    Ok(bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect())
}

fn model_state_from_dir(dir: &Path) -> Result<FaceModelState, String> {
    let v4_path = dir.join("manifest.v4.json");
    if v4_path.is_file() {
        return model_state_from_v4(dir, &v4_path);
    }
    let manifest_path = dir.join("manifest.json");
    let manifest_text = fs::read_to_string(&manifest_path)
        .map_err(|_| format!("未找到模型清单：{}", manifest_path.display()))?;
    let manifest: FaceModelManifest = serde_json::from_str(&manifest_text)
        .map_err(|error| format!("模型清单格式无效：{error}"))?;
    if manifest.schema_version < 1 || manifest.schema_version > 3 {
        return Err(format!(
            "不支持的人物检测模型清单版本：{}",
            manifest.schema_version
        ));
    }
    if manifest.model_version.trim().is_empty() || manifest.model_version == "uninstalled" {
        return Err("人脸检测模型尚未安装".to_string());
    }
    let detector_path = validate_model_asset(dir, "检测", &manifest.detector)?;
    let recognizer_path = match manifest.recognizer {
        Some(asset) => Some(validate_model_asset(dir, "识别", &asset)?),
        None => None,
    };
    let mut optional_error = None;
    let person_detector_path = manifest.person_detector.as_ref().and_then(|asset| {
        match validate_model_asset(dir, "人体检测", asset) {
            Ok(path) => Some(path),
            Err(error) => {
                optional_error = Some(error);
                None
            }
        }
    });
    let person_recognizer_path = manifest.person_recognizer.as_ref().and_then(|asset| {
        match validate_model_asset(dir, "人体识别", asset) {
            Ok(path) => Some(path),
            Err(error) => {
                if optional_error.is_none() {
                    optional_error = Some(error);
                }
                None
            }
        }
    });
    let face_embedding_space_id = legacy_embedding_space("face", &manifest.model_version);
    let body_embedding_space_id = legacy_embedding_space("body", &manifest.model_version);
    Ok(FaceModelState {
        ready: true,
        version: Some(manifest.model_version),
        profile_id: Some("legacy".to_string()),
        face_embedding_space_id: Some(face_embedding_space_id),
        body_embedding_space_id: Some(body_embedding_space_id),
        error: optional_error,
        detector_path: Some(detector_path),
        recognizer_path,
        person_detector_path,
        person_recognizer_path,
        runtime_backend: RuntimeBackend::OnnxRuntime,
        face_recognizer_spec: FaceRecognizerSpec::default(),
        person_reid_spec: PersonReIdSpec::default(),
    })
}

fn model_state_from_v4(dir: &Path, manifest_path: &Path) -> Result<FaceModelState, String> {
    let manifest_text = fs::read_to_string(manifest_path)
        .map_err(|_| format!("未找到模型清单：{}", manifest_path.display()))?;
    let manifest: VisionManifestV4 = serde_json::from_str(&manifest_text)
        .map_err(|error| format!("模型清单格式无效：{error}"))?;
    validate_manifest_v4(&manifest).map_err(|error| format!("模型清单校验失败：{error}"))?;
    let runtime_backend = RuntimeBackend::from_manifest_name(&manifest.profile.engine)
        .ok_or_else(|| "当前兼容运行时不支持该模型引擎".to_string())?;
    if runtime_backend == RuntimeBackend::OpenVino {
        openvino_runtime::verify_runtime()?;
    }

    let component = |id: &str| {
        manifest
            .components
            .iter()
            .find(|component| component.id == id)
            .ok_or_else(|| "V4 模型管线缺少组件".to_string())
    };
    let face_detector = manifest
        .components
        .iter()
        .find(|component| component.category == "face_detector")
        .ok_or_else(|| "V4 模型管线缺少人脸检测器".to_string())?;
    let face_recognizer = component(&manifest.pipeline.face_engine)?;
    let person_detector = component(&manifest.pipeline.person_detector)?;
    let person_reid = component(&manifest.pipeline.person_re_id_engine)?;
    let supported_adapter_stack = match runtime_backend {
        RuntimeBackend::OnnxRuntime => {
            face_detector.adapter_id == "builtin.face-detector.yunet.v1"
                && matches!(
                    face_recognizer.adapter_id.as_str(),
                    "builtin.face-recognizer.sface.v1" | "builtin.face-recognizer.arcface.v1"
                )
                && person_detector.adapter_id == "builtin.person-detector.yolox.v1"
                && matches!(
                    person_reid.adapter_id.as_str(),
                    "builtin.person-reid.youtu.v1" | "builtin.person-reid.osnet.v1"
                )
        }
        RuntimeBackend::OpenVino => {
            face_detector.adapter_id == "builtin.face-detector.omz.v1"
                && face_recognizer.adapter_id == "builtin.face-recognizer.omz.v1"
                && person_detector.adapter_id == "builtin.person-detector.omz.v1"
                && matches!(
                    person_reid.adapter_id.as_str(),
                    "builtin.person-reid.omz.v1" | "builtin.person-reid.omz.0286.v1"
                )
        }
    };
    if !supported_adapter_stack {
        return Err("当前兼容运行时不支持该 V4 模型适配器组合".to_string());
    }
    let face_recognizer_spec = face_recognizer_spec_from_v4(face_recognizer)?;
    let person_reid_spec = person_reid_spec_from_v4(person_reid)?;
    let pipeline =
        resolve_pipeline(&manifest).map_err(|error| format!("模型管线解析失败：{error}"))?;
    Ok(FaceModelState {
        ready: true,
        version: Some(manifest.profile.version),
        profile_id: Some(manifest.profile.id),
        face_embedding_space_id: Some(
            pipeline
                .face_recognizer
                .embedding_space_id
                .as_str()
                .to_string(),
        ),
        body_embedding_space_id: Some(pipeline.person_reid.embedding_space_id.as_str().to_string()),
        error: None,
        detector_path: Some(validate_v4_model_asset(dir, "人脸检测", face_detector)?),
        recognizer_path: Some(validate_v4_model_asset(dir, "人脸识别", face_recognizer)?),
        person_detector_path: Some(validate_v4_model_asset(dir, "人体检测", person_detector)?),
        person_recognizer_path: Some(validate_v4_model_asset(dir, "人体识别", person_reid)?),
        runtime_backend,
        face_recognizer_spec,
        person_reid_spec,
    })
}

fn legacy_embedding_space(modality: &str, model_version: &str) -> String {
    let digest = hex::encode(Sha256::digest(model_version.as_bytes()));
    format!("legacy.{modality}.{}", &digest[..16])
}

fn validate_v4_model_asset(
    dir: &Path,
    label: &str,
    asset: &V4ComponentDescriptor,
) -> Result<PathBuf, String> {
    let path = validate_model_asset(
        dir,
        label,
        &FaceModelAsset {
            file: asset.file.clone(),
            sha256: asset.sha256.clone(),
        },
    )?;
    if asset.engine == "openvino" {
        openvino_runtime::validate_ir_pair(&path)?;
    }
    Ok(path)
}

fn face_recognizer_spec_from_v4(
    component: &V4ComponentDescriptor,
) -> Result<FaceRecognizerSpec, String> {
    let input = component
        .input
        .as_ref()
        .ok_or_else(|| "人脸识别模型缺少输入语义".to_string())?;
    let output = component
        .output
        .as_ref()
        .ok_or_else(|| "人脸识别模型缺少输出语义".to_string())?;
    let (width, height) = match input.resize_mode.as_str() {
        "aligned_112" => (112, 112),
        value => value
            .split_once('x')
            .and_then(|(width, height)| {
                Some((width.parse::<u32>().ok()?, height.parse::<u32>().ok()?))
            })
            .filter(|(width, height)| (64..=512).contains(width) && (64..=512).contains(height))
            .ok_or_else(|| "人脸识别模型输入尺寸无效".to_string())?,
    };
    if output.embedding_dimension == 0 || output.distance_metric != "cosine" {
        return Err("人脸识别模型输出语义不受支持".to_string());
    }
    match component.adapter_id.as_str() {
        "builtin.face-recognizer.sface.v1"
            if input.color_order == "BGR"
                && input.normalization == "sface_127_5"
                && output.embedding_dimension == 128 => {}
        "builtin.face-recognizer.arcface.v1"
            if matches!(input.color_order.as_str(), "RGB" | "BGR")
                && input.normalization == "arcface_127_5"
                && output.embedding_dimension == 512 => {}
        "builtin.face-recognizer.omz.v1"
            if input.color_order == "BGR"
                && input.normalization == "openvino_retail"
                && output.embedding_dimension == 256 => {}
        _ => return Err("当前兼容运行时不支持该人脸识别模型语义".to_string()),
    }
    Ok(FaceRecognizerSpec {
        width,
        height,
        output_dimension: output.embedding_dimension as usize,
        color_order: input.color_order.clone(),
        normalization: input.normalization.clone(),
    })
}

fn person_reid_spec_from_v4(component: &V4ComponentDescriptor) -> Result<PersonReIdSpec, String> {
    let input = component
        .input
        .as_ref()
        .ok_or_else(|| "人体识别模型缺少输入语义".to_string())?;
    let output = component
        .output
        .as_ref()
        .ok_or_else(|| "人体识别模型缺少输出语义".to_string())?;
    let (height, width) = input
        .resize_mode
        .split_once('x')
        .and_then(|(height, width)| Some((height.parse::<u32>().ok()?, width.parse::<u32>().ok()?)))
        .filter(|(height, width)| (32..=1024).contains(height) && (16..=1024).contains(width))
        .ok_or_else(|| "人体识别模型输入尺寸无效".to_string())?;
    let supported = match component.adapter_id.as_str() {
        "builtin.person-reid.youtu.v1" | "builtin.person-reid.osnet.v1" => {
            matches!(input.color_order.as_str(), "RGB" | "BGR") && input.normalization == "imagenet"
        }
        "builtin.person-reid.omz.v1" | "builtin.person-reid.omz.0286.v1" => {
            input.color_order == "BGR" && input.normalization == "openvino_retail"
        }
        _ => false,
    };
    if !supported || output.embedding_dimension == 0 || output.distance_metric != "cosine" {
        return Err("人体识别模型语义不受支持".to_string());
    }
    Ok(PersonReIdSpec {
        width,
        height,
        output_dimension: output.embedding_dimension as usize,
        color_order: input.color_order.clone(),
        normalization: input.normalization.clone(),
    })
}

fn validate_model_asset(
    dir: &Path,
    label: &str,
    asset: &FaceModelAsset,
) -> Result<PathBuf, String> {
    if asset.file.trim().is_empty() || asset.sha256.len() != 64 {
        return Err(format!("{label}模型清单不完整"));
    }
    let path = dir.join(&asset.file);
    let bytes = fs::read(&path).map_err(|_| format!("缺少{label}模型：{}", path.display()))?;
    if !hex::encode(Sha256::digest(&bytes)).eq_ignore_ascii_case(&asset.sha256) {
        return Err(format!("{label}模型校验失败"));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_photo_requires_selection_only_when_multiple_candidates_exist() {
        let candidates = vec![
            ReferencePhotoCandidate {
                candidate_id: "face-0".to_string(),
                x: 0.1,
                y: 0.1,
                width: 0.2,
                height: 0.5,
                score: 0.9,
                uses_person_crop: true,
            },
            ReferencePhotoCandidate {
                candidate_id: "face-1".to_string(),
                x: 0.6,
                y: 0.2,
                width: 0.2,
                height: 0.5,
                score: 0.8,
                uses_person_crop: false,
            },
        ];
        assert!(select_reference_photo_candidate(&candidates, None).is_err());
        assert_eq!(
            select_reference_photo_candidate(&candidates, Some("face-1"))
                .expect("selected candidate")
                .candidate_id,
            "face-1"
        );
    }

    #[test]
    fn expanded_face_crop_stays_inside_image_bounds() {
        let (x, y, width, height, is_person_crop) =
            expanded_face_reference_crop(95.0, 80.0, 20.0, 20.0, 100.0, 100.0);
        assert!(!is_person_crop);
        assert!(x >= 0.0 && y >= 0.0);
        assert!(x + width <= 100.0);
        assert!(y + height <= 100.0);
    }

    #[test]
    fn settings_are_clamped_to_safe_sampling_range() {
        let settings = FaceMonitorLocalSettings {
            enabled: true,
            face_recognition_enabled: true,
            body_recognition_enabled: false,
            device_id: Some("  camera-1  ".to_string()),
            pause_during_call: false,
            sample_fps: 99,
            face_min_confidence: 0,
            body_min_confidence: 200,
            consecutive_hits: 0,
            face_cooldown_seconds: 1,
            body_cooldown_seconds: 100_000,
            applied_policy_version: -1,
        }
        .normalized();
        assert_eq!(settings.sample_fps, 5);
        assert_eq!(settings.face_min_confidence, 1);
        assert_eq!(settings.body_min_confidence, 100);
        assert_eq!(settings.consecutive_hits, 1);
        assert_eq!(settings.face_cooldown_seconds, 5);
        assert_eq!(settings.body_cooldown_seconds, 86_400);
        assert_eq!(settings.applied_policy_version, 0);
        assert_eq!(settings.device_id.as_deref(), Some("camera-1"));
    }

    #[test]
    fn arcface_v4_semantics_use_a_dynamic_512_dimension_embedding() {
        let component: V4ComponentDescriptor = serde_json::from_str(
            r#"{
                "id":"face-recognizer",
                "category":"face_recognizer",
                "family":"arcface",
                "file":"arcface.onnx",
                "sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "adapterId":"builtin.face-recognizer.arcface.v1",
                "engine":"onnxruntime",
                "input":{"colorOrder":"RGB","resizeMode":"aligned_112","normalization":"arcface_127_5"},
                "output":{"embeddingDimension":512,"distanceMetric":"cosine"}
            }"#,
        )
        .expect("arcface component");

        let spec = face_recognizer_spec_from_v4(&component).expect("arcface spec");
        assert_eq!(spec.output_dimension, 512);
        assert_eq!(spec.color_order, "RGB");
        assert_eq!(spec.normalization, "arcface_127_5");
    }

    #[test]
    fn omz_retail_v4_semantics_accept_openvino_bgr_embeddings() {
        let face: V4ComponentDescriptor = serde_json::from_str(
            r#"{
                "id":"face-recognizer","category":"face_recognizer","family":"omz-face-reid",
                "file":"face.xml","sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "adapterId":"builtin.face-recognizer.omz.v1","engine":"openvino",
                "input":{"colorOrder":"BGR","resizeMode":"128x128","normalization":"openvino_retail"},
                "output":{"embeddingDimension":256,"distanceMetric":"cosine"}
            }"#,
        )
        .expect("omz face component");
        let body: V4ComponentDescriptor = serde_json::from_str(
            r#"{
                "id":"person-reid","category":"person_reid","family":"omz-person-reid-0288",
                "file":"person.xml","sha256":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "adapterId":"builtin.person-reid.omz.v1","engine":"openvino",
                "input":{"colorOrder":"BGR","resizeMode":"256x128","normalization":"openvino_retail"},
                "output":{"embeddingDimension":256,"distanceMetric":"cosine"}
            }"#,
        )
        .expect("omz person component");
        assert_eq!(
            face_recognizer_spec_from_v4(&face)
                .unwrap()
                .output_dimension,
            256
        );
        assert_eq!(
            person_reid_spec_from_v4(&body).unwrap().output_dimension,
            256
        );
    }

    #[test]
    fn openvino_detection_output_decodes_normalized_boxes() {
        let output = [
            0.0, 1.0, 0.92, 0.10, 0.20, 0.60, 0.80, // valid
            -1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, // sentinel
        ];
        let decoded = decode_openvino_detection_output(&output, 200.0, 100.0, 0.50);
        assert_eq!(decoded.len(), 1);
        let (x, y, width, height, score) = decoded[0];
        assert!((x - 20.0).abs() < 0.001);
        assert!((y - 20.0).abs() < 0.001);
        assert!((width - 100.0).abs() < 0.001);
        assert!((height - 60.0).abs() < 0.001);
        assert!((score - 0.92).abs() < 0.001);
    }

    #[test]
    fn body_recognition_defaults_to_five_minute_cooldown() {
        let settings = FaceMonitorLocalSettings::default();
        assert_eq!(settings.face_cooldown_seconds, 60);
        assert_eq!(settings.body_cooldown_seconds, 300);
    }

    #[test]
    fn disabled_runtime_rejects_frames_without_retaining_them() {
        let runtime = FaceMonitorRuntime::default();
        assert!(runtime.recognize_frame(&[1, 2, 3], &[]).unwrap().is_none());
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
    fn face_and_body_matches_have_independent_cooldowns() {
        let runtime = FaceMonitorRuntime::default();
        assert!(runtime.accept_match("camera-person:confirmed:person-1", 90, 60, 1, 300, 1_000));
        assert!(runtime.accept_match("camera-person:suspected:person-1", 80, 60, 1, 5, 1_000));
        assert!(!runtime.accept_match("camera-person:confirmed:person-1", 90, 60, 1, 300, 6_000));
        assert!(runtime.accept_match("camera-person:suspected:person-1", 80, 60, 1, 5, 6_000));
    }

    #[test]
    fn match_gate_does_not_accumulate_hits_across_a_stale_gap() {
        let runtime = FaceMonitorRuntime::default();
        assert!(!runtime.accept_match("suspected:person-1", 80, 60, 2, 60, 1_000));
        assert!(!runtime.accept_match("suspected:person-1", 80, 60, 2, 60, 5_000));
        assert!(runtime.accept_match("suspected:person-1", 80, 60, 2, 60, 5_500));
    }

    #[test]
    fn match_gate_resets_when_candidate_is_missing_from_an_intermediate_frame() {
        let runtime = FaceMonitorRuntime::default();
        let key = "camera-person:suspected:person-1";
        assert!(!runtime.accept_match(key, 80, 60, 2, 60, 1_000));
        runtime.prepare_match_frame(&[]);
        assert!(!runtime.accept_match(key, 80, 60, 2, 60, 1_500));
        assert!(runtime.accept_match(key, 80, 60, 2, 60, 2_000));
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
    fn v4_osnet_manifest_uses_its_own_embedding_dimension() {
        let temp = tempfile::tempdir().unwrap();
        for (name, contents) in [
            ("face-detector.onnx", b"face-detector".as_slice()),
            ("face.onnx", b"face".as_slice()),
            ("person-detector.onnx", b"person-detector".as_slice()),
            ("osnet.onnx", b"osnet".as_slice()),
        ] {
            fs::write(temp.path().join(name), contents).unwrap();
        }
        let sha =
            |name: &str| hex::encode(Sha256::digest(fs::read(temp.path().join(name)).unwrap()));
        fs::write(temp.path().join("manifest.v4.json"), format!(r#"{{
          "schemaVersion":4,
          "package":{{"id":"test.osnet","version":"1.0.0"}},
          "profile":{{"id":"office-osnet-x025","version":"1.0.0","tier":"balanced","displayName":"OSNet","provider":"test","engine":"onnxruntime","supportedBackends":["cpu"]}},
          "pipeline":{{"personDetector":"person-detector","faceEngine":"face-recognizer","personReIdEngine":"person-reid","fusionPolicy":"quality-temporal-v1"}},
          "components":[
            {{"id":"face-detector","category":"face_detector","family":"yunet","file":"face-detector.onnx","sha256":"{}","adapterId":"builtin.face-detector.yunet.v1","engine":"onnxruntime"}},
            {{"id":"face-recognizer","category":"face_recognizer","family":"sface","file":"face.onnx","sha256":"{}","adapterId":"builtin.face-recognizer.sface.v1","engine":"onnxruntime","input":{{"colorOrder":"BGR","resizeMode":"aligned_112","normalization":"sface_127_5"}},"output":{{"embeddingDimension":128,"distanceMetric":"cosine"}}}},
            {{"id":"person-detector","category":"person_detector","family":"yolox","file":"person-detector.onnx","sha256":"{}","adapterId":"builtin.person-detector.yolox.v1","engine":"onnxruntime"}},
            {{"id":"person-reid","category":"person_reid","family":"osnet-x025","file":"osnet.onnx","sha256":"{}","adapterId":"builtin.person-reid.osnet.v1","engine":"onnxruntime","input":{{"colorOrder":"RGB","resizeMode":"256x128","normalization":"imagenet"}},"output":{{"embeddingDimension":512,"distanceMetric":"cosine"}}}}
          ]
        }}"#, sha("face-detector.onnx"), sha("face.onnx"), sha("person-detector.onnx"), sha("osnet.onnx"))).unwrap();

        let state = model_state_from_dir(temp.path()).expect("v4 OSNet state");
        assert_eq!(state.person_reid_spec.output_dimension, 512);
        assert_eq!(state.person_reid_spec.width, 128);
        assert_eq!(state.person_reid_spec.height, 256);
        assert_ne!(
            state.face_embedding_space_id.as_deref(),
            state.body_embedding_space_id.as_deref()
        );
        assert!(state
            .body_embedding_space_id
            .as_deref()
            .is_some_and(|space| space.contains("body.osnet-x025")));
    }

    #[test]
    fn bundled_onnx_detector_can_be_opened_by_onnx_runtime() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/object-models");
        let mut detector = ort::session::Session::builder()
            .unwrap()
            .commit_from_file(root.join("presence-detector.onnx"))
            .unwrap();
        assert_eq!(detector.inputs().len(), 1);
        assert!(detector.outputs().len() >= 6);
        let input = Array4::<f32>::zeros((1, 3, 640, 640));
        let outputs = detector
            .run(ort::inputs![
                ort::value::TensorRef::from_array_view(&input).unwrap()
            ])
            .unwrap();
        let (_, cls8) = outputs[0].try_extract_tensor::<f32>().unwrap();
        assert_eq!(cls8.len(), 6400);
    }

    #[test]
    fn bundled_v4_profile_passes_candidate_runtime_validation() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/object-models");
        FaceMonitorRuntime::validate_candidate_model_dir(&root)
            .expect("bundled V4 profile candidate runtime");
    }

    #[cfg(feature = "openvino-runtime")]
    #[test]
    fn packaged_omz_profile_compiles_all_components_on_cpu() {
        let resource_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources");
        let profile = std::env::var("LANCHAT_OMZ_SMOKE_PROFILE")
            .unwrap_or_else(|_| "office-omz-retail-0288".to_string());
        let model_dir = resource_root.join("model-profiles").join(profile);
        if !model_dir.is_dir() {
            // 官方 Profile 只在发布流水线下载，开发环境没有模型包时不把无关
            // 的本地单测变成网络依赖；CI 会在运行本测试前显式校验该目录。
            return;
        }
        crate::vision::openvino_runtime::configure_packaged_runtime(Some(&resource_root))
            .expect("packaged OpenVINO Runtime");
        FaceMonitorRuntime::validate_candidate_model_dir(&model_dir)
            .expect("OMZ Profile should compile on the packaged CPU runtime");
    }

    fn stride_layer<'a>(
        stride: f32,
        cls: &'a [f32],
        obj: &'a [f32],
        bbox: &'a [f32],
        kps: &'a [f32],
    ) -> (f32, &'a [f32], &'a [f32], &'a [f32], &'a [f32]) {
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
        let layers = [
            stride_layer(8.0, &cls, &obj, &bbox, &kps),
            stride_layer(16.0, &[], &[], &[], &[]),
            stride_layer(32.0, &[], &[], &[], &[]),
        ];
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
        let layers = [
            stride_layer(8.0, &cls, &obj, &bbox, &kps),
            stride_layer(16.0, &[], &[], &[], &[]),
            stride_layer(32.0, &[], &[], &[], &[]),
        ];
        assert!(decode_faces(layers, 8.0, 0.6).is_empty());
    }

    #[test]
    fn nms_faces_suppresses_overlapping_keeps_separated() {
        let face = |x1: f32, y1: f32, score: f32| DetectedFace {
            x1,
            y1,
            w: 10.0,
            h: 10.0,
            landmarks: [(0.0, 0.0); 5],
            score,
        };
        let overlapping = nms_faces(vec![face(0.0, 0.0, 0.9), face(1.0, 1.0, 0.8)], 0.35, 5);
        assert_eq!(overlapping.len(), 1);
        assert!((overlapping[0].score - 0.9).abs() < 1e-6);
        let separated = nms_faces(vec![face(0.0, 0.0, 0.7), face(50.0, 50.0, 0.9)], 0.35, 5);
        assert_eq!(separated.len(), 2);
        assert!((separated[0].score - 0.9).abs() < 1e-6);
    }

    #[test]
    fn nms_faces_respects_top_k() {
        let face = |offset: f32, score: f32| DetectedFace {
            x1: offset,
            y1: offset,
            w: 10.0,
            h: 10.0,
            landmarks: [(0.0, 0.0); 5],
            score,
        };
        let faces = (0..8)
            .map(|i| face(i as f32 * 40.0, 0.5 + i as f32 * 0.01))
            .collect();
        assert_eq!(nms_faces(faces, 0.35, 5).len(), 5);
    }

    #[test]
    fn align_face_112_outputs_fixed_size_for_non_degenerate_points() {
        let image = image::RgbImage::from_pixel(320, 240, image::Rgb([128, 96, 64]));
        // 把模板点放大 2 倍并平移，模拟原图坐标空间的关键点。
        let landmarks: [(f32, f32); 5] =
            ALIGN_TEMPLATE.map(|(x, y)| (x * 2.0 + 40.0, y * 2.0 + 30.0));
        let aligned = align_face_112(&image, landmarks);
        assert_eq!(aligned.width(), 112);
        assert_eq!(aligned.height(), 112);
        // 纯色图对齐后中心区域仍应接近原色。
        let center = aligned.get_pixel(56, 56);
        assert!((i32::from(center[0]) - 128).abs() <= 2);
    }

    #[test]
    fn align_face_112_falls_back_for_degenerate_points() {
        let image = image::RgbImage::from_pixel(64, 64, image::Rgb([10, 20, 30]));
        let landmarks = [(32.0, 32.0); 5];
        let aligned = align_face_112(&image, landmarks);
        assert_eq!((aligned.width(), aligned.height()), (112, 112));
    }

    #[test]
    fn cosine_similarity_matches_expected_angles() {
        let same = cosine_similarity(&[1.0, 2.0, 3.0], &[1.0, 2.0, 3.0]);
        assert!((same - 1.0).abs() < 1e-4);
        let orthogonal = cosine_similarity(&[1.0, 0.0], &[0.0, 1.0]);
        assert!(orthogonal.abs() < 1e-4);
        let opposite = cosine_similarity(&[1.0, 2.0], &[-1.0, -2.0]);
        assert!((opposite + 1.0).abs() < 1e-4);
        assert_eq!(cosine_similarity(&[0.0, 0.0], &[1.0, 1.0]), 0.0);
    }

    #[test]
    fn normalize_embedding_yields_unit_norm() {
        let mut embedding = [3.0_f32, -4.0, 0.0, 12.0];
        normalize_embedding(&mut embedding);
        let norm = embedding
            .iter()
            .map(|value| value * value)
            .sum::<f32>()
            .sqrt();
        assert!((norm - 1.0).abs() < 1e-6);
    }

    fn template(person_id: &str, embedding: [f32; 128]) -> PersonTemplate {
        PersonTemplate {
            person_id: person_id.to_string(),
            display_name: format!("人员{person_id}"),
            face_embeddings: vec![embedding.to_vec()],
            body_embeddings: vec![],
        }
    }

    fn matched(person_id: &str, confidence: u8, level: &str) -> FaceMatch {
        FaceMatch {
            person_id: person_id.to_string(),
            display_name: person_id.to_string(),
            confidence,
            recognition_level: level.to_string(),
            face_confidence: (level == "confirmed").then_some(confidence),
            body_confidence: (level == "suspected").then_some(confidence),
        }
    }

    #[test]
    fn same_track_face_and_body_are_emitted_as_one_confirmed_match() {
        let runtime = FaceMonitorRuntime::default();
        let fused = runtime
            .fuse_track_matches(
                "track-1",
                1,
                TrackEvidence {
                    face: Some((matched("alice", 91, "confirmed"), 0.95)),
                    body: Some((matched("alice", 84, "suspected"), 0.90)),
                },
            )
            .expect("fused match");

        assert_eq!(fused.person_id, "alice");
        assert_eq!(fused.recognition_level, "confirmed");
        assert_eq!(fused.face_confidence, Some(91));
        assert_eq!(fused.body_confidence, Some(84));
    }

    #[test]
    fn body_only_track_stays_suspected() {
        let runtime = FaceMonitorRuntime::default();
        let fused = runtime
            .fuse_track_matches(
                "track-1",
                1,
                TrackEvidence {
                    face: None,
                    body: Some((matched("alice", 84, "suspected"), 0.90)),
                },
            )
            .expect("body candidate");

        assert_eq!(fused.recognition_level, "suspected");
        assert_eq!(fused.face_confidence, None);
        assert_eq!(fused.body_confidence, Some(84));
    }

    #[test]
    fn best_match_picks_highest_similarity_and_maps_confidence() {
        let mut identical = [0.0_f32; 128];
        identical[0] = 1.0;
        let mut half = [0.0_f32; 128];
        half[0] = 0.5;
        half[1] = (0.75_f32).sqrt();
        let people = vec![template("a", identical), template("b", half)];
        let matched = best_face_match(&identical, &people).expect("match");
        assert_eq!(matched.person_id, "a");
        assert_eq!(matched.confidence, 100);
        // 与 b 的相似度为 0.5 → 置信度 50。
        let mut query = identical;
        query[0] = 1.0;
        let matched = best_face_match(&query, &[template("b", half)]).expect("match");
        assert_eq!(matched.confidence, 50);
    }

    #[test]
    fn weighted_similarity_uses_top_samples_without_a_single_outlier_dominating() {
        let mut query = [0.0_f32; 128];
        query[0] = 1.0;
        let mut close = [0.0_f32; 128];
        close[0] = 0.80;
        close[1] = 0.60;
        let mut medium = [0.0_f32; 128];
        medium[0] = 0.70;
        medium[1] = (0.51_f32).sqrt();
        let mut weak = [0.0_f32; 128];
        weak[0] = 0.20;
        weak[1] = (0.96_f32).sqrt();
        let score =
            weighted_person_similarity(&query, &[close.to_vec(), medium.to_vec(), weak.to_vec()])
                .expect("weighted score");
        assert!(
            score < 0.80 && score > 0.60,
            "score should blend top samples: {score}"
        );
    }

    #[test]
    fn best_match_clamps_negative_similarity_to_zero() {
        let mut positive = [0.0_f32; 128];
        positive[0] = 1.0;
        let mut opposite = [0.0_f32; 128];
        opposite[0] = -1.0;
        let matched = best_face_match(&positive, &[template("neg", opposite)]).expect("match");
        assert_eq!(matched.confidence, 0);
    }

    #[test]
    fn best_match_returns_none_without_people() {
        let embedding = [0.0_f32; 128];
        assert!(best_face_match(&embedding, &[]).is_none());
    }

    #[test]
    fn embedding_bytes_round_trip() {
        let mut embedding = [0.0_f32; 128];
        for (index, value) in embedding.iter_mut().enumerate() {
            *value = index as f32 * 0.01 - 0.5;
        }
        let restored = embedding_from_bytes(&embedding_bytes(&embedding)).expect("restore");
        assert_eq!(embedding, restored);
        assert!(embedding_from_bytes(&[0u8; 8]).is_err());
    }

    #[test]
    fn bundled_onnx_recognizer_outputs_128_dim_embedding() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/object-models");
        let mut recognizer = ort::session::Session::builder()
            .unwrap()
            .commit_from_file(root.join("face-recognizer.onnx"))
            .unwrap();
        let input = Array4::<f32>::zeros((1, 3, 112, 112));
        let outputs = recognizer
            .run(ort::inputs![
                ort::value::TensorRef::from_array_view(&input).unwrap()
            ])
            .unwrap();
        let (_, tensor) = outputs[0].try_extract_tensor::<f32>().unwrap();
        assert_eq!(tensor.len(), 128);
    }

    #[test]
    fn bundled_person_models_have_expected_shapes() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/object-models");
        let mut detector = ort::session::Session::builder()
            .unwrap()
            .commit_from_file(root.join("person-detector.onnx"))
            .unwrap();
        let detector_input = Array4::<f32>::zeros((1, 3, 640, 640));
        let outputs = detector
            .run(ort::inputs![ort::value::TensorRef::from_array_view(
                &detector_input
            )
            .unwrap()])
            .unwrap();
        let (_, tensor) = outputs[0].try_extract_tensor::<f32>().unwrap();
        assert_eq!(tensor.len(), 8400 * 85);

        let mut recognizer = ort::session::Session::builder()
            .unwrap()
            .commit_from_file(root.join("person-recognizer.onnx"))
            .unwrap();
        let recognizer_input = Array4::<f32>::zeros((1, 3, 256, 128));
        let outputs = recognizer
            .run(ort::inputs![ort::value::TensorRef::from_array_view(
                &recognizer_input
            )
            .unwrap()])
            .unwrap();
        let (_, tensor) = outputs[0].try_extract_tensor::<f32>().unwrap();
        assert_eq!(tensor.len(), PERSON_REID_DIM);
    }

    #[test]
    fn body_match_is_always_suspected() {
        let mut person = template("body", [0.0; 128]);
        let mut body = vec![0.0_f32; PERSON_REID_DIM];
        body[0] = 1.0;
        person.body_embeddings = vec![body.clone()];
        let matched = best_body_match(&body, &[person]).expect("body match");
        assert_eq!(matched.recognition_level, "suspected");
        assert_eq!(matched.body_confidence, Some(100));
        assert_eq!(matched.face_confidence, None);
    }

    #[test]
    fn body_match_rejects_ambiguous_people() {
        let mut query = vec![0.0_f32; PERSON_REID_DIM];
        query[0] = 1.0;
        let mut first = template("first", [0.0; 128]);
        let mut first_body = vec![0.0_f32; PERSON_REID_DIM];
        first_body[0] = 0.80;
        first_body[1] = 0.60;
        first.body_embeddings = vec![first_body];
        let mut second = template("second", [0.0; 128]);
        let mut second_body = vec![0.0_f32; PERSON_REID_DIM];
        second_body[0] = 0.77;
        second_body[1] = (1.0_f32 - 0.77_f32.powi(2)).sqrt();
        second.body_embeddings = vec![second_body];

        assert!(best_body_match(&query, &[first, second]).is_none());
    }

    #[test]
    fn reference_person_selection_rejects_tiny_or_implausible_crops() {
        let image = image::RgbImage::new(640, 480);
        let tiny = DetectedPerson {
            x: 20.0,
            y: 20.0,
            w: 20.0,
            h: 50.0,
            score: 0.95,
        };
        let too_wide = DetectedPerson {
            x: 80.0,
            y: 40.0,
            w: 300.0,
            h: 100.0,
            score: 0.90,
        };
        assert!(select_reference_person(&[tiny, too_wide], &image).is_none());

        let valid = DetectedPerson {
            x: 120.0,
            y: 30.0,
            w: 100.0,
            h: 360.0,
            score: 0.80,
        };
        assert!(select_reference_person(&[valid], &image).is_some());
    }

    #[test]
    fn faceless_photo_reports_missing_face_smoke() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources");
        let runtime = FaceMonitorRuntime::from_resource_dirs(Some(root));
        assert!(runtime.status().recognizer_ready);
        // 确定性伪随机噪声图，不可能包含人脸。
        let mut noise = image::RgbImage::new(240, 240);
        let mut seed = 12_345_u32;
        for pixel in noise.pixels_mut() {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            *pixel = image::Rgb([(seed >> 8) as u8, (seed >> 16) as u8, (seed >> 24) as u8]);
        }
        let mut png = Vec::new();
        noise
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let error = runtime.embedding_from_photo_bytes(&png).unwrap_err();
        assert!(error.contains("未检测到人脸"), "unexpected error: {error}");
    }
}
