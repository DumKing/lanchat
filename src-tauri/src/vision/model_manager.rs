//! 官方视觉模型目录与数据包安装。
//!
//! 模型包只能包含清单和 ONNX 数据文件：目录必须经过 Ed25519 签名，包本身
//! 还要经过 SHA-256、Manifest V3 与路径安全校验。解压始终发生在 staging，
//! 成功后才原子替换安装目录。

use super::{
    manifest::{validate_manifest, VisionManifestV3},
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
    "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";
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
    pub download_url: String,
    pub package_sha256: String,
    #[serde(default)]
    pub package_size_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct InstalledVisionPackage {
    pub profile: VisionCatalogProfile,
    pub install_dir: PathBuf,
    pub manifest_json: String,
    pub bytes: u64,
}

pub fn parse_signed_catalog(bytes: &[u8]) -> Result<VisionCatalog, String> {
    let signed: SignedCatalog =
        serde_json::from_slice(bytes).map_err(|_| "VISION_CATALOG_INVALID".to_string())?;
    let key_ring = TrustedKeyRing::new(CATALOG_ROOT_KEY_ID, CATALOG_ROOT_PUBLIC_KEY_HEX);
    verify_catalog(&signed, &key_ring)?;
    let catalog: VisionCatalog = serde_json::from_value(signed.catalog)
        .map_err(|_| "VISION_CATALOG_PAYLOAD_INVALID".to_string())?;
    if catalog.schema_version != 1 || catalog.profiles.is_empty() {
        return Err("VISION_CATALOG_SCHEMA_UNSUPPORTED".to_string());
    }
    for profile in &catalog.profiles {
        if profile.profile_id.trim().is_empty()
            || profile.profile_version.trim().is_empty()
            || profile.display_name.trim().is_empty()
            || profile.download_url.trim().is_empty()
            || profile.package_sha256.len() != 64
            || !profile
                .package_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err("VISION_CATALOG_PROFILE_INVALID".to_string());
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
        let manifest_path = model_dir.join("manifest.v3.json");
        let manifest_json = fs::read_to_string(&manifest_path)
            .map_err(|_| "VISION_PACKAGE_MANIFEST_MISSING".to_string())?;
        let manifest: VisionManifestV3 = serde_json::from_str(&manifest_json)
            .map_err(|_| "VISION_PACKAGE_MANIFEST_INVALID".to_string())?;
        validate_manifest(&manifest)?;
        if manifest.profile.id != profile.profile_id
            || manifest.profile.version != profile.profile_version
        {
            return Err("VISION_PACKAGE_PROFILE_MISMATCH".to_string());
        }
        // 旧推理适配层仍读取 manifest.json，模型包必须带它以保持一个兼容发布周期。
        if !model_dir.join("manifest.json").is_file() {
            return Err("VISION_PACKAGE_LEGACY_MANIFEST_MISSING".to_string());
        }
        verify_component_assets(&model_dir, &manifest)?;
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

fn safe_segment(value: &str) -> Result<&str, String> {
    let value = value.trim();
    if value.is_empty() || value.contains(['/', '\\']) || value == "." || value == ".." {
        Err("VISION_PACKAGE_PATH_INVALID".to_string())
    } else {
        Ok(value)
    }
}

fn verify_component_assets(model_dir: &Path, manifest: &VisionManifestV3) -> Result<(), String> {
    for component in &manifest.components {
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
        if !hex::encode(Sha256::digest(bytes)).eq_ignore_ascii_case(&component.sha256) {
            return Err("VISION_PACKAGE_COMPONENT_HASH_MISMATCH".to_string());
        }
    }
    Ok(())
}
