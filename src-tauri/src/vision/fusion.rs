//! 人脸与人体识别结果的质量门控、时序稳定和决策融合。
//!
//! 这里不比较原始向量，输入已经是各自 Embedding Space 内的候选结果；
//! 因此 SFace/ArcFace、OSNet/OMZ 可以独立替换而不破坏融合规则。

use std::collections::{BTreeMap, VecDeque};

use super::alert::AlertDispatch;
use super::types::IdentityDecision;

#[derive(Debug, Clone, PartialEq)]
pub struct FusionEvidence {
    pub person_id: String,
    pub score: f32,
    pub quality: f32,
    pub modality: FusionModality,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FusionModality {
    Face,
    Body,
}

impl FusionEvidence {
    pub fn face(person_id: impl Into<String>, score: f32, quality: f32) -> Self {
        Self {
            person_id: person_id.into(),
            score,
            quality,
            modality: FusionModality::Face,
        }
    }

    pub fn body(person_id: impl Into<String>, score: f32, quality: f32) -> Self {
        Self {
            person_id: person_id.into(),
            score,
            quality,
            modality: FusionModality::Body,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FusionPolicy {
    pub face_min_score: f32,
    pub face_min_quality: f32,
    pub body_min_score: f32,
    pub body_min_quality: f32,
    pub temporal_window_frames: usize,
    pub required_consistent_frames: usize,
}

impl Default for FusionPolicy {
    fn default() -> Self {
        Self {
            face_min_score: 75.0,
            face_min_quality: 0.50,
            body_min_score: 70.0,
            body_min_quality: 0.50,
            temporal_window_frames: 5,
            required_consistent_frames: 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FusedIdentityDecision {
    pub person_id: Option<String>,
    pub decision: IdentityDecision,
    pub dispatch: AlertDispatch,
    pub supporting_frames: usize,
}

#[derive(Debug, Clone)]
struct FrameCandidate {
    frame_id: u64,
    person_id: String,
    decision: IdentityDecision,
}

/// 仅记录极小的 Track 窗口，不保存图像或 Embedding。
#[derive(Debug, Default)]
pub struct TemporalIdentityFusion {
    policy: FusionPolicy,
    tracks: BTreeMap<String, VecDeque<FrameCandidate>>,
}

impl TemporalIdentityFusion {
    pub fn new(policy: FusionPolicy) -> Self {
        Self {
            policy: FusionPolicy {
                temporal_window_frames: policy.temporal_window_frames.clamp(1, 30),
                required_consistent_frames: policy.required_consistent_frames.clamp(1, 30),
                ..policy
            },
            tracks: BTreeMap::new(),
        }
    }

    pub fn observe(
        &mut self,
        track_id: &str,
        frame_id: u64,
        face: Option<FusionEvidence>,
        body: Option<FusionEvidence>,
    ) -> Option<FusedIdentityDecision> {
        let candidate = self.select_candidate(face, body)?;
        let history = self.tracks.entry(track_id.to_string()).or_default();
        if history
            .back()
            .is_some_and(|previous| frame_id <= previous.frame_id)
        {
            return None;
        }
        history.push_back(FrameCandidate {
            frame_id,
            person_id: candidate.person_id.clone(),
            decision: candidate.decision,
        });
        while history.len() > self.policy.temporal_window_frames {
            history.pop_front();
        }

        let supporting_frames = history
            .iter()
            .filter(|entry| {
                entry.person_id == candidate.person_id && entry.decision == candidate.decision
            })
            .count();
        if supporting_frames < self.policy.required_consistent_frames {
            return None;
        }
        Some(FusedIdentityDecision {
            person_id: Some(candidate.person_id),
            decision: candidate.decision,
            dispatch: match candidate.decision {
                IdentityDecision::ConfirmedFace | IdentityDecision::ConfirmedFusion => {
                    AlertDispatch::LanAndLocal
                }
                IdentityDecision::ProbableBody | IdentityDecision::Unknown => {
                    AlertDispatch::LocalOnly
                }
            },
            supporting_frames,
        })
    }

    pub fn clear_track(&mut self, track_id: &str) {
        self.tracks.remove(track_id);
    }

    /// 摄像头流切换或当前帧完全无候选时清除短时窗口，避免旧画面残留的
    /// 身份证据参与下一段画面。
    pub fn reset(&mut self) {
        self.tracks.clear();
    }

    fn select_candidate(
        &self,
        face: Option<FusionEvidence>,
        body: Option<FusionEvidence>,
    ) -> Option<FrameCandidate> {
        let face = face.filter(|evidence| {
            evidence.modality == FusionModality::Face
                && evidence.score >= self.policy.face_min_score
                && evidence.quality >= self.policy.face_min_quality
        });
        let body = body.filter(|evidence| {
            evidence.modality == FusionModality::Body
                && evidence.score >= self.policy.body_min_score
                && evidence.quality >= self.policy.body_min_quality
        });
        match (face, body) {
            (Some(face), Some(body)) if face.person_id == body.person_id => Some(FrameCandidate {
                frame_id: 0,
                person_id: face.person_id,
                decision: IdentityDecision::ConfirmedFusion,
            }),
            (Some(face), _) => Some(FrameCandidate {
                frame_id: 0,
                person_id: face.person_id,
                decision: IdentityDecision::ConfirmedFace,
            }),
            (_, Some(body)) => Some(FrameCandidate {
                frame_id: 0,
                person_id: body.person_id,
                decision: IdentityDecision::ProbableBody,
            }),
            _ => None,
        }
    }
}
