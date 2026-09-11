use crate::plugin::manifest::{parse_and_validate_manifest, PluginManifest};
use crate::plugin::storage::PluginStorage;
use std::fs;
use std::path::Path;

pub fn load_enabled_plugin_manifests(
    storage: &PluginStorage,
    host_version: &str,
) -> Result<Vec<PluginManifest>, String> {
    let mut manifests = Vec::new();
    for installation in storage.list()?.into_iter().filter(|record| record.enabled) {
        let install_path = storage
            .version_install_path(&installation.plugin_id, &installation.active_version)?
            .ok_or_else(|| format!("插件 {} 的活动版本路径不存在", installation.plugin_id))?;
        let bytes = fs::read(Path::new(&install_path).join("plugin.json"))
            .map_err(|error| format!("读取插件 {} 清单失败: {error}", installation.plugin_id))?;
        let manifest = parse_and_validate_manifest(&bytes, host_version)?;
        if manifest.id != installation.plugin_id || manifest.version != installation.active_version {
            return Err(format!("插件 {} 的活动清单与安装记录不一致", installation.plugin_id));
        }
        manifests.push(manifest);
    }
    manifests.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(manifests)
}

#[cfg(test)]
mod tests {
    use super::load_enabled_plugin_manifests;
    use crate::plugin::storage::PluginStorage;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn only_returns_valid_enabled_active_manifests() {
        let root = tempdir().unwrap();
        let database = PluginStorage::open(root.path().join("plugins.sqlite")).unwrap();
        let enabled_path = root.path().join("enabled");
        let disabled_path = root.path().join("disabled");
        fs::create_dir_all(&enabled_path).unwrap();
        fs::create_dir_all(&disabled_path).unwrap();
        fs::write(enabled_path.join("plugin.json"), manifest("com.lanchat.gomoku", "0.8.0")).unwrap();
        fs::write(disabled_path.join("plugin.json"), manifest("com.lanchat.vision", "0.8.0")).unwrap();

        register(&database, "com.lanchat.gomoku", &enabled_path, true);
        register(&database, "com.lanchat.vision", &disabled_path, false);

        let manifests = load_enabled_plugin_manifests(&database, "0.8.0").unwrap();
        assert_eq!(manifests.len(), 1);
        assert_eq!(manifests[0].id, "com.lanchat.gomoku");
    }

    fn register(database: &PluginStorage, plugin_id: &str, path: &std::path::Path, enabled: bool) {
        database
            .register_version(plugin_id, "0.8.0", &path.to_string_lossy(), "sha", None, 1)
            .unwrap();
        database
            .activate(plugin_id, "0.8.0", &[], "development", 1)
            .unwrap();
        database.set_enabled(plugin_id, enabled, 2).unwrap();
    }

    fn manifest(id: &str, version: &str) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "manifestVersion": 1,
            "id": id,
            "name": id,
            "version": version,
            "apiVersion": "1.0.0",
            "minHostVersion": "0.8.0",
            "type": "web",
            "entry": "dist/index.html",
            "icon": "icon.png",
            "singleton": true,
            "capabilities": [],
            "contributes": { "navigation": [{ "id": "feature", "title": "Feature" }] }
        }))
        .unwrap()
    }
}
