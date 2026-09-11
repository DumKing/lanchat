use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Mutex;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledPluginRecord {
    pub plugin_id: String,
    #[serde(default)]
    pub display_name: Option<String>,
    pub active_version: String,
    pub previous_version: Option<String>,
    pub enabled: bool,
    pub granted_capabilities: Vec<String>,
    pub source: String,
    pub signature_key_id: Option<String>,
    pub installed_at: i64,
    pub updated_at: i64,
    pub last_error: Option<String>,
}

pub struct PluginStorage {
    connection: Mutex<Connection>,
}

impl PluginStorage {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, String> {
        let connection = Connection::open(path).map_err(|error| format!("打开插件数据库失败: {error}"))?;
        connection
            .execute_batch(
                r#"
                PRAGMA foreign_keys = ON;
                CREATE TABLE IF NOT EXISTS plugin_versions (
                    plugin_id TEXT NOT NULL,
                    version TEXT NOT NULL,
                    install_path TEXT NOT NULL,
                    package_sha256 TEXT NOT NULL,
                    signature_key_id TEXT,
                    installed_at INTEGER NOT NULL,
                    PRIMARY KEY (plugin_id, version)
                );
                CREATE TABLE IF NOT EXISTS plugin_installations (
                    plugin_id TEXT PRIMARY KEY,
                    active_version TEXT NOT NULL,
                    previous_version TEXT,
                    enabled INTEGER NOT NULL DEFAULT 0,
                    granted_capabilities_json TEXT NOT NULL DEFAULT '[]',
                    source TEXT NOT NULL,
                    signature_key_id TEXT,
                    installed_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL,
                    last_error TEXT,
                    FOREIGN KEY (plugin_id, active_version)
                      REFERENCES plugin_versions(plugin_id, version)
                );
                CREATE TABLE IF NOT EXISTS plugin_private_storage (
                    plugin_id TEXT NOT NULL,
                    storage_key TEXT NOT NULL,
                    value_json TEXT NOT NULL,
                    updated_at INTEGER NOT NULL,
                    PRIMARY KEY (plugin_id, storage_key)
                );
                "#,
            )
            .map_err(|error| format!("初始化插件数据库失败: {error}"))?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn register_version(
        &self,
        plugin_id: &str,
        version: &str,
        install_path: &str,
        package_sha256: &str,
        signature_key_id: Option<&str>,
        installed_at: i64,
    ) -> Result<(), String> {
        let connection = self.connection.lock().map_err(|_| "插件数据库锁已损坏".to_string())?;
        connection
            .execute(
                "INSERT OR REPLACE INTO plugin_versions (plugin_id,version,install_path,package_sha256,signature_key_id,installed_at) VALUES (?1,?2,?3,?4,?5,?6)",
                params![plugin_id, version, install_path, package_sha256, signature_key_id, installed_at],
            )
            .map_err(|error| format!("保存插件版本失败: {error}"))?;
        Ok(())
    }

