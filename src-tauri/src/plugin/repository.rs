use crate::plugin::manifest::{parse_and_validate_manifest, PluginManifest};
use crate::plugin::package::{
    extract_package, parse_integrity_manifest, verify_integrity, PackageLimits,
};
use crate::plugin::signature::{
    canonical_signature_payload, sha256_hex, PluginKeyring,
};
use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const MAX_PACKAGE_BYTES: u64 = 128 * 1024 * 1024;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackageSignature {
    key_id: String,
    signature: String,
}

#[derive(Debug, Clone)]
pub struct InstalledPackage {
    pub manifest: PluginManifest,
    pub install_path: PathBuf,
    pub package_sha256: String,
    pub signature_key_id: Option<String>,
    pub development: bool,
}

pub struct PluginRepository {
    root: PathBuf,
    host_version: String,
    keyring: PluginKeyring,
}

impl PluginRepository {
    pub fn new(root: impl Into<PathBuf>, host_version: impl Into<String>, keyring: PluginKeyring) -> Self {
        Self {
            root: root.into(),
            host_version: host_version.into(),
            keyring,
        }
    }

    pub fn install(&self, package_path: &Path, allow_unsigned_development: bool) -> Result<InstalledPackage, String> {
        let metadata = fs::metadata(package_path).map_err(|error| format!("读取插件包失败: {error}"))?;
        if !metadata.is_file() || metadata.len() > MAX_PACKAGE_BYTES {
            return Err("插件包不存在或压缩体积超过限制".to_string());
        }
        let package_bytes = fs::read(package_path).map_err(|error| format!("读取插件包失败: {error}"))?;
        let package_sha256 = sha256_hex(&package_bytes);
        let staging = self.root.join(".staging").join(Uuid::new_v4().to_string());

        let result = (|| {
            let extracted = extract_package(
                Cursor::new(&package_bytes),
                &staging,
                PackageLimits::default(),
            )?;
            let plugin_json = fs::read(staging.join("plugin.json"))
                .map_err(|_| "插件包缺少 plugin.json".to_string())?;
            let integrity_json = fs::read(staging.join("integrity.json"))
                .map_err(|_| "插件包缺少 integrity.json".to_string())?;
            let manifest = parse_and_validate_manifest(&plugin_json, &self.host_version)?;
            let integrity = parse_integrity_manifest(&integrity_json)?;
            verify_integrity(&staging, &integrity)?;

            let actual_files = extracted
                .iter()
                .map(|path| path.to_string_lossy().replace('\\', "/"))
                .filter(|path| path != "integrity.json" && path != "signature.json")
                .map(|path| path.to_ascii_lowercase())
                .collect::<HashSet<_>>();
            let declared_files = integrity
                .files
                .iter()
                .map(|file| file.path.to_ascii_lowercase())
                .collect::<HashSet<_>>();
            if actual_files != declared_files {
                return Err("完整性清单必须覆盖插件包内全部业务文件".to_string());
            }
            if !actual_files.contains("plugin.json") {
                return Err("完整性清单必须包含 plugin.json".to_string());
            }

            let signature_path = staging.join("signature.json");
            let (signature_key_id, development) = if signature_path.exists() {
                let signature: PackageSignature = serde_json::from_slice(
                    &fs::read(&signature_path).map_err(|error| format!("读取插件签名失败: {error}"))?,
                )
                .map_err(|error| format!("signature.json 解析失败: {error}"))?;
                let signature_bytes = hex::decode(&signature.signature)
                    .map_err(|_| "插件签名必须是十六进制".to_string())?;
                let payload = canonical_signature_payload(
                    &manifest.id,
                    &manifest.version,
                    &sha256_hex(&plugin_json),
                    &sha256_hex(&integrity_json),
                )?;
                self.keyring.verify(&signature.key_id, &payload, &signature_bytes)?;
                (Some(signature.key_id), false)
            } else if allow_unsigned_development {
                (None, true)
            } else {
                return Err("正式插件包必须包含可信签名".to_string());
            };

            let install_path = self
                .root
                .join("plugins")
                .join(&manifest.id)
                .join("versions")
                .join(&manifest.version);
            if install_path.exists() {
                return Err("该插件版本已经安装".to_string());
            }
            let parent = install_path.parent().ok_or_else(|| "插件安装路径无效".to_string())?;
            fs::create_dir_all(parent).map_err(|error| format!("创建插件版本目录失败: {error}"))?;
            fs::rename(&staging, &install_path).map_err(|error| format!("原子安装插件版本失败: {error}"))?;

            Ok(InstalledPackage {
                manifest,
                install_path,
                package_sha256,
                signature_key_id,
                development,
            })
        })();

        if result.is_err() && staging.exists() {
            let _ = fs::remove_dir_all(&staging);
        }
        result
    }

