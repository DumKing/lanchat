use crate::plugin::manifest::validate_plugin_id;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginCatalog {
    pub schema_version: u32,
    pub generated_at: Option<String>,
    pub packages: Vec<CatalogPackage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogPackage {
    pub id: String,
    pub name: String,
    pub version: String,
    pub download_url: String,
    pub sha256: String,
    pub signature: String,
    pub key_id: String,
    pub min_host_version: String,
}

pub fn parse_catalog(bytes: &[u8]) -> Result<PluginCatalog, String> {
    let catalog: PluginCatalog = serde_json::from_slice(bytes)
        .map_err(|error| format!("插件目录解析失败: {error}"))?;
    if catalog.schema_version != 1 {
        return Err("不支持的插件目录版本".to_string());
    }
    let mut versions = HashSet::new();
    for package in &catalog.packages {
        validate_plugin_id(&package.id)?;
        Version::parse(&package.version).map_err(|_| format!("插件版本无效: {}", package.id))?;
        Version::parse(&package.min_host_version)
            .map_err(|_| format!("最低宿主版本无效: {}", package.id))?;
        if package.name.trim().is_empty() {
            return Err(format!("插件名称不能为空: {}", package.id));
        }
        if !package.download_url.starts_with("https://") {
            return Err(format!("插件下载地址必须使用 HTTPS: {}", package.id));
        }
        if package.sha256.len() != 64 || !package.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(format!("插件包哈希无效: {}", package.id));
        }
        if package.signature.len() != 128 || !package.signature.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(format!("插件包签名无效: {}", package.id));
        }
        if package.key_id.trim().is_empty() {
            return Err(format!("插件签名 keyId 不能为空: {}", package.id));
        }
        if !versions.insert((package.id.clone(), package.version.clone())) {
            return Err(format!("插件目录包含重复版本: {} {}", package.id, package.version));
        }
    }
    Ok(catalog)
}

pub fn compatible_packages(catalog: &PluginCatalog, host_version: &str) -> Result<Vec<CatalogPackage>, String> {
    let host = Version::parse(host_version).map_err(|_| "宿主版本无效".to_string())?;
    Ok(catalog
        .packages
        .iter()
        .filter(|package| Version::parse(&package.min_host_version).is_ok_and(|minimum| host >= minimum))
        .cloned()
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package(url: &str) -> serde_json::Value {
        serde_json::json!({
            "id": "com.lanchat.gomoku",
            "name": "五子棋",
            "version": "1.0.0",
            "downloadUrl": url,
            "sha256": "a".repeat(64),
            "signature": "b".repeat(128),
            "keyId": "official-2026",
            "minHostVersion": "0.8.0"
        })
    }

    #[test]
    fn accepts_versioned_https_catalog_and_filters_host_version() {
        let catalog = parse_catalog(&serde_json::to_vec(&serde_json::json!({
            "schemaVersion": 1,
            "generatedAt": null,
            "packages": [package("https://github.com/DumKing/plugin.lcp")]
        })).unwrap()).unwrap();
        assert!(compatible_packages(&catalog, "0.7.2").unwrap().is_empty());
        assert_eq!(compatible_packages(&catalog, "0.8.0").unwrap().len(), 1);
    }

    #[test]
    fn rejects_non_https_and_duplicate_versions() {
        let invalid = serde_json::json!({
            "schemaVersion": 1,
            "generatedAt": null,
            "packages": [package("http://example.test/plugin.lcp")]
        });
        assert!(parse_catalog(&serde_json::to_vec(&invalid).unwrap()).is_err());

        let duplicate = serde_json::json!({
            "schemaVersion": 1,
            "generatedAt": null,
            "packages": [
                package("https://example.test/one.lcp"),
                package("https://example.test/two.lcp")
            ]
        });
        assert!(parse_catalog(&serde_json::to_vec(&duplicate).unwrap()).is_err());
    }
}
