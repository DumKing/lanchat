//! Manifest V4 明确描述模型组合，而不是把同一组权重包装成多个性能档位。

use semver::Version;
use serde::Deserialize;
use std::{
    collections::HashSet,
    path::{Component, Path},
};

use crate::vision::backend::{adapter_descriptor, is_registered_adapter as backend_has_adapter};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VisionManifestV4 {
    pub schema_version: u16,
    pub package: PackageDescriptor,
    pub profile: ProfileDescriptor,
    pub pipeline: PipelineDescriptor,
    pub components: Vec<ComponentDescriptor>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageDescriptor {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileDescriptor {
    pub id: String,
    pub version: String,
    pub tier: String,
    pub display_name: String,
    pub provider: String,
    pub engine: String,
    pub supported_backends: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineDescriptor {
    pub person_detector: String,
    pub face_engine: String,
    pub person_re_id_engine: String,
    pub fusion_policy: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentDescriptor {
    pub id: String,
    pub category: String,
    pub family: String,
    pub file: String,
    pub sha256: String,
    pub adapter_id: String,
    pub engine: String,
    /// OpenVINO IR 的 BIN、模型校准数据等与主模型共同安装并校验的文件。
    #[serde(default)]
    pub auxiliary_files: Vec<AuxiliaryFileDescriptor>,
    pub input: Option<ComponentInput>,
    pub output: Option<ComponentOutput>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuxiliaryFileDescriptor {
    pub file: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentInput {
    pub color_order: String,
    pub resize_mode: String,
    pub normalization: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentOutput {
    pub embedding_dimension: u16,
    pub distance_metric: String,
}

pub fn validate_manifest_v4(manifest: &VisionManifestV4) -> Result<(), String> {
    if manifest.schema_version != 4 {
        return Err("VISION_MANIFEST_SCHEMA_UNSUPPORTED".to_string());
    }
    let package_version = Version::parse(&manifest.package.version)
        .map_err(|_| "VISION_MANIFEST_PACKAGE_VERSION_INVALID".to_string())?;
    let profile_version = Version::parse(&manifest.profile.version)
        .map_err(|_| "VISION_MANIFEST_PROFILE_VERSION_INVALID".to_string())?;
    if package_version != profile_version {
        return Err("VISION_MANIFEST_PROFILE_VERSION_MISMATCH".to_string());
    }
    if manifest.package.id.trim().is_empty()
        || manifest.profile.id.trim().is_empty()
        || manifest.profile.display_name.trim().is_empty()
        || manifest.profile.provider.trim().is_empty()
    {
        return Err("VISION_MANIFEST_ID_EMPTY".to_string());
    }
    if !matches!(
        manifest.profile.tier.as_str(),
        "low_resource" | "balanced" | "experimental"
    ) {
        return Err("VISION_MANIFEST_PROFILE_TIER_INVALID".to_string());
    }
    if !matches!(manifest.profile.engine.as_str(), "onnxruntime" | "openvino")
        || manifest.profile.supported_backends.is_empty()
        || manifest
            .profile
            .supported_backends
            .iter()
            .any(|backend| !matches!(backend.as_str(), "cpu" | "directml" | "openvino_cpu"))
    {
        return Err("VISION_MANIFEST_ENGINE_UNSUPPORTED".to_string());
    }

    let mut ids = HashSet::new();
    let mut files = HashSet::new();
    for component in &manifest.components {
        validate_component(component, &manifest.profile.engine, &mut ids, &mut files)?;
    }
    validate_pipeline(&manifest.pipeline, &manifest.components)
}

fn validate_component(
    component: &ComponentDescriptor,
    profile_engine: &str,
    ids: &mut HashSet<String>,
    files: &mut HashSet<String>,
) -> Result<(), String> {
    if component.id.trim().is_empty()
        || component.category.trim().is_empty()
        || component.family.trim().is_empty()
        || component.adapter_id.trim().is_empty()
        || component.engine != profile_engine
        || !ids.insert(component.id.clone())
    {
        return Err("VISION_MANIFEST_COMPONENT_INVALID".to_string());
    }
    validate_asset_descriptor(&component.file, &component.sha256, files)?;
    for auxiliary in &component.auxiliary_files {
        validate_asset_descriptor(&auxiliary.file, &auxiliary.sha256, files)?;
    }
    if component.engine == "openvino" {
        let expected_bin = Path::new(&component.file).with_extension("bin");
        if Path::new(&component.file)
            .extension()
            .and_then(|value| value.to_str())
            != Some("xml")
            || !component
                .auxiliary_files
                .iter()
                .any(|asset| Path::new(&asset.file) == expected_bin)
        {
            return Err("VISION_MANIFEST_OPENVINO_IR_PAIR_INVALID".to_string());
        }
    }
    let Some(adapter) = adapter_descriptor(&component.adapter_id) else {
        return Err("VISION_MANIFEST_ADAPTER_UNSUPPORTED".to_string());
    };
    if adapter.backend.manifest_name() != profile_engine {
        return Err("VISION_MANIFEST_ADAPTER_ENGINE_MISMATCH".to_string());
    }
    if component.category == "face_recognizer" || component.category == "person_reid" {
        let input = component
            .input
            .as_ref()
            .ok_or_else(|| "VISION_MANIFEST_COMPONENT_INPUT_MISSING".to_string())?;
        let output = component
            .output
            .as_ref()
            .ok_or_else(|| "VISION_MANIFEST_COMPONENT_OUTPUT_MISSING".to_string())?;
        if !matches!(input.color_order.as_str(), "RGB" | "BGR")
            || input.resize_mode.trim().is_empty()
            || input.normalization.trim().is_empty()
            || output.embedding_dimension == 0
            || output.distance_metric.trim().is_empty()
        {
            return Err("VISION_MANIFEST_COMPONENT_SEMANTICS_INVALID".to_string());
        }
    }
    Ok(())
}

fn validate_asset_descriptor(
    file: &str,
    sha256: &str,
    files: &mut HashSet<String>,
) -> Result<(), String> {
    let path = Path::new(file.trim());
    if file.trim().is_empty()
        || path.is_absolute()
        || path.components().any(|part| {
            matches!(
                part,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
        || sha256.len() != 64
        || !sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
        || !files.insert(file.to_string())
    {
        return Err("VISION_MANIFEST_COMPONENT_INVALID".to_string());
    }
    Ok(())
}

fn validate_pipeline(
    pipeline: &PipelineDescriptor,
    components: &[ComponentDescriptor],
) -> Result<(), String> {
    let expected = [
        (&pipeline.person_detector, "person_detector"),
        (&pipeline.face_engine, "face_recognizer"),
        (&pipeline.person_re_id_engine, "person_reid"),
    ];
    for (id, category) in expected {
        if components
            .iter()
            .find(|component| component.id == *id)
            .is_none_or(|component| component.category != category)
        {
            return Err("VISION_MANIFEST_PIPELINE_INVALID".to_string());
        }
    }
    if pipeline.fusion_policy != "quality-temporal-v1" {
        return Err("VISION_MANIFEST_FUSION_POLICY_UNSUPPORTED".to_string());
    }
    Ok(())
}

pub fn is_registered_adapter(adapter_id: &str) -> bool {
    backend_has_adapter(adapter_id)
}
