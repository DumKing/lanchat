use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Component, Path};

pub const SUPPORTED_MANIFEST_VERSION: u32 = 1;
pub const SUPPORTED_API_MAJOR: u64 = 1;

const KNOWN_CAPABILITIES: &[&str] = &[
    "devices.read",
    "chat.read",
    "chat.send",
    "rooms.read",
    "rooms.write",
    "leaderboard.read",
    "leaderboard.write",
    "storage.private",
    "theme.read",
    "ui.notify",
    "ui.filePicker",
    "network.lan",
    "network.internet",
    "logger.write",
    "native.sidecar",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifest {
    pub manifest_version: u32,
    pub id: String,
    pub name: String,
    pub version: String,
    pub api_version: String,
    pub min_host_version: String,
    #[serde(rename = "type")]
    pub plugin_type: PluginType,
    pub entry: String,
    pub icon: String,
    pub singleton: bool,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub contributes: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PluginType {
    Web,
    WebSidecar,
}

fn valid_plugin_id(id: &str) -> bool {
    let segments = id.split('.').collect::<Vec<_>>();
    segments.len() >= 3
        && segments.iter().all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
}

pub fn validate_relative_asset_path(value: &str) -> Result<(), String> {
    if value.is_empty() || value.contains('\\') {
        return Err("资源路径必须使用非空的正斜杠相对路径".to_string());
    }
    let path = Path::new(value);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!("不安全的插件资源路径: {value}"));
    }
    Ok(())
}

pub fn parse_and_validate_manifest(bytes: &[u8], host_version: &str) -> Result<PluginManifest, String> {
    let manifest: PluginManifest =
        serde_json::from_slice(bytes).map_err(|error| format!("plugin.json 解析失败: {error}"))?;

    if manifest.manifest_version != SUPPORTED_MANIFEST_VERSION {
        return Err("不支持的插件清单版本".to_string());
    }
    if !valid_plugin_id(&manifest.id) {
        return Err("插件 ID 必须是至少三段的小写反向域名".to_string());
    }
    if manifest.name.trim().is_empty() || manifest.name.chars().count() > 80 {
        return Err("插件名称长度无效".to_string());
    }
    Version::parse(&manifest.version).map_err(|_| "插件版本不是合法 SemVer".to_string())?;
    let api = Version::parse(&manifest.api_version).map_err(|_| "API 版本不是合法 SemVer".to_string())?;
    if api.major != SUPPORTED_API_MAJOR {
        return Err("宿主不支持此插件 API 主版本".to_string());
    }
    let host = Version::parse(host_version).map_err(|_| "宿主版本不是合法 SemVer".to_string())?;
    let requirement = VersionReq::parse(&format!(">={}", manifest.min_host_version))
        .map_err(|_| "最低宿主版本不是合法 SemVer".to_string())?;
    if !requirement.matches(&host) {
        return Err(format!("插件要求 LanChat {} 或更高版本", manifest.min_host_version));
    }
    validate_relative_asset_path(&manifest.entry)?;
    validate_relative_asset_path(&manifest.icon)?;

    let known = KNOWN_CAPABILITIES.iter().copied().collect::<HashSet<_>>();
    let mut unique = HashSet::new();
    for capability in &manifest.capabilities {
        if !known.contains(capability.as_str()) {
            return Err(format!("未知插件能力: {capability}"));
        }
        if !unique.insert(capability) {
            return Err(format!("重复插件能力: {capability}"));
        }
    }
    if manifest.plugin_type == PluginType::WebSidecar
        && !manifest.capabilities.iter().any(|value| value == "native.sidecar")
    {
        return Err("web-sidecar 插件必须声明 native.sidecar 能力".to_string());
    }

    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest_json(overrides: serde_json::Value) -> Vec<u8> {
        let mut value = serde_json::json!({
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
            "capabilities": ["rooms.read", "rooms.write"]
        });
        if let (Some(target), Some(source)) = (value.as_object_mut(), overrides.as_object()) {
            target.extend(source.clone());
        }
        serde_json::to_vec(&value).unwrap()
    }

    #[test]
    fn validates_manifest_and_host_compatibility() {
        let manifest = parse_and_validate_manifest(&manifest_json(serde_json::json!({})), "0.8.0")
            .expect("manifest is valid");
        assert_eq!(manifest.id, "com.lanchat.gomoku");
        assert!(parse_and_validate_manifest(&manifest_json(serde_json::json!({})), "0.7.2").is_err());
    }

    #[test]
    fn rejects_path_traversal_and_unknown_capability() {
        assert!(parse_and_validate_manifest(
            &manifest_json(serde_json::json!({"entry": "../index.html"})),
            "0.8.0"
        )
        .is_err());
        assert!(parse_and_validate_manifest(
            &manifest_json(serde_json::json!({"capabilities": ["shell.execute"]})),
            "0.8.0"
        )
        .is_err());
    }

    #[test]
    fn sidecar_requires_explicit_capability() {
        assert!(parse_and_validate_manifest(
            &manifest_json(serde_json::json!({"type": "web-sidecar"})),
            "0.8.0"
        )
        .is_err());
    }
}