    pub fn discard_version(&self, plugin_id: &str, version: &str) -> Result<(), String> {
        let path = self.root.join("plugins").join(plugin_id).join("versions").join(version);
        if path.exists() {
            fs::remove_dir_all(path).map_err(|error| format!("清理插件版本目录失败: {error}"))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::signature::TrustedPluginKey;
    use ed25519_dalek::{Signer, SigningKey};
    use std::io::{Cursor, Read, Write};
    use tempfile::tempdir;
    use zip::write::SimpleFileOptions;
    use zip::ZipArchive;

    fn signed_package(signing: &SigningKey, tamper: bool) -> Vec<u8> {
        let plugin_json = serde_json::to_vec(&serde_json::json!({
            "manifestVersion": 1,
            "id": "com.lanchat.gomoku",
            "name": "五子棋",
            "version": "1.0.0",
            "apiVersion": "1.0.0",
            "minHostVersion": "0.8.0",
            "type": "web",
            "entry": "dist/index.html",
            "icon": "assets/icon.png",
            "singleton": true,
            "capabilities": ["rooms.read"]
        })).unwrap();
        let index = if tamper { b"tampered".as_slice() } else { b"app".as_slice() };
        let integrity_json = serde_json::to_vec(&serde_json::json!({
            "schemaVersion": 1,
            "files": [
                {"path": "plugin.json", "sha256": sha256_hex(&plugin_json), "size": plugin_json.len()},
                {"path": "dist/index.html", "sha256": sha256_hex(b"app"), "size": 3}
            ]
        })).unwrap();
        let payload = canonical_signature_payload(
            "com.lanchat.gomoku",
            "1.0.0",
            &sha256_hex(&plugin_json),
            &sha256_hex(&integrity_json),
        ).unwrap();
        let signature_json = serde_json::to_vec(&serde_json::json!({
            "keyId": "official-test",
            "signature": hex::encode(signing.sign(&payload).to_bytes())
        })).unwrap();

        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, contents) in [
            ("plugin.json", plugin_json.as_slice()),
            ("integrity.json", integrity_json.as_slice()),
            ("signature.json", signature_json.as_slice()),
            ("dist/index.html", index),
        ] {
            writer.start_file(name, SimpleFileOptions::default()).unwrap();
            writer.write_all(contents).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    fn repository(root: &Path, signing: &SigningKey) -> PluginRepository {
        PluginRepository::new(
            root,
            "0.8.0",
            PluginKeyring::new(
                [TrustedPluginKey {
                    key_id: "official-test".to_string(),
                    public_key: signing.verifying_key().to_bytes(),
                }],
                [],
            ),
        )
    }

    #[test]
    fn installs_valid_signed_package_into_immutable_version_directory() {
        let temp = tempdir().unwrap();
        let signing = SigningKey::from_bytes(&[31; 32]);
        let package_path = temp.path().join("gomoku.lcp");
        fs::write(&package_path, signed_package(&signing, false)).unwrap();
        let installed = repository(&temp.path().join("repository"), &signing)
            .install(&package_path, false)
            .expect("signed package installs");
        assert_eq!(installed.signature_key_id.as_deref(), Some("official-test"));
        assert!(!installed.development);
        assert!(installed.install_path.join("dist/index.html").exists());
    }

    #[test]
    fn rejects_tampered_package_and_cleans_staging_directory() {
        let temp = tempdir().unwrap();
        let signing = SigningKey::from_bytes(&[31; 32]);
        let root = temp.path().join("repository");
        let package_path = temp.path().join("gomoku.lcp");
        fs::write(&package_path, signed_package(&signing, true)).unwrap();
        assert!(repository(&root, &signing).install(&package_path, false).is_err());
        let staging = root.join(".staging");
        assert!(staging.read_dir().map(|mut entries| entries.next().is_none()).unwrap_or(true));
    }

    #[test]
    fn unsigned_package_requires_explicit_development_mode() {
        let temp = tempdir().unwrap();
        let signing = SigningKey::from_bytes(&[31; 32]);
        let bytes = signed_package(&signing, false);
        let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for index in 0..archive.len() {
            let mut file = archive.by_index(index).unwrap();
            if file.name() == "signature.json" { continue; }
            let name = file.name().to_string();
            let mut contents = Vec::new();
            file.read_to_end(&mut contents).unwrap();
            writer.start_file(name, SimpleFileOptions::default()).unwrap();
            writer.write_all(&contents).unwrap();
        }
        let package_path = temp.path().join("unsigned.lcp");
        fs::write(&package_path, writer.finish().unwrap().into_inner()).unwrap();
        let root = temp.path().join("repository");
        assert!(repository(&root, &signing).install(&package_path, false).is_err());
        assert!(repository(&root, &signing).install(&package_path, true).unwrap().development);
    }
}
