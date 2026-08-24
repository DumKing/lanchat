//! 模型包信任边界与本地缓存淘汰策略。
//!
//! 远程来源必须由官方签名 Catalog 解析；本地导入刻意保持可用但永久标为
//! `unsigned_local`，因此不能被远程管理员策略强制启用。

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelPackageSource {
    Bundled,
    OfficialCatalog,
    GithubRelease,
    Mirror,
    LocalImport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSignature {
    pub key_id: String,
    pub signature_hex: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignedCatalog {
    pub catalog: serde_json::Value,
    pub signature: CatalogSignature,
}

/// 根密钥、经根密钥签发的轮换密钥和吊销列表均可离线缓存。
#[derive(Debug, Clone, Default)]
pub struct TrustedKeyRing {
    keys: BTreeMap<String, String>,
    revoked_key_ids: BTreeSet<String>,
}

impl TrustedKeyRing {
    pub fn new(root_key_id: impl Into<String>, root_public_key_hex: impl Into<String>) -> Self {
        let mut keys = BTreeMap::new();
        keys.insert(root_key_id.into(), root_public_key_hex.into());
        Self {
            keys,
            revoked_key_ids: BTreeSet::new(),
        }
    }

    pub fn trust_rotated_key(
        &mut self,
        key_id: impl Into<String>,
        public_key_hex: impl Into<String>,
    ) {
        self.keys.insert(key_id.into(), public_key_hex.into());
    }

    pub fn revoke(&mut self, key_id: impl Into<String>) {
        self.revoked_key_ids.insert(key_id.into());
    }
}

pub fn verify_catalog(catalog: &SignedCatalog, key_ring: &TrustedKeyRing) -> Result<(), String> {
    if key_ring.revoked_key_ids.contains(&catalog.signature.key_id) {
        return Err("VISION_CATALOG_KEY_REVOKED".to_string());
    }
    let Some(public_key_hex) = key_ring.keys.get(&catalog.signature.key_id) else {
        return Err("VISION_CATALOG_KEY_UNKNOWN".to_string());
    };
    let public_key = decode_fixed::<32>(public_key_hex, "VISION_CATALOG_KEY_INVALID")?;
    let signature = decode_fixed::<64>(
        &catalog.signature.signature_hex,
        "VISION_CATALOG_SIGNATURE_INVALID",
    )?;
    let payload = serde_json::to_vec(&catalog.catalog)
        .map_err(|_| "VISION_CATALOG_PAYLOAD_INVALID".to_string())?;
    VerifyingKey::from_bytes(&public_key)
        .map_err(|_| "VISION_CATALOG_KEY_INVALID".to_string())?
        .verify(&payload, &Signature::from_bytes(&signature))
        .map_err(|_| "VISION_CATALOG_SIGNATURE_INVALID".to_string())
}

/// 返回 UI/存储使用的信任状态标识。
pub fn verify_package_source(
    source: ModelPackageSource,
    signature_verified: Option<bool>,
) -> Result<&'static str, String> {
    match source {
        ModelPackageSource::Bundled => Ok("bundled"),
        ModelPackageSource::LocalImport => Ok("unsigned_local"),
        ModelPackageSource::OfficialCatalog
        | ModelPackageSource::GithubRelease
        | ModelPackageSource::Mirror => match signature_verified {
            Some(true) => Ok("verified"),
            _ => Err("VISION_PACKAGE_SIGNATURE_REQUIRED".to_string()),
        },
    }
}

/// 官方和本地包均为数据包，禁止携带代码、动态库或路径穿越条目。
pub fn validate_package_entries(entries: &[impl AsRef<str>]) -> Result<(), String> {
    for entry in entries {
        let entry = entry.as_ref().replace('\\', "/");
        if entry.is_empty()
            || entry.starts_with('/')
            || entry
                .split('/')
                .any(|segment| segment == ".." || segment.is_empty())
        {
            return Err("VISION_PACKAGE_ENTRY_INVALID".to_string());
        }
        let lower = entry.to_ascii_lowercase();
        if [
            ".exe", ".dll", ".so", ".dylib", ".wasm", ".bat", ".cmd", ".ps1",
        ]
        .iter()
        .any(|extension| lower.ends_with(extension))
        {
            return Err("VISION_PACKAGE_ENTRY_FORBIDDEN".to_string());
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelCacheEntry {
    pub profile_id: String,
    pub bytes: u64,
    pub last_accessed_at: i64,
    pub baseline: bool,
    pub active: bool,
    pub last_known_good: bool,
}

impl ModelCacheEntry {
    pub fn new(profile_id: impl Into<String>, bytes: u64, last_accessed_at: i64) -> Self {
        Self {
            profile_id: profile_id.into(),
            bytes,
            last_accessed_at,
            baseline: false,
            active: false,
            last_known_good: false,
        }
    }

    pub fn baseline(mut self) -> Self {
        self.baseline = true;
        self
    }

    pub fn active(mut self) -> Self {
        self.active = true;
        self
    }

    pub fn last_known_good(mut self) -> Self {
        self.last_known_good = true;
        self
    }

    fn protected(&self) -> bool {
        self.baseline || self.active || self.last_known_good
    }
}

/// 返回按 LRU 顺序应删除的 Profile；调用方可逐个删除，直到满足缓存上限。
pub fn eviction_candidates(entries: &[ModelCacheEntry], max_bytes: u64) -> Vec<String> {
    let total_bytes = entries.iter().map(|entry| entry.bytes).sum::<u64>();
    if total_bytes <= max_bytes {
        return Vec::new();
    }
    let mut reclaimable = entries
        .iter()
        .filter(|entry| !entry.protected())
        .collect::<Vec<_>>();
    reclaimable.sort_by_key(|entry| (entry.last_accessed_at, entry.profile_id.as_str()));
    let mut remaining = total_bytes;
    let mut result = Vec::new();
    for entry in reclaimable {
        if remaining <= max_bytes {
            break;
        }
        remaining = remaining.saturating_sub(entry.bytes);
        result.push(entry.profile_id.clone());
    }
    result
}

fn decode_fixed<const N: usize>(value: &str, error_code: &str) -> Result<[u8; N], String> {
    let bytes = hex::decode(value).map_err(|_| error_code.to_string())?;
    bytes.try_into().map_err(|_| error_code.to_string())
}
