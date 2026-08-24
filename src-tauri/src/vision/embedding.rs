//! Embedding Space 是模型家族与预处理契约的边界。
//! 不同 Space 的向量没有可比性，必须在进入匹配层前被隔离。

use super::matching::{match_identity, IdentityMatch, ReferenceEmbedding};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EmbeddingSpaceId(String);

impl EmbeddingSpaceId {
    pub fn new(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into().trim().to_string();
        if value.is_empty()
            || value.len() > 128
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        {
            return Err("VISION_EMBEDDING_SPACE_INVALID".to_string());
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone)]
pub struct ScopedReferenceEmbedding {
    pub person_id: String,
    pub embedding_space_id: EmbeddingSpaceId,
    pub vector: Vec<f32>,
    pub quality_weight: f32,
}

impl ScopedReferenceEmbedding {
    pub fn new(
        person_id: impl Into<String>,
        embedding_space_id: EmbeddingSpaceId,
        vector: Vec<f32>,
        quality_weight: f32,
    ) -> Self {
        Self {
            person_id: person_id.into(),
            embedding_space_id,
            vector,
            quality_weight,
        }
    }
}

pub fn match_identity_in_space(
    embedding_space_id: &EmbeddingSpaceId,
    probe: &[f32],
    references: &[ScopedReferenceEmbedding],
    top_k: usize,
    min_normalized_score: f32,
    min_normalized_margin: f32,
) -> Option<IdentityMatch> {
    let scoped_references = references
        .iter()
        .filter(|reference| &reference.embedding_space_id == embedding_space_id)
        .map(|reference| {
            ReferenceEmbedding::new(
                reference.person_id.clone(),
                reference.vector.clone(),
                reference.quality_weight,
            )
        })
        .collect::<Vec<_>>();
    match_identity(
        probe,
        &scoped_references,
        top_k,
        min_normalized_score,
        min_normalized_margin,
    )
}
