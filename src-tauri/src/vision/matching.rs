//! 多参考图身份匹配：质量加权原型与 Top-K 一致性，而不是取单张最高分。

use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct ReferenceEmbedding {
    pub person_id: String,
    pub vector: Vec<f32>,
    pub quality_weight: f32,
}

impl ReferenceEmbedding {
    pub fn new(person_id: impl Into<String>, vector: Vec<f32>, quality_weight: f32) -> Self {
        Self {
            person_id: person_id.into(),
            vector,
            quality_weight: quality_weight.clamp(0.05, 1.0),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct IdentityMatch {
    pub person_id: String,
    pub raw_similarity: f32,
    pub normalized_match_score: f32,
    pub second_best_similarity: Option<f32>,
    pub margin: Option<f32>,
    pub supporting_samples: usize,
}

pub fn match_identity(
    probe: &[f32],
    references: &[ReferenceEmbedding],
    top_k: usize,
    min_normalized_score: f32,
    min_normalized_margin: f32,
) -> Option<IdentityMatch> {
    let mut per_person = BTreeMap::<&str, Vec<(f32, f32)>>::new();
    for reference in references {
        let similarity = cosine_similarity(probe, &reference.vector)?;
        per_person
            .entry(&reference.person_id)
            .or_default()
            .push((similarity, reference.quality_weight));
    }
    let top_k = top_k.max(1);
    let mut candidates = per_person
        .into_iter()
        .filter_map(|(person_id, mut samples)| {
            samples.sort_by(|left, right| {
                right
                    .0
                    .partial_cmp(&left.0)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            let selected = samples.into_iter().take(top_k).collect::<Vec<_>>();
            let total_weight = selected.iter().map(|(_, weight)| weight).sum::<f32>();
            (total_weight > f32::EPSILON).then(|| {
                let raw_similarity = selected
                    .iter()
                    .map(|(score, weight)| score * weight)
                    .sum::<f32>()
                    / total_weight;
                (person_id.to_string(), raw_similarity, selected.len())
            })
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let (person_id, raw_similarity, supporting_samples) = candidates.first()?.clone();
    let normalized_match_score = normalize_similarity(raw_similarity);
    let second_best_similarity = candidates.get(1).map(|candidate| candidate.1);
    let margin =
        second_best_similarity.map(|score| normalized_match_score - normalize_similarity(score));
    if normalized_match_score < min_normalized_score
        || margin.is_some_and(|value| value < min_normalized_margin)
    {
        return None;
    }
    Some(IdentityMatch {
        person_id,
        raw_similarity,
        normalized_match_score,
        second_best_similarity,
        margin,
        supporting_samples,
    })
}

/// 这是模型分数的统一展示刻度，不是“识别概率”。阈值永远按 EmbeddingSpace 保存。
pub fn normalize_similarity(raw_similarity: f32) -> f32 {
    ((raw_similarity.clamp(-1.0, 1.0) + 1.0) * 50.0 * 10.0).round() / 10.0
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> Option<f32> {
    if left.is_empty() || left.len() != right.len() {
        return None;
    }
    let mut dot = 0.0;
    let mut left_norm = 0.0;
    let mut right_norm = 0.0;
    for (left, right) in left.iter().zip(right) {
        dot += left * right;
        left_norm += left * left;
        right_norm += right * right;
    }
    let denominator = left_norm.sqrt() * right_norm.sqrt();
    (denominator > f32::EPSILON).then_some(dot / denominator)
}
