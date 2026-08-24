//! 官方视觉模型目录与数据包安装。
//!
//! 模型包只能包含清单和 ONNX 数据文件：目录必须经过 Ed25519 签名，包本身
//! 还要经过 SHA-256、Manifest V3 与路径安全校验。解压始终发生在 staging，
//! 成功后才原子替换安装目录。

use super::{
    manifest::{validate_manifest, VisionManifestV3},
    profile::manifest_v4::{validate_manifest_v4, VisionManifestV4},
    registry::{validate_package_entries, verify_catalog, SignedCatalog, TrustedKeyRing},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Cursor,
    path::{Path, PathBuf},
};
use uuid::Uuid;

pub const OFFICIAL_CATALOG_URL: &str =
    "https://github.com/DumKing/lanchat/releases/latest/download/vision-catalog.json";
/// 发布目录的根公钥。发布模型时必须由配套的离线私钥签名 catalog 字段。
const CATALOG_ROOT_KEY_ID: &str = "lanchat-vision-root-v1";
const CATALOG_ROOT_PUBLIC_KEY_HEX: &str =
    "533dbbf94d409b4e8ecb0453e546cc6bd9ced8f88d3f4c0c4b8ca8ac7b796925";
const MAX_MODEL_PACKAGE_BYTES: usize = 1024 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VisionCatalog {
    pub schema_version: u16,
    pub profiles: Vec<VisionCatalogProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VisionCatalogProfile {
    pub profile_id: String,
    pub profile_version: String,
    pub display_name: String,
    pub tier: String,
    /// V2 目录强制声明实际模型组合，防止同一权重被伪装为多个 Profile。
    #[serde(default)]
    pub model_stack: Option<VisionModelStack>,
    pub download_url: String,
    pub package_sha256: String,
    #[serde(default)]
    pub package_size_bytes: u64,
    /// 档位启用时应用的本机建议参数。超管锁定策略始终优先于这些建议值。
    #[serde(default)]
    pub recommended_settings: Option<VisionProfileRecommendedSettings>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VisionModelStack {
    pub inference_engine: String,
    pub person_detector: String,
    pub face_engine: String,
    pub person_re_id_engine: String,
    pub provider: String,
    pub license: String,
}

impl VisionModelStack {
    fn validate(&self) -> bool {
        matches!(self.inference_engine.as_str(), "onnxruntime" | "openvino")
            && matches!(
                self.person_detector.as_str(),
                "yolox" | "omz-person-detection"
            )
            && matches!(
                self.face_engine.as_str(),
                "sface" | "arcface" | "omz-face-reid"
            )
            && matches!(
                self.person_re_id_engine.as_str(),
                "youtureid"
                    | "osnet-x025"
                    | "omz-person-reid-0288"
                    | "omz-person-reid-0286"
                    | "fastreid"
            )
            && !self.provider.trim().is_empty()
            && !self.license.trim().is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VisionProfileRecommendedSettings {
    pub sample_fps: u8,
    pub face_min_confidence: u8,
    pub body_min_confidence: u8,
    pub consecutive_hits: u8,
}

#[derive(Debug, Clone)]
pub struct InstalledVisionPackage {
    pub profile: VisionCatalogProfile,
    pub install_dir: PathBuf,
    pub manifest_json: String,
    pub bytes: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct PackageManifest {
    pub profile_id: String,
    pub profile_version: String,
    pub components: Vec<PackageComponentAsset>,
    requires_legacy_manifest: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct PackageComponentAsset {
    pub adapter_id: String,
    pub file: String,
    pub sha256: String,
    pub engine: Option<String>,
    pub is_primary_model: bool,
}

/// V3 只为现有基线兼容保留；新模型包应使用 V4 来声明真正的模型组合。
pub(crate) fn parse_package_manifest(manifest_json: &str) -> Result<PackageManifest, String> {
    let value: serde_json::Value = serde_json::from_str(manifest_json)
        .map_err(|_| "VISION_PACKAGE_MANIFEST_INVALID".to_string())?;
    match value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
    {
        Some(3) => {
            let manifest: VisionManifestV3 = serde_json::from_value(value)
                .map_err(|_| "VISION_PACKAGE_MANIFEST_INVALID".to_string())?;
            validate_manifest(&manifest)?;
            Ok(PackageManifest {
                profile_id: manifest.profile.id,
                profile_version: manifest.profile.version,
                components: manifest
                    .components
                    .into_iter()
                    .map(|component| PackageComponentAsset {
                        adapter_id: component.adapter_id,
                        file: component.file,
                        sha256: component.sha256,
                        engine: None,
                        is_primary_model: true,
                    })
                    .collect(),
                requires_legacy_manifest: true,
            })
        }
        Some(4) => {
            let manifest: VisionManifestV4 = serde_json::from_value(value)
                .map_err(|_| "VISION_PACKAGE_MANIFEST_INVALID".to_string())?;
            validate_manifest_v4(&manifest)?;
            let components = manifest
                .components
                .iter()
                .flat_map(|component| {
                    std::iter::once(PackageComponentAsset {
                        adapter_id: component.adapter_id.clone(),
                        file: component.file.clone(),
                        sha256: component.sha256.clone(),
                        engine: Some(component.engine.clone()),
                        is_primary_model: true,
                    })
                    .chain(component.auxiliary_files.iter().map(|asset| {
                        PackageComponentAsset {
                            adapter_id: String::new(),
                            file: asset.file.clone(),
                            sha256: asset.sha256.clone(),
                            engine: Some(component.engine.clone()),
                            is_primary_model: false,
                        }
                    }))
                })
                .collect();
            Ok(PackageManifest {
                profile_id: manifest.profile.id,
                profile_version: manifest.profile.version,
                components,
                requires_legacy_manifest: false,
            })
        }
        _ => Err("VISION_MANIFEST_SCHEMA_UNSUPPORTED".to_string()),
    }
}

/// 官方目录的模型组合描述必须与 ZIP 内实际 V4 管线一致。
/// 这能防止模型资产被错误标注为另一个 Profile，而不仅仅依赖显示名称。
pub(crate) fn validate_catalog_model_stack(
    manifest_json: &str,
    stack: &VisionModelStack,
) -> Result<(), String> {
    let value: serde_json::Value = serde_json::from_str(manifest_json)
        .map_err(|_| "VISION_PACKAGE_MODEL_STACK_MISMATCH".to_string())?;
    if value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        != Some(4)
    {
        return Err("VISION_PACKAGE_MODEL_STACK_MISMATCH".to_string());
    }
    let manifest: VisionManifestV4 = serde_json::from_value(value)
        .map_err(|_| "VISION_PACKAGE_MODEL_STACK_MISMATCH".to_string())?;
    let component = |id: &str| {
        manifest
            .components
            .iter()
            .find(|component| component.id == id)
            .ok_or_else(|| "VISION_PACKAGE_MODEL_STACK_MISMATCH".to_string())
    };
    let face = component(&manifest.pipeline.face_engine)?;
    let person_detector = component(&manifest.pipeline.person_detector)?;
    let person_reid = component(&manifest.pipeline.person_re_id_engine)?;
    if manifest.profile.engine != stack.inference_engine
        || face.family != stack.face_engine
        || person_detector.family != stack.person_detector
        || person_reid.family != stack.person_re_id_engine
    {
        return Err("VISION_PACKAGE_MODEL_STACK_MISMATCH".to_string());
    }
    Ok(())
}

pub fn parse_signed_catalog(bytes: &[u8]) -> Result<VisionCatalog, String> {
    let key_ring = TrustedKeyRing::new(CATALOG_ROOT_KEY_ID, CATALOG_ROOT_PUBLIC_KEY_HEX);
    parse_signed_catalog_with_key_ring(bytes, &key_ring)
}

pub(crate) fn parse_signed_catalog_with_key_ring(
    bytes: &[u8],
    key_ring: &TrustedKeyRing,
) -> Result<VisionCatalog, String> {
    let signed: SignedCatalog =
        serde_json::from_slice(bytes).map_err(|_| "VISION_CATALOG_INVALID".to_string())?;
    verify_catalog(&signed, &key_ring)?;
    let catalog: VisionCatalog = serde_json::from_value(signed.catalog)
        .map_err(|_| "VISION_CATALOG_PAYLOAD_INVALID".to_string())?;
    if !matches!(catalog.schema_version, 1 | 2) || catalog.profiles.is_empty() {
        return Err("VISION_CATALOG_SCHEMA_UNSUPPORTED".to_string());
    }
    for profile in &catalog.profiles {
        if profile.profile_id.trim().is_empty()
            || profile.profile_version.trim().is_empty()
            || profile.display_name.trim().is_empty()
            || profile.download_url.trim().is_empty()
            || !matches!(
                profile.tier.as_str(),
                "low_resource" | "balanced" | "experimental"
            )
            || profile.package_sha256.len() != 64
            || !profile
                .package_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err("VISION_CATALOG_PROFILE_INVALID".to_string());
        }
        if catalog.schema_version >= 2
            && profile
                .model_stack
                .as_ref()
                .is_none_or(|stack| !stack.validate())
        {
            return Err("VISION_CATALOG_MODEL_STACK_INVALID".to_string());
        }
        if profile
            .model_stack
            .as_ref()
            .is_some_and(|stack| !stack.validate())
        {
            return Err("VISION_CATALOG_MODEL_STACK_INVALID".to_string());
        }
        if let Some(settings) = &profile.recommended_settings {
            let valid = (1..=5).contains(&settings.sample_fps)
                && (1..=100).contains(&settings.face_min_confidence)
                && (1..=100).contains(&settings.body_min_confidence)
                && (1..=20).contains(&settings.consecutive_hits);
            if !valid {
                return Err("VISION_CATALOG_RECOMMENDED_SETTINGS_INVALID".to_string());
            }
        }
    }
    Ok(catalog)
}

pub async fn fetch_official_catalog(client: &reqwest::Client) -> Result<VisionCatalog, String> {
    let response = crate::authorized_update_request(client, OFFICIAL_CATALOG_URL)
        .send()
        .await
        .map_err(|error| format!("VISION_CATALOG_DOWNLOAD_FAILED:{error}"))?
        .error_for_status()
        .map_err(|error| format!("VISION_CATALOG_DOWNLOAD_FAILED:{error}"))?;
    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("VISION_CATALOG_DOWNLOAD_FAILED:{error}"))?;
    parse_signed_catalog(&bytes)
}

pub async fn download_and_install(
    client: &reqwest::Client,
    profile: &VisionCatalogProfile,
    model_root: &Path,
) -> Result<InstalledVisionPackage, String> {
    if !crate::is_allowed_remote_update_url(&profile.download_url) {
        return Err("VISION_PACKAGE_URL_INVALID".to_string());
    }
    let response = crate::authorized_update_request(client, &profile.download_url)
        .send()
        .await
        .map_err(|error| format!("VISION_PACKAGE_DOWNLOAD_FAILED:{error}"))?
        .error_for_status()
        .map_err(|error| format!("VISION_PACKAGE_DOWNLOAD_FAILED:{error}"))?;
    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("VISION_PACKAGE_DOWNLOAD_FAILED:{error}"))?;
    if bytes.len() > MAX_MODEL_PACKAGE_BYTES {
        return Err("VISION_PACKAGE_TOO_LARGE".to_string());
    }
    let package_bytes = bytes.len() as u64;
    let actual_hash = hex::encode(Sha256::digest(&bytes));
    if !actual_hash.eq_ignore_ascii_case(&profile.package_sha256) {
        return Err("VISION_PACKAGE_HASH_MISMATCH".to_string());
    }

    let profile_root = model_root.join(safe_segment(&profile.profile_id)?);
    fs::create_dir_all(&profile_root)
        .map_err(|error| format!("VISION_PACKAGE_INSTALL_FAILED:{error}"))?;
    let staging = profile_root.join(format!(".staging-{}", Uuid::new_v4()));
    let result = (|| {
        fs::create_dir_all(&staging)
            .map_err(|error| format!("VISION_PACKAGE_INSTALL_FAILED:{error}"))?;
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
            .map_err(|_| "VISION_PACKAGE_ARCHIVE_INVALID".to_string())?;
        let entries = (0..archive.len())
            .filter_map(|index| {
                archive
                    .by_index(index)
                    .ok()
                    .and_then(|entry| (!entry.is_dir()).then(|| entry.name().to_string()))
            })
            .collect::<Vec<_>>();
        validate_package_entries(&entries)?;
        for index in 0..archive.len() {
            let mut entry = archive
                .by_index(index)
                .map_err(|_| "VISION_PACKAGE_ARCHIVE_INVALID".to_string())?;
            if entry.is_dir() {
                continue;
            }
            let relative = Path::new(entry.name());
            let target = staging.join(relative);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| format!("VISION_PACKAGE_INSTALL_FAILED:{error}"))?;
            }
            let mut file = fs::File::create(&target)
                .map_err(|error| format!("VISION_PACKAGE_INSTALL_FAILED:{error}"))?;
            std::io::copy(&mut entry, &mut file)
                .map_err(|error| format!("VISION_PACKAGE_INSTALL_FAILED:{error}"))?;
        }
        let model_dir = staging.join("object-models");
        let (manifest_json, manifest) = read_and_validate_package_manifest(&model_dir)?;
        if manifest.profile_id != profile.profile_id
            || manifest.profile_version != profile.profile_version
        {
            return Err("VISION_PACKAGE_PROFILE_MISMATCH".to_string());
        }
        if let Some(stack) = &profile.model_stack {
            validate_catalog_model_stack(&manifest_json, stack)?;
        }
        let destination = profile_root.join(safe_segment(&profile.profile_version)?);
        let backup = profile_root.join(format!(".backup-{}", Uuid::new_v4()));
        if destination.exists() {
            fs::rename(&destination, &backup)
                .map_err(|error| format!("VISION_PACKAGE_INSTALL_FAILED:{error}"))?;
        }
        if let Err(error) = fs::rename(&staging, &destination) {
            if backup.exists() {
                let _ = fs::rename(&backup, &destination);
            }
            return Err(format!("VISION_PACKAGE_INSTALL_FAILED:{error}"));
        }
        if backup.exists() {
            let _ = fs::remove_dir_all(&backup);
        }
        Ok(InstalledVisionPackage {
            profile: profile.clone(),
            install_dir: destination,
            manifest_json,
            bytes: package_bytes,
        })
    })();
    if staging.exists() {
        let _ = fs::remove_dir_all(&staging);
    }
    result
}

/// Profile 激活前的同一套预检：已安装目录不会因数据库状态而绕过清单校验。
pub(crate) fn validate_installed_package(install_dir: &Path) -> Result<PackageManifest, String> {
    let model_dir = install_dir.join("object-models");
    read_and_validate_package_manifest(&model_dir).map(|(_, manifest)| manifest)
}

fn read_and_validate_package_manifest(
    model_dir: &Path,
) -> Result<(String, PackageManifest), String> {
    let manifest_path_v4 = model_dir.join("manifest.v4.json");
    let manifest_path_v3 = model_dir.join("manifest.v3.json");
    let manifest_json = fs::read_to_string(&manifest_path_v4)
        .or_else(|_| fs::read_to_string(&manifest_path_v3))
        .map_err(|_| "VISION_PACKAGE_MANIFEST_MISSING".to_string())?;
    let manifest = parse_package_manifest(&manifest_json)?;
    if manifest.requires_legacy_manifest && !model_dir.join("manifest.json").is_file() {
        return Err("VISION_PACKAGE_LEGACY_MANIFEST_MISSING".to_string());
    }
    verify_component_assets(model_dir, &manifest.components)?;
    Ok((manifest_json, manifest))
}

fn safe_segment(value: &str) -> Result<&str, String> {
    let value = value.trim();
    if value.is_empty() || value.contains(['/', '\\']) || value == "." || value == ".." {
        Err("VISION_PACKAGE_PATH_INVALID".to_string())
    } else {
        Ok(value)
    }
}

fn verify_component_assets(
    model_dir: &Path,
    components: &[PackageComponentAsset],
) -> Result<(), String> {
    for component in components {
        let relative = Path::new(component.file.trim());
        if relative.is_absolute()
            || relative
                .components()
                .any(|part| matches!(part, std::path::Component::ParentDir))
        {
            return Err("VISION_PACKAGE_COMPONENT_PATH_INVALID".to_string());
        }
        let bytes = fs::read(model_dir.join(relative))
            .map_err(|_| "VISION_PACKAGE_COMPONENT_MISSING".to_string())?;
        if (component.is_primary_model && component.adapter_id.trim().is_empty())
            || !hex::encode(Sha256::digest(bytes)).eq_ignore_ascii_case(&component.sha256)
        {
            return Err("VISION_PACKAGE_COMPONENT_HASH_MISMATCH".to_string());
        }
        if component.is_primary_model && component.engine.as_deref() == Some("openvino") {
            super::openvino_runtime::validate_ir_pair(&model_dir.join(relative))?;
        }
    }
    Ok(())
}
