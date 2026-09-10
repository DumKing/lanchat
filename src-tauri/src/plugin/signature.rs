use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct TrustedPluginKey {
    pub key_id: String,
    pub public_key: [u8; 32],
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrustedKeyFile {
    schema_version: u32,
    #[serde(default)]
    keys: Vec<TrustedKeyEntry>,
    #[serde(default)]
    revoked_key_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrustedKeyEntry {
    key_id: String,
    public_key: String,
}

#[derive(Debug, Default, Clone)]
pub struct PluginKeyring {
    trusted: HashMap<String, [u8; 32]>,
    revoked: HashSet<String>,
}

impl PluginKeyring {
    pub fn new(keys: impl IntoIterator<Item = TrustedPluginKey>, revoked: impl IntoIterator<Item = String>) -> Self {
        Self {
            trusted: keys.into_iter().map(|key| (key.key_id, key.public_key)).collect(),
            revoked: revoked.into_iter().collect(),
        }
    }

    pub fn from_json(bytes: &[u8]) -> Result<Self, String> {
        let file: TrustedKeyFile = serde_json::from_slice(bytes)
            .map_err(|error| format!("可信插件密钥文件解析失败: {error}"))?;
        if file.schema_version != 1 {
            return Err("不支持的可信插件密钥文件版本".to_string());
        }
        let mut keys = Vec::with_capacity(file.keys.len());
        for entry in file.keys {
            let decoded = hex::decode(&entry.public_key)
                .map_err(|_| format!("插件公钥必须是十六进制: {}", entry.key_id))?;
            let public_key: [u8; 32] = decoded
                .try_into()
                .map_err(|_| format!("插件公钥长度无效: {}", entry.key_id))?;
            keys.push(TrustedPluginKey {
                key_id: entry.key_id,
                public_key,
            });
        }
        Ok(Self::new(keys, file.revoked_key_ids))
    }

    pub fn verify(&self, key_id: &str, payload: &[u8], signature: &[u8]) -> Result<(), String> {
        if self.revoked.contains(key_id) {
            return Err(format!("插件签名密钥已撤销: {key_id}"));
        }
        let public_key = self
            .trusted
            .get(key_id)
            .ok_or_else(|| format!("插件签名密钥不受信任: {key_id}"))?;
        let key = VerifyingKey::from_bytes(public_key).map_err(|_| "插件公钥格式无效".to_string())?;
        let signature = Signature::from_slice(signature).map_err(|_| "插件签名格式无效".to_string())?;
        key.verify(payload, &signature)
            .map_err(|_| "插件签名校验失败".to_string())
    }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

pub fn canonical_signature_payload(
    plugin_id: &str,
    version: &str,
    plugin_json_sha256: &str,
    integrity_json_sha256: &str,
) -> Result<Vec<u8>, String> {
    let mut fields = BTreeMap::new();
    fields.insert("integritySha256", integrity_json_sha256);
    fields.insert("pluginId", plugin_id);
    fields.insert("pluginJsonSha256", plugin_json_sha256);
    fields.insert("version", version);
    serde_json::to_vec(&fields).map_err(|error| format!("签名输入生成失败: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    #[test]
    fn canonical_payload_is_repeatable_and_sorted() {
        let payload = canonical_signature_payload("com.lanchat.gomoku", "1.0.0", "aa", "bb")
            .expect("payload");
        assert_eq!(
            String::from_utf8(payload).unwrap(),
            r#"{"integritySha256":"bb","pluginId":"com.lanchat.gomoku","pluginJsonSha256":"aa","version":"1.0.0"}"#
        );
    }

    #[test]
    fn verifies_trusted_key_and_rejects_revoked_key() {
        let signing = SigningKey::from_bytes(&[23; 32]);
        let payload = b"signed plugin";
        let signature = signing.sign(payload).to_bytes();
        let trusted = TrustedPluginKey {
            key_id: "official-2026".to_string(),
            public_key: signing.verifying_key().to_bytes(),
        };

        let keyring = PluginKeyring::new([trusted.clone()], []);
        assert!(keyring.verify("official-2026", payload, &signature).is_ok());
        assert!(keyring.verify("unknown", payload, &signature).is_err());

        let revoked = PluginKeyring::new([trusted], ["official-2026".to_string()]);
        assert!(revoked.verify("official-2026", payload, &signature).is_err());
    }

    #[test]
    fn loads_versioned_keyring_file() {
        let signing = SigningKey::from_bytes(&[11; 32]);
        let json = serde_json::to_vec(&serde_json::json!({
            "schemaVersion": 1,
            "keys": [{
                "keyId": "official-test",
                "publicKey": hex::encode(signing.verifying_key().to_bytes())
            }],
            "revokedKeyIds": []
        }))
        .unwrap();
        let keyring = PluginKeyring::from_json(&json).expect("keyring loads");
        let payload = b"plugin";
        assert!(keyring
            .verify("official-test", payload, &signing.sign(payload).to_bytes())
            .is_ok());
    }
}
