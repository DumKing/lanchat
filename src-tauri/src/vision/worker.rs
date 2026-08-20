//! 单 Worker 的最新帧邮箱。模型推理接入后只从该邮箱取帧。

use std::sync::Mutex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisionFrame {
    pub stream_id: String,
    pub stream_generation: u64,
    pub frame_id: u64,
    pub captured_at_ms: i64,
    pub width: u16,
    pub height: u16,
    pub rgba: Vec<u8>,
}

#[derive(Default)]
struct MailboxState {
    active_stream: Option<(String, u64)>,
    latest: Option<VisionFrame>,
    dropped_frames: u64,
    stream_resets: u64,
}

/// 容量固定为 1：处理慢时宁可跳过旧帧，也不允许 UI/摄像头生产方积压。
#[derive(Default)]
pub struct LatestFrameMailbox {
    state: Mutex<MailboxState>,
}

impl LatestFrameMailbox {
    pub fn submit(&self, frame: VisionFrame) {
        let mut state = self.state.lock().expect("vision mailbox lock poisoned");
        let next_stream = (frame.stream_id.clone(), frame.stream_generation);
        if state.active_stream.as_ref() != Some(&next_stream) {
            if state.active_stream.is_some() {
                state.stream_resets += 1;
            }
            if state.latest.take().is_some() {
                state.dropped_frames += 1;
            }
            state.active_stream = Some(next_stream);
        }
        if state.latest.replace(frame).is_some() {
            state.dropped_frames += 1;
        }
    }

    pub fn take(&self) -> Option<VisionFrame> {
        self.state
            .lock()
            .expect("vision mailbox lock poisoned")
            .latest
            .take()
    }

    pub fn dropped_frames(&self) -> u64 {
        self.state
            .lock()
            .expect("vision mailbox lock poisoned")
            .dropped_frames
    }

    pub fn stream_reset_count(&self) -> u64 {
        self.state
            .lock()
            .expect("vision mailbox lock poisoned")
            .stream_resets
    }
}
