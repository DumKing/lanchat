//! 单 Worker 的最新帧邮箱。模型推理接入后只从该邮箱取帧。
//!
//! 前端只提交 `LCVF` Raw RGBA 信封，不允许把 PNG/JPEG/Base64 或 JSON 数组
//! 送入命令层。命令处理只负责校验并写入容量为 1 的邮箱。

use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, RgbaImage};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;
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

/// 兼容旧识别器的短期桥接：Raw RGBA 只在专用 Worker 内编码为 JPEG，
/// 不会回传 WebView，也不会保存到数据库或在局域网上发送。
pub fn encode_frame_as_jpeg(frame: &VisionFrame) -> Result<Vec<u8>, String> {
    let packed_stride = usize::from(frame.width) * 4;
    let source_stride =
        usize::try_from(frame.stride).map_err(|_| "VISION_FRAME_LENGTH_INVALID".to_string())?;
    let height = usize::from(frame.height);
    let pixels = if source_stride == packed_stride {
        frame.rgba.clone()
    } else {
        let mut packed = Vec::with_capacity(packed_stride * height);
        for row in frame.rgba.chunks_exact(source_stride).take(height) {
            packed.extend_from_slice(&row[..packed_stride]);
        }
        packed
    };
    let image = RgbaImage::from_raw(u32::from(frame.width), u32::from(frame.height), pixels)
        .ok_or_else(|| "VISION_FRAME_LENGTH_INVALID".to_string())?;
    let mut encoded = Vec::new();
    JpegEncoder::new_with_quality(&mut encoded, 78)
        .encode_image(&DynamicImage::ImageRgba8(image))
        .map_err(|error| format!("VISION_FRAME_JPEG_ENCODE_FAILED:{error}"))?;
    Ok(encoded)
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
pub struct LatestFrameMailbox {
    state: Mutex<MailboxState>,
    signal: Condvar,
}

impl Default for LatestFrameMailbox {
    fn default() -> Self {
        Self {
            state: Mutex::new(MailboxState::default()),
            signal: Condvar::new(),
        }
    }
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
        self.signal.notify_one();
    }

    pub fn take(&self) -> Option<VisionFrame> {
        self.state
            .lock()
            .expect("vision mailbox lock poisoned")
            .latest
            .take()
    }

    pub fn wait_take(&self, timeout: Duration) -> Option<VisionFrame> {
        let state = self.state.lock().expect("vision mailbox lock poisoned");
        let mut state = if state.latest.is_some() {
            state
        } else {
            self.signal
                .wait_timeout(state, timeout)
                .expect("vision mailbox lock poisoned")
                .0
        };
        state.latest.take()
    }

    pub fn wake_all(&self) {
        self.signal.notify_all();
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

    pub fn pending_frame_bytes(&self) -> u64 {
        self.state
            .lock()
            .expect("vision mailbox lock poisoned")
            .latest
            .as_ref()
            .map(|frame| frame.rgba.len() as u64)
            .unwrap_or(0)
    }

    pub fn queue_depth(&self) -> u8 {
        u8::from(
            self.state
                .lock()
                .expect("vision mailbox lock poisoned")
                .latest
                .is_some(),
        )
    }
}

/// 视觉运行时唯一的 CPU Worker。命令线程只写入邮箱；模型推理、兼容编码和
/// 后续跟踪都在该线程顺序执行，因此永远不会堆积多个旧画面。
pub struct VisionWorker {
    mailbox: Arc<LatestFrameMailbox>,
    shutdown_requested: Arc<AtomicBool>,
    handle: Mutex<Option<JoinHandle<()>>>,
}

impl VisionWorker {
    pub fn start<F>(mailbox: Arc<LatestFrameMailbox>, processor: F) -> Self
    where
        F: FnMut(VisionFrame) + Send + 'static,
    {
        let worker_mailbox = mailbox.clone();
        let shutdown_requested = Arc::new(AtomicBool::new(false));
        let worker_shutdown = shutdown_requested.clone();
        let handle = std::thread::Builder::new()
            .name("lanchat-vision-worker".to_string())
            .spawn(move || {
                let mut processor = processor;
                while !worker_shutdown.load(Ordering::Acquire) {
                    if let Some(frame) = worker_mailbox.wait_take(Duration::from_millis(200)) {
                        if !worker_shutdown.load(Ordering::Acquire) {
                            processor(frame);
                        }
                    }
                }
            })
            .expect("start vision worker thread");
        Self {
            mailbox,
            shutdown_requested,
            handle: Mutex::new(Some(handle)),
        }
    }

    pub fn shutdown(&self) {
        self.shutdown_requested.store(true, Ordering::Release);
        self.mailbox.wake_all();
        if let Some(handle) = self
            .handle
            .lock()
            .expect("vision worker handle lock poisoned")
            .take()
        {
            let _ = handle.join();
        }
    }
}

impl Drop for VisionWorker {
    fn drop(&mut self) {
        self.shutdown();
    }
}
