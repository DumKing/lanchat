//! 仅内存存在的轻量人物 Track。
//!
//! Track 不记录行走轨迹，只用于在短时间窗口内把同一人的人脸和人体证据合并。

use crate::vision::types::VisionModality;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundingBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl BoundingBox {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    fn center(self) -> (f32, f32) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    fn iou(self, other: Self) -> f32 {
        let left = self.x.max(other.x);
        let top = self.y.max(other.y);
        let right = (self.x + self.width).min(other.x + other.width);
        let bottom = (self.y + self.height).min(other.y + other.height);
        let intersection = (right - left).max(0.0) * (bottom - top).max(0.0);
        let union = self.width * self.height + other.width * other.height - intersection;
        if union <= f32::EPSILON {
            0.0
        } else {
            intersection / union
        }
    }

    fn center_distance(self, other: Self) -> f32 {
        let (x1, y1) = self.center();
        let (x2, y2) = other.center();
        ((x1 - x2).powi(2) + (y1 - y2).powi(2)).sqrt()
    }
}

#[derive(Debug, Clone)]
pub struct Detection {
    pub modality: VisionModality,
    pub bounds: BoundingBox,
}

impl Detection {
    pub fn new(modality: VisionModality, bounds: BoundingBox) -> Self {
        Self { modality, bounds }
    }
}

#[derive(Debug, Clone)]
struct Track {
    id: String,
    bounds: BoundingBox,
    last_seen_at: i64,
}

pub struct TrackStore {
    ttl_ms: i64,
    next_id: u64,
    tracks: Vec<Track>,
}

impl TrackStore {
    pub fn new(ttl_ms: i64) -> Self {
        Self {
            ttl_ms: ttl_ms.max(1),
            next_id: 1,
            tracks: Vec::new(),
        }
    }

    /// 返回当前检测归属的 Track。不同模态使用较宽松的中心距离，方便把同一人的
    /// 小人脸框和大人体框合并；同模态仍优先使用 IoU。
    pub fn observe(&mut self, detection: Detection, observed_at: i64) -> String {
        self.tracks
            .retain(|track| observed_at.saturating_sub(track.last_seen_at) <= self.ttl_ms);
        let matched = self
            .tracks
            .iter_mut()
            .filter(|track| {
                track.bounds.iou(detection.bounds) >= 0.20
                    || track.bounds.center_distance(detection.bounds) <= 0.18
            })
            .min_by(|left, right| {
                left.bounds
                    .center_distance(detection.bounds)
                    .partial_cmp(&right.bounds.center_distance(detection.bounds))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        if let Some(track) = matched {
            track.bounds = detection.bounds;
            track.last_seen_at = observed_at;
            return track.id.clone();
        }
        let id = format!("track-{}", self.next_id);
        self.next_id += 1;
        self.tracks.push(Track {
            id: id.clone(),
            bounds: detection.bounds,
            last_seen_at: observed_at,
        });
        id
    }

    pub fn reset(&mut self) {
        self.tracks.clear();
    }
}
