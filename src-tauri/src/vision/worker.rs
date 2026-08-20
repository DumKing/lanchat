//! 单 Worker 的最新帧邮箱。模型推理接入后只从该邮箱取帧。
//!
//! 前端只提交 `LCVF` Raw RGBA 信封，不允许把 PNG/JPEG/Base64 或 JSON 数组
//! 送入命令层。命令处理只负责校验并写入容量为 1 的邮箱。

use std::sync::Mutex;
use uuid::Uuid;

pub const RAW_FRAME_MAGIC: &[u8; 4] = b"LCVF";
pub const RAW_FRAME_SCHEMA_VERSION: u16 = 1;
pub const RAW_FRAME_HEADER_LEN: usize = 60;
const PIXEL_FORMAT_RGBA8: u8 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisionFrame {
    pub stream_id: String,
    pub stream_generation: u64,
    pub frame_id: u64,
    pub captured_at_ms: i64,
    pub width: u16,
    pub height: u16,
    pub stride: u32,
    pub rgba: Vec<u8>,
}

/// 解码固定头部的 Raw Frame Envelope。
///
/// Layout: magic(4), schema(2), pixel_format(1), flags(1), stream UUID(16),
/// generation(8), frame id(8), captured_at(8), width(2), height(2), stride(4),
/// payload length(4), RGBA payload(N).
pub fn decode_raw_frame(bytes: &[u8]) -> Result<VisionFrame, String> {
    if bytes.len() < RAW_FRAME_HEADER_LEN || &bytes[0..4] != RAW_FRAME_MAGIC {
        return Err("VISION_FRAME_HEADER_INVALID".to_string());
    }
    let schema_version = read_u16(bytes, 4)?;
    if schema_version != RAW_FRAME_SCHEMA_VERSION || bytes[6] != PIXEL_FORMAT_RGBA8 {
        return Err("VISION_FRAME_FORMAT_UNSUPPORTED".to_string());
    }
    let stream_bytes: [u8; 16] = bytes[8..24]
        .try_into()
        .map_err(|_| "VISION_FRAME_HEADER_INVALID".to_string())?;
    let stream_id = Uuid::from_bytes(stream_bytes).to_string();
    let stream_generation = read_u64(bytes, 24)?;
    let frame_id = read_u64(bytes, 32)?;
    let captured_at_ms = read_i64(bytes, 40)?;
    let width = read_u16(bytes, 48)?;
    let height = read_u16(bytes, 50)?;
    let stride = read_u32(bytes, 52)?;
    let payload_len = usize::try_from(read_u32(bytes, 56)?)
        .map_err(|_| "VISION_FRAME_LENGTH_INVALID".to_string())?;
    let expected_payload_len = usize::try_from(stride)
        .ok()
        .and_then(|value| value.checked_mul(usize::from(height)))
        .ok_or_else(|| "VISION_FRAME_LENGTH_INVALID".to_string())?;
    if width == 0
        || height == 0
        || stride < u32::from(width) * 4
        || payload_len != expected_payload_len
        || bytes.len() != RAW_FRAME_HEADER_LEN + payload_len
    {
        return Err("VISION_FRAME_LENGTH_INVALID".to_string());
    }
    Ok(VisionFrame {
        stream_id,
        stream_generation,
        frame_id,
        captured_at_ms,
        width,
        height,
        stride,
        rgba: bytes[RAW_FRAME_HEADER_LEN..].to_vec(),
    })
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, String> {
    bytes
        .get(offset..offset + 2)
        .and_then(|value| value.try_into().ok())
        .map(u16::from_le_bytes)
        .ok_or_else(|| "VISION_FRAME_HEADER_INVALID".to_string())
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    bytes
        .get(offset..offset + 4)
        .and_then(|value| value.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| "VISION_FRAME_HEADER_INVALID".to_string())
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, String> {
    bytes
        .get(offset..offset + 8)
        .and_then(|value| value.try_into().ok())
        .map(u64::from_le_bytes)
        .ok_or_else(|| "VISION_FRAME_HEADER_INVALID".to_string())
}

fn read_i64(bytes: &[u8], offset: usize) -> Result<i64, String> {
    bytes
        .get(offset..offset + 8)
        .and_then(|value| value.try_into().ok())
        .map(i64::from_le_bytes)
        .ok_or_else(|| "VISION_FRAME_HEADER_INVALID".to_string())
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
