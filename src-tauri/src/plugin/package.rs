use crate::plugin::manifest::validate_relative_asset_path;
use crate::plugin::signature::sha256_hex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::io::{Read, Seek};
use std::path::{Path, PathBuf};
use zip::ZipArchive;

#[derive(Debug, Clone, Copy)]
pub struct PackageLimits {
    pub max_files: usize,
    pub max_unpacked_bytes: u64,
}

impl Default for PackageLimits {
    fn default() -> Self {
        Self {
            max_files: 4_096,
            max_unpacked_bytes: 256 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrityManifest {
    pub schema_version: u32,
    pub files: Vec<IntegrityFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrityFile {
    pub path: String,
    pub sha256: String,
    pub size: u64,
}

fn is_symlink(unix_mode: Option<u32>) -> bool {
    unix_mode.is_some_and(|mode| mode & 0o170000 == 0o120000)
}

fn normalized_entry_name(name: &str, is_dir: bool) -> Result<String, String> {
    let value = if is_dir { name.trim_end_matches('/') } else { name };
    if value.is_empty() && is_dir {
        return Ok(String::new());
    }
    validate_relative_asset_path(value)?;
    Ok(value.to_string())
}

pub fn extract_package<R: Read + Seek>(reader: R, destination: &Path, limits: PackageLimits) -> Result<Vec<PathBuf>, String> {
    let mut archive = ZipArchive::new(reader).map_err(|error| format!("插件包不是有效 ZIP: {error}"))?;
    if archive.len() > limits.max_files {
        return Err("插件包文件数量超过限制".to_string());
    }

    let mut entries = Vec::with_capacity(archive.len());
    let mut names = HashSet::new();
    let mut total_size = 0_u64;
    for index in 0..archive.len() {
        let file = archive.by_index(index).map_err(|error| format!("读取插件包失败: {error}"))?;
        if is_symlink(file.unix_mode()) {
            return Err(format!("插件包不能包含符号链接: {}", file.name()));
        }
        let name = normalized_entry_name(file.name(), file.is_dir())?;
        if name.is_empty() {
            continue;
        }
        let collision_key = name.to_ascii_lowercase();
        if !names.insert(collision_key) {
            return Err(format!("插件包包含重复路径: {name}"));
        }
        total_size = total_size
            .checked_add(file.size())
            .ok_or_else(|| "插件包解压体积溢出".to_string())?;
        if total_size > limits.max_unpacked_bytes {
            return Err("插件包解压体积超过限制".to_string());
        }
        entries.push((index, name, file.is_dir(), file.size()));
    }

    fs::create_dir_all(destination).map_err(|error| format!("创建插件暂存目录失败: {error}"))?;
    let mut extracted = Vec::new();
    for (index, name, is_dir, declared_size) in entries {
        let output = destination.join(&name);
        if is_dir {
            fs::create_dir_all(&output).map_err(|error| format!("创建插件目录失败: {error}"))?;
            continue;
        }
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).map_err(|error| format!("创建插件目录失败: {error}"))?;
        }
        let mut input = archive.by_index(index).map_err(|error| format!("读取插件文件失败: {error}"))?;
        let mut output_file = fs::File::create(&output).map_err(|error| format!("创建插件文件失败: {error}"))?;
        let copied = std::io::copy(&mut input, &mut output_file)
            .map_err(|error| format!("写入插件文件失败: {error}"))?;
        if copied != declared_size {
            return Err(format!("插件文件长度不一致: {name}"));
        }
        extracted.push(PathBuf::from(name));
    }
    Ok(extracted)
}

pub fn parse_integrity_manifest(bytes: &[u8]) -> Result<IntegrityManifest, String> {
    let manifest: IntegrityManifest =
        serde_json::from_slice(bytes).map_err(|error| format!("integrity.json 解析失败: {error}"))?;
    if manifest.schema_version != 1 {
        return Err("不支持的完整性清单版本".to_string());
    }
    let mut paths = HashSet::new();
    for file in &manifest.files {
        validate_relative_asset_path(&file.path)?;
        if !paths.insert(file.path.to_ascii_lowercase()) {
            return Err(format!("完整性清单包含重复路径: {}", file.path));
        }
        if file.sha256.len() != 64 || !file.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(format!("文件哈希格式无效: {}", file.path));
        }
    }
    Ok(manifest)
}

pub fn verify_integrity(root: &Path, manifest: &IntegrityManifest) -> Result<(), String> {
    for expected in &manifest.files {
        let path = root.join(&expected.path);
        let metadata = fs::symlink_metadata(&path)
            .map_err(|_| format!("完整性清单中的文件不存在: {}", expected.path))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(format!("完整性清单只能引用普通文件: {}", expected.path));
        }
        if metadata.len() != expected.size {
            return Err(format!("插件文件长度校验失败: {}", expected.path));
        }
        let bytes = fs::read(&path).map_err(|error| format!("读取插件文件失败: {error}"))?;
        if !sha256_hex(&bytes).eq_ignore_ascii_case(&expected.sha256) {
            return Err(format!("插件文件哈希校验失败: {}", expected.path));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};
    use tempfile::tempdir;
    use zip::write::SimpleFileOptions;

    fn archive(entries: &[(&str, &[u8])]) -> Cursor<Vec<u8>> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, contents) in entries {
            writer.start_file(*name, SimpleFileOptions::default()).unwrap();
            writer.write_all(contents).unwrap();
        }
        writer.finish().unwrap()
    }

    #[test]
    fn extracts_safe_package() {
        let temp = tempdir().unwrap();
        let files = extract_package(
            archive(&[("plugin.json", b"{}"), ("dist/index.html", b"ok")]),
            temp.path(),
            PackageLimits::default(),
        )
        .expect("safe package extracts");
        assert_eq!(files.len(), 2);
        assert_eq!(fs::read(temp.path().join("dist/index.html")).unwrap(), b"ok");
    }

    #[test]
    fn rejects_path_traversal_and_case_collisions() {
        let temp = tempdir().unwrap();
        assert!(extract_package(
            archive(&[("../outside.txt", b"bad")]),
            temp.path(),
            PackageLimits::default()
        )
        .is_err());
        assert!(extract_package(
            archive(&[("dist/App.js", b"a"), ("dist/app.js", b"b")]),
            temp.path(),
            PackageLimits::default()
        )
        .is_err());
    }

    #[test]
    fn enforces_unpacked_size_limit_before_writing() {
        let temp = tempdir().unwrap();
        let result = extract_package(
            archive(&[("large.bin", b"12345")]),
            temp.path(),
            PackageLimits { max_files: 2, max_unpacked_bytes: 4 },
        );
        assert!(result.is_err());
        assert!(!temp.path().join("large.bin").exists());
    }

    #[test]
    fn verifies_integrity_hash_and_size() {
        let temp = tempdir().unwrap();
        fs::write(temp.path().join("index.html"), b"hello").unwrap();
        let manifest = IntegrityManifest {
            schema_version: 1,
            files: vec![IntegrityFile {
                path: "index.html".to_string(),
                sha256: sha256_hex(b"hello"),
                size: 5,
            }],
        };
        assert!(verify_integrity(temp.path(), &manifest).is_ok());
        fs::write(temp.path().join("index.html"), b"changed").unwrap();
        assert!(verify_integrity(temp.path(), &manifest).is_err());
    }
}
