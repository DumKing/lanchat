//! Manifest V3 的解析与可重复校验。

use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashSet;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VisionManifestV3 {
    pub schema_version: u16,
    pub package: PackageDescriptor,
    pub profile: ProfileDescriptor,
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
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentDescriptor {
    pub id: String,
    pub category: String,
    pub file: String,
    pub sha256: String,
    pub adapter_id: String,
    pub input: Option<ComponentInput>,
    pub output: Option<ComponentOutput>,
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

/// 当前只允许 Manifest V3；旧清单由旧 face_monitor 兼容读取，不能伪装成 V3。
pub fn validate_manifest(manifest: &VisionManifestV3) -> Result<(), String> {
    if manifest.schema_version != 3 {
        return Err("VISION_MANIFEST_SCHEMA_UNSUPPORTED".to_string());
    }
    let package_version = Version::parse(&manifest.package.version)
        .map_err(|_| "VISION_MANIFEST_PACKAGE_VERSION_INVALID".to_string())?;
    let profile_version = Version::parse(&manifest.profile.version)
        .map_err(|_| "VISION_MANIFEST_PROFILE_VERSION_INVALID".to_string())?;
    if package_version != profile_version {
        return Err("VISION_MANIFEST_PROFILE_VERSION_MISMATCH".to_string());
    }
    if manifest.package.id.trim().is_empty() || manifest.profile.id.trim().is_empty() {
        return Err("VISION_MANIFEST_ID_EMPTY".to_string());
    }
    if !matches!(manifest.profile.tier.as_str(), "low_resource" | "balanced" | "experimental") {
        return Err("VISION_MANIFEST_PROFILE_TIER_INVALID".to_string());
    }

    let mut ids = HashSet::new();
    for component in &manifest.components {
        if component.id.trim().is_empty()
            || component.file.trim().is_empty()
            || component.adapter_id.trim().is_empty()
            || component.sha256.len() != 64
            || !component.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
            || !ids.insert(component.id.as_str())
        {
            return Err("VISION_MANIFEST_COMPONENT_INVALID".to_string());
        }
        if component.category.ends_with("recognizer") || component.category == "person_reid" {
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
    }
    Ok(())
}

/// 由所有会影响向量数值语义的字段计算，不能信任 Manifest 自报 ID。
pub fn embedding_space_id(manifest: &VisionManifestV3, component_id: &str) -> Result<String, String> {
    validate_manifest(manifest)?;
    let component = manifest
        .components
        .iter()
        .find(|component| component.id == component_id)
        .ok_or_else(|| "VISION_MANIFEST_COMPONENT_NOT_FOUND".to_string())?;
    let input = component
        .input
        .as_ref()
        .ok_or_else(|| "VISION_MANIFEST_COMPONENT_INPUT_MISSING".to_string())?;
    let output = component
        .output
        .as_ref()
        .ok_or_else(|| "VISION_MANIFEST_COMPONENT_OUTPUT_MISSING".to_string())?;
    let canonical = format!(
        "modality={};sha256={};adapter={};color={};resize={};normalization={};dimension={};metric={}",
        component.category,
        component.sha256.to_ascii_lowercase(),
        component.adapter_id,
        input.color_order,
        input.resize_mode,
        input.normalization,
        output.embedding_dimension,
        output.distance_metric,
    );
    Ok(hex::encode(Sha256::digest(canonical.as_bytes())))
}
