//! V4 Manifest 到可执行视觉管线的纯解析层。
//!
//! 该层不持有模型 Session，因此 Profile 的清单校验、激活预检和 Worker 可以复用。

use super::manifest_v4::{ComponentDescriptor, VisionManifestV4};
use crate::vision::{
    backend::{adapter_descriptor, AdapterRole, RuntimeBackend},
    embedding::EmbeddingSpaceId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineComponent {
    pub id: String,
    pub adapter_id: String,
    pub model_file: String,
    pub backend: RuntimeBackend,
    pub embedding_space_id: EmbeddingSpaceId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedVisionPipeline {
    pub profile_id: String,
    pub profile_version: String,
    pub person_detector: PipelineComponent,
    pub face_recognizer: PipelineComponent,
    pub person_reid: PipelineComponent,
}

pub fn resolve_pipeline(manifest: &VisionManifestV4) -> Result<ResolvedVisionPipeline, String> {
    let resolve = |id: &str, role: AdapterRole| -> Result<PipelineComponent, String> {
        let component = manifest
            .components
            .iter()
            .find(|component| component.id == id)
            .ok_or_else(|| "VISION_MANIFEST_PIPELINE_INVALID".to_string())?;
        resolve_component(component, role)
    };
    Ok(ResolvedVisionPipeline {
        profile_id: manifest.profile.id.clone(),
        profile_version: manifest.profile.version.clone(),
        person_detector: resolve(
            &manifest.pipeline.person_detector,
            AdapterRole::PersonDetector,
        )?,
        face_recognizer: resolve(&manifest.pipeline.face_engine, AdapterRole::FaceRecognizer)?,
        person_reid: resolve(
            &manifest.pipeline.person_re_id_engine,
            AdapterRole::PersonReId,
        )?,
    })
}

fn resolve_component(
    component: &ComponentDescriptor,
    expected_role: AdapterRole,
) -> Result<PipelineComponent, String> {
    let descriptor = adapter_descriptor(&component.adapter_id)
        .ok_or_else(|| "VISION_MANIFEST_ADAPTER_UNSUPPORTED".to_string())?;
    if descriptor.role != expected_role || descriptor.backend.manifest_name() != component.engine {
        return Err("VISION_MANIFEST_PIPELINE_ADAPTER_MISMATCH".to_string());
    }
    let embedding_space_id = EmbeddingSpaceId::new(format!(
        "{}.{}",
        descriptor.embedding_space_namespace,
        &component.sha256[..16]
    ))?;
    Ok(PipelineComponent {
        id: component.id.clone(),
        adapter_id: component.adapter_id.clone(),
        model_file: component.file.clone(),
        backend: descriptor.backend,
        embedding_space_id,
    })
}