    pub fn activate(
        &self,
        plugin_id: &str,
        version: &str,
        requested_capabilities: &[String],
        source: &str,
        updated_at: i64,
    ) -> Result<(), String> {
        let mut connection = self.connection.lock().map_err(|_| "插件数据库锁已损坏".to_string())?;
        let transaction = connection.transaction().map_err(|error| format!("启动插件事务失败: {error}"))?;
        let version_key: Option<Option<String>> = transaction
            .query_row(
                "SELECT signature_key_id FROM plugin_versions WHERE plugin_id=?1 AND version=?2",
                params![plugin_id, version],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| format!("读取插件版本失败: {error}"))?;
        let signature_key_id = version_key.ok_or_else(|| "准备启用的插件版本不存在".to_string())?;
        let previous: Option<(String, Option<String>, i64)> = transaction
            .query_row(
                "SELECT active_version,previous_version,installed_at FROM plugin_installations WHERE plugin_id=?1",
                [plugin_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(|error| format!("读取插件安装状态失败: {error}"))?;
        let capabilities = requested_capabilities.iter().cloned().collect::<BTreeSet<_>>();
        let capabilities_json = serde_json::to_string(&capabilities).map_err(|error| format!("序列化插件权限失败: {error}"))?;
        let (previous_version, installed_at) = match previous {
            Some((active, _older, first_installed_at)) if active != version => (Some(active), first_installed_at),
            Some((_active, older, first_installed_at)) => (older, first_installed_at),
            None => (None, updated_at),
        };
        transaction
            .execute(
                "INSERT INTO plugin_installations (plugin_id,active_version,previous_version,enabled,granted_capabilities_json,source,signature_key_id,installed_at,updated_at,last_error)
                 VALUES (?1,?2,?3,0,?4,?5,?6,?7,?8,NULL)
                 ON CONFLICT(plugin_id) DO UPDATE SET active_version=excluded.active_version,previous_version=excluded.previous_version,enabled=0,granted_capabilities_json=excluded.granted_capabilities_json,source=excluded.source,signature_key_id=excluded.signature_key_id,updated_at=excluded.updated_at,last_error=NULL",
                params![plugin_id, version, previous_version, capabilities_json, source, signature_key_id, installed_at, updated_at],
            )
            .map_err(|error| format!("激活插件版本失败: {error}"))?;
        transaction.commit().map_err(|error| format!("提交插件事务失败: {error}"))
    }

    pub fn rollback(&self, plugin_id: &str, updated_at: i64) -> Result<String, String> {
        let mut connection = self.connection.lock().map_err(|_| "插件数据库锁已损坏".to_string())?;
        let transaction = connection.transaction().map_err(|error| format!("启动插件事务失败: {error}"))?;
        let (active, previous): (String, Option<String>) = transaction
            .query_row(
                "SELECT active_version,previous_version FROM plugin_installations WHERE plugin_id=?1",
                [plugin_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|_| "插件没有可回滚的安装状态".to_string())?;
        let previous = previous.ok_or_else(|| "插件没有可回滚版本".to_string())?;
        let signature_key_id: Option<String> = transaction
            .query_row(
                "SELECT signature_key_id FROM plugin_versions WHERE plugin_id=?1 AND version=?2",
                params![plugin_id, previous],
                |row| row.get(0),
            )
            .map_err(|_| "回滚版本文件记录不存在".to_string())?;
        transaction
            .execute(
                "UPDATE plugin_installations SET active_version=?1,previous_version=?2,enabled=0,signature_key_id=?3,updated_at=?4,last_error=NULL WHERE plugin_id=?5",
                params![previous, active, signature_key_id, updated_at, plugin_id],
            )
            .map_err(|error| format!("回滚插件失败: {error}"))?;
        transaction.commit().map_err(|error| format!("提交插件事务失败: {error}"))?;
        Ok(previous)
    }

    pub fn set_enabled(&self, plugin_id: &str, enabled: bool, updated_at: i64) -> Result<(), String> {
        let connection = self.connection.lock().map_err(|_| "插件数据库锁已损坏".to_string())?;
        let changed = connection
            .execute(
                "UPDATE plugin_installations SET enabled=?1,updated_at=?2 WHERE plugin_id=?3",
                params![enabled as i32, updated_at, plugin_id],
            )
            .map_err(|error| format!("更新插件启用状态失败: {error}"))?;
        if changed == 0 {
            return Err("插件尚未安装".to_string());
        }
        Ok(())
    }

    pub fn set_granted_capabilities(&self, plugin_id: &str, capabilities: &[String], updated_at: i64) -> Result<(), String> {
        let capabilities = capabilities.iter().cloned().collect::<BTreeSet<_>>();
        let value = serde_json::to_string(&capabilities).map_err(|error| format!("序列化插件权限失败: {error}"))?;
        let connection = self.connection.lock().map_err(|_| "插件数据库锁已损坏".to_string())?;
        let changed = connection
            .execute(
                "UPDATE plugin_installations SET granted_capabilities_json=?1,enabled=0,updated_at=?2 WHERE plugin_id=?3",
                params![value, updated_at, plugin_id],
            )
            .map_err(|error| format!("更新插件权限失败: {error}"))?;
        if changed == 0 {
            return Err("插件尚未安装".to_string());
        }
        Ok(())
    }

    pub fn get(&self, plugin_id: &str) -> Result<Option<InstalledPluginRecord>, String> {
        let connection = self.connection.lock().map_err(|_| "插件数据库锁已损坏".to_string())?;
        connection
            .query_row(
                "SELECT plugin_id,active_version,previous_version,enabled,granted_capabilities_json,source,signature_key_id,installed_at,updated_at,last_error FROM plugin_installations WHERE plugin_id=?1",
                [plugin_id],
                |row| {
                    let capabilities: String = row.get(4)?;
                    Ok(InstalledPluginRecord {
                        plugin_id: row.get(0)?,
                        display_name: None,
                        active_version: row.get(1)?,
                        previous_version: row.get(2)?,
                        enabled: row.get::<_, i64>(3)? != 0,
                        granted_capabilities: serde_json::from_str(&capabilities).unwrap_or_default(),
                        source: row.get(5)?,
                        signature_key_id: row.get(6)?,
                        installed_at: row.get(7)?,
                        updated_at: row.get(8)?,
                        last_error: row.get(9)?,
                    })
                },
            )
            .optional()
            .map_err(|error| format!("读取插件状态失败: {error}"))
    }

    pub fn list(&self) -> Result<Vec<InstalledPluginRecord>, String> {
        let connection = self.connection.lock().map_err(|_| "插件数据库锁已损坏".to_string())?;
        let mut statement = connection
            .prepare("SELECT plugin_id,active_version,previous_version,enabled,granted_capabilities_json,source,signature_key_id,installed_at,updated_at,last_error FROM plugin_installations ORDER BY plugin_id")
            .map_err(|error| format!("准备插件列表查询失败: {error}"))?;
        let rows = statement
            .query_map([], |row| {
                let capabilities: String = row.get(4)?;
                Ok(InstalledPluginRecord {
                    plugin_id: row.get(0)?,
                    display_name: None,
                    active_version: row.get(1)?,
                    previous_version: row.get(2)?,
                    enabled: row.get::<_, i64>(3)? != 0,
                    granted_capabilities: serde_json::from_str(&capabilities).unwrap_or_default(),
                    source: row.get(5)?,
                    signature_key_id: row.get(6)?,
                    installed_at: row.get(7)?,
                    updated_at: row.get(8)?,
                    last_error: row.get(9)?,
                })
            })
            .map_err(|error| format!("查询插件列表失败: {error}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("读取插件列表失败: {error}"))
    }

    pub fn version_install_path(&self, plugin_id: &str, version: &str) -> Result<Option<String>, String> {
        let connection = self.connection.lock().map_err(|_| "插件数据库锁已损坏".to_string())?;
        connection
            .query_row(
                "SELECT install_path FROM plugin_versions WHERE plugin_id=?1 AND version=?2",
                params![plugin_id, version],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| format!("读取插件版本路径失败: {error}"))
    }

    pub fn unregister_version(&self, plugin_id: &str, version: &str) -> Result<(), String> {
        let connection = self.connection.lock().map_err(|_| "插件数据库锁已损坏".to_string())?;
        connection
            .execute(
                "DELETE FROM plugin_versions WHERE plugin_id=?1 AND version=?2 AND NOT EXISTS(SELECT 1 FROM plugin_installations WHERE plugin_id=?1 AND (active_version=?2 OR previous_version=?2))",
                params![plugin_id, version],
            )
            .map_err(|error| format!("清理插件版本记录失败: {error}"))?;
        Ok(())
    }

    pub fn uninstall(&self, plugin_id: &str, delete_private_data: bool) -> Result<(), String> {
        let mut connection = self.connection.lock().map_err(|_| "插件数据库锁已损坏".to_string())?;
        let transaction = connection.transaction().map_err(|error| format!("启动插件卸载事务失败: {error}"))?;
        let removed = transaction
            .execute("DELETE FROM plugin_installations WHERE plugin_id=?1", [plugin_id])
            .map_err(|error| format!("删除插件安装状态失败: {error}"))?;
        if removed == 0 {
            return Err("插件尚未安装".to_string());
        }
        transaction
            .execute("DELETE FROM plugin_versions WHERE plugin_id=?1", [plugin_id])
            .map_err(|error| format!("删除插件版本记录失败: {error}"))?;
        if delete_private_data {
            transaction
                .execute("DELETE FROM plugin_private_storage WHERE plugin_id=?1", [plugin_id])
                .map_err(|error| format!("删除插件私有数据失败: {error}"))?;
        }
        transaction.commit().map_err(|error| format!("提交插件卸载事务失败: {error}"))
    }

    pub fn storage_set(&self, plugin_id: &str, key: &str, value: &serde_json::Value, updated_at: i64) -> Result<(), String> {
        let value = serde_json::to_string(value).map_err(|error| format!("序列化插件数据失败: {error}"))?;
        let connection = self.connection.lock().map_err(|_| "插件数据库锁已损坏".to_string())?;
        connection
            .execute(
                "INSERT INTO plugin_private_storage (plugin_id,storage_key,value_json,updated_at) VALUES (?1,?2,?3,?4) ON CONFLICT(plugin_id,storage_key) DO UPDATE SET value_json=excluded.value_json,updated_at=excluded.updated_at",
                params![plugin_id, key, value, updated_at],
            )
            .map_err(|error| format!("保存插件数据失败: {error}"))?;
        Ok(())
    }

    pub fn storage_get(&self, plugin_id: &str, key: &str) -> Result<Option<serde_json::Value>, String> {
        let connection = self.connection.lock().map_err(|_| "插件数据库锁已损坏".to_string())?;
        let value: Option<String> = connection
            .query_row(
                "SELECT value_json FROM plugin_private_storage WHERE plugin_id=?1 AND storage_key=?2",
                params![plugin_id, key],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| format!("读取插件数据失败: {error}"))?;
        value
            .map(|value| serde_json::from_str(&value).map_err(|error| format!("插件数据损坏: {error}")))
            .transpose()
    }

    pub fn delete_private_storage(&self, plugin_id: &str) -> Result<usize, String> {
        let connection = self.connection.lock().map_err(|_| "插件数据库锁已损坏".to_string())?;
        connection
            .execute("DELETE FROM plugin_private_storage WHERE plugin_id=?1", [plugin_id])
            .map_err(|error| format!("删除插件私有数据失败: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn values(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| (*item).to_string()).collect()
    }

    #[test]
    fn activates_versions_and_rolls_back_with_plugin_disabled() {
        let temp = tempdir().unwrap();
        let storage = PluginStorage::open(temp.path().join("plugins.sqlite3")).unwrap();
        storage.register_version("com.lanchat.gomoku", "1.0.0", "v1", "aa", Some("key-1"), 1).unwrap();
        storage.activate("com.lanchat.gomoku", "1.0.0", &values(&["rooms.read"]), "official", 2).unwrap();
        storage.set_enabled("com.lanchat.gomoku", true, 3).unwrap();
        storage.register_version("com.lanchat.gomoku", "1.1.0", "v2", "bb", Some("key-1"), 4).unwrap();
        storage.activate("com.lanchat.gomoku", "1.1.0", &values(&["rooms.read", "rooms.write"]), "official", 5).unwrap();

        let active = storage.get("com.lanchat.gomoku").unwrap().unwrap();
        assert_eq!(active.active_version, "1.1.0");
        assert_eq!(active.previous_version.as_deref(), Some("1.0.0"));
        assert!(!active.enabled);

        assert_eq!(storage.rollback("com.lanchat.gomoku", 6).unwrap(), "1.0.0");
        let rolled_back = storage.get("com.lanchat.gomoku").unwrap().unwrap();
        assert_eq!(rolled_back.active_version, "1.0.0");
        assert_eq!(rolled_back.previous_version.as_deref(), Some("1.1.0"));
        assert!(!rolled_back.enabled);
    }

    #[test]
    fn permission_changes_disable_running_plugin() {
        let temp = tempdir().unwrap();
        let storage = PluginStorage::open(temp.path().join("plugins.sqlite3")).unwrap();
        storage.register_version("com.lanchat.gomoku", "1.0.0", "v1", "aa", None, 1).unwrap();
        storage.activate("com.lanchat.gomoku", "1.0.0", &values(&["rooms.read"]), "development", 2).unwrap();
        storage.set_enabled("com.lanchat.gomoku", true, 3).unwrap();
        storage.set_granted_capabilities("com.lanchat.gomoku", &[], 4).unwrap();
        assert!(!storage.get("com.lanchat.gomoku").unwrap().unwrap().enabled);
    }

    #[test]
    fn private_storage_is_isolated_by_plugin_id() {
        let temp = tempdir().unwrap();
        let storage = PluginStorage::open(temp.path().join("plugins.sqlite3")).unwrap();
        storage.storage_set("com.lanchat.a", "settings", &serde_json::json!({"sound": true}), 1).unwrap();
        storage.storage_set("com.lanchat.b", "settings", &serde_json::json!({"sound": false}), 1).unwrap();
        assert_eq!(storage.storage_get("com.lanchat.a", "settings").unwrap(), Some(serde_json::json!({"sound": true})));
        assert_eq!(storage.delete_private_storage("com.lanchat.a").unwrap(), 1);
        assert_eq!(storage.storage_get("com.lanchat.a", "settings").unwrap(), None);
        assert_eq!(storage.storage_get("com.lanchat.b", "settings").unwrap(), Some(serde_json::json!({"sound": false})));
    }

    #[test]
    fn uninstall_can_retain_or_delete_private_data() {
        let temp = tempdir().unwrap();
        let storage = PluginStorage::open(temp.path().join("plugins.sqlite3")).unwrap();
        for plugin_id in ["com.lanchat.keep", "com.lanchat.delete"] {
            storage.register_version(plugin_id, "1.0.0", "v1", "aa", None, 1).unwrap();
            storage.activate(plugin_id, "1.0.0", &[], "development", 2).unwrap();
            storage.storage_set(plugin_id, "settings", &serde_json::json!({"value": 1}), 3).unwrap();
        }
        storage.uninstall("com.lanchat.keep", false).unwrap();
        storage.uninstall("com.lanchat.delete", true).unwrap();
        assert!(storage.get("com.lanchat.keep").unwrap().is_none());
        assert_eq!(storage.storage_get("com.lanchat.keep", "settings").unwrap(), Some(serde_json::json!({"value": 1})));
        assert_eq!(storage.storage_get("com.lanchat.delete", "settings").unwrap(), None);
    }
}
