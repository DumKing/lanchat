//! 同 Track 多模态证据融合与告警分发边界。

use crate::vision::types::IdentityDecision;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertDispatch {
    LocalOnly,
    LanAndLocal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FusedAlertDecision {
    pub person_id: Option<String>,
    pub decision: IdentityDecision,
    pub dispatch: AlertDispatch,
}

/// 仅强人脸或同一身份的人脸+人体融合可自动广播；人体单独命中永远保留在本机。
pub fn fuse_track_evidence(
    face: Option<(&str, u8)>,
    body: Option<(&str, u8)>,
) -> FusedAlertDecision {
    const FACE_CONFIRM_SCORE: u8 = 75;
    const BODY_FUSION_SCORE: u8 = 70;
    match (face, body) {
        (Some((face_person, face_score)), Some((body_person, body_score)))
            if face_person == body_person
                && face_score >= FACE_CONFIRM_SCORE
                && body_score >= BODY_FUSION_SCORE =>
        {
            FusedAlertDecision {
                person_id: Some(face_person.to_string()),
                decision: IdentityDecision::ConfirmedFusion,
                dispatch: AlertDispatch::LanAndLocal,
            }
        }
        (Some((face_person, face_score)), _) if face_score >= FACE_CONFIRM_SCORE => {
            FusedAlertDecision {
                person_id: Some(face_person.to_string()),
                decision: IdentityDecision::ConfirmedFace,
                dispatch: AlertDispatch::LanAndLocal,
            }
        }
        (_, Some((body_person, _))) => FusedAlertDecision {
            person_id: Some(body_person.to_string()),
            decision: IdentityDecision::ProbableBody,
            dispatch: AlertDispatch::LocalOnly,
        },
        _ => FusedAlertDecision {
            person_id: None,
            decision: IdentityDecision::Unknown,
            dispatch: AlertDispatch::LocalOnly,
        },
    }
}
