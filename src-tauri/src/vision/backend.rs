//! 模型适配器注册表。
//!
//! 清单只能引用这里明确注册的适配器。每个适配器绑定运行时、输入输出语义及
//! Embedding Space 命名空间，避免把不同模型的向量误当成同类数据。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeBackend {
    OnnxRuntime,
    OpenVino,
}

impl RuntimeBackend {
    pub fn from_manifest_name(value: &str) -> Option<Self> {
        match value {
            "onnxruntime" => Some(Self::OnnxRuntime),
            "openvino" => Some(Self::OpenVino),
            _ => None,
        }
    }

    pub fn manifest_name(self) -> &'static str {
        match self {
            Self::OnnxRuntime => "onnxruntime",
            Self::OpenVino => "openvino",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterRole {
    FaceDetector,
    FaceRecognizer,
    PersonDetector,
    PersonReId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdapterDescriptor {
    pub id: &'static str,
    pub role: AdapterRole,
    pub family: &'static str,
    pub backend: RuntimeBackend,
    /// 此名称参与 EmbeddingSpaceId 构建；同名空间才允许比较向量。
    pub embedding_space_namespace: &'static str,
}

const ADAPTERS: &[AdapterDescriptor] = &[
    AdapterDescriptor {
        id: "builtin.face-detector.yunet.v1",
        role: AdapterRole::FaceDetector,
        family: "yunet",
        backend: RuntimeBackend::OnnxRuntime,
        embedding_space_namespace: "detector.yunet.v1",
    },
    AdapterDescriptor {
        id: "builtin.face-recognizer.sface.v1",
        role: AdapterRole::FaceRecognizer,
        family: "sface",
        backend: RuntimeBackend::OnnxRuntime,
        embedding_space_namespace: "face.sface.v1",
    },
    AdapterDescriptor {
        id: "builtin.face-recognizer.arcface.v1",
        role: AdapterRole::FaceRecognizer,
        family: "arcface",
        backend: RuntimeBackend::OnnxRuntime,
        embedding_space_namespace: "face.arcface.v1",
    },
    AdapterDescriptor {
        id: "builtin.face-recognizer.omz.v1",
        role: AdapterRole::FaceRecognizer,
        family: "omz-face-reid",
        backend: RuntimeBackend::OpenVino,
        embedding_space_namespace: "face.omz-reid.v1",
    },
    AdapterDescriptor {
        id: "builtin.face-detector.omz.v1",
        role: AdapterRole::FaceDetector,
        family: "omz-face-detector",
        backend: RuntimeBackend::OpenVino,
        embedding_space_namespace: "detector.omz-face.v1",
    },
    AdapterDescriptor {
        id: "builtin.person-detector.yolox.v1",
        role: AdapterRole::PersonDetector,
        family: "yolox",
        backend: RuntimeBackend::OnnxRuntime,
        embedding_space_namespace: "detector.yolox.v1",
    },
    AdapterDescriptor {
        id: "builtin.person-detector.omz.v1",
        role: AdapterRole::PersonDetector,
        family: "omz-person-detection",
        backend: RuntimeBackend::OpenVino,
        embedding_space_namespace: "detector.omz-person.v1",
    },
    AdapterDescriptor {
        id: "builtin.person-reid.youtu.v1",
        role: AdapterRole::PersonReId,
        family: "youtureid",
        backend: RuntimeBackend::OnnxRuntime,
        embedding_space_namespace: "body.youtu.v1",
    },
    AdapterDescriptor {
        id: "builtin.person-reid.osnet.v1",
        role: AdapterRole::PersonReId,
        family: "osnet-x025",
        backend: RuntimeBackend::OnnxRuntime,
        embedding_space_namespace: "body.osnet-x025.v1",
    },
    AdapterDescriptor {
        id: "builtin.person-reid.omz.v1",
        role: AdapterRole::PersonReId,
        family: "omz-person-reid-0288",
        backend: RuntimeBackend::OpenVino,
        embedding_space_namespace: "body.omz-retail-0288.v1",
    },
    AdapterDescriptor {
        id: "builtin.person-reid.omz.0286.v1",
        role: AdapterRole::PersonReId,
        family: "omz-person-reid-0286",
        backend: RuntimeBackend::OpenVino,
        embedding_space_namespace: "body.omz-retail-0286.v1",
    },
    AdapterDescriptor {
        id: "builtin.person-reid.fastreid.v1",
        role: AdapterRole::PersonReId,
        family: "fastreid",
        backend: RuntimeBackend::OnnxRuntime,
        embedding_space_namespace: "body.fastreid.v1",
    },
];

pub fn adapter_descriptor(adapter_id: &str) -> Option<&'static AdapterDescriptor> {
    ADAPTERS.iter().find(|adapter| adapter.id == adapter_id)
}

pub fn is_registered_adapter(adapter_id: &str) -> bool {
    adapter_descriptor(adapter_id).is_some()
}

/// ONNX Runtime 随应用现有依赖可用；Windows 正式包会随资源目录携带
/// OpenVINO Runtime。运行时不可用时禁止激活 OpenVINO Profile，不能静默回退
/// 到 ONNX 权重。
pub fn backend_activation_reason(backend: RuntimeBackend) -> Option<&'static str> {
    match backend {
        RuntimeBackend::OnnxRuntime => None,
        RuntimeBackend::OpenVino => openvino_activation_reason(),
    }
}

pub fn backend_activation_reason_by_name(engine: &str) -> Option<&'static str> {
    RuntimeBackend::from_manifest_name(engine).and_then(backend_activation_reason)
}

#[cfg(feature = "openvino-runtime")]
fn openvino_activation_reason() -> Option<&'static str> {
    openvino::Core::new()
        .map(|_| ())
        .map_err(|_| "VISION_OPENVINO_RUNTIME_REQUIRED")
        .err()
}

#[cfg(not(feature = "openvino-runtime"))]
fn openvino_activation_reason() -> Option<&'static str> {
    Some("VISION_OPENVINO_RUNTIME_REQUIRED")
}
