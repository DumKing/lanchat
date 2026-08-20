use super::worker::{LatestFrameMailbox, VisionFrame};

fn raw_frame(stream_id: uuid::Uuid, generation: u64, width: u16, height: u16) -> Vec<u8> {
    let stride = u32::from(width) * 4;
    let payload_len = usize::try_from(stride).unwrap() * usize::from(height);
    let mut bytes = Vec::with_capacity(60 + payload_len);
    bytes.extend_from_slice(b"LCVF");
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.push(1); // RGBA8
    bytes.push(0);
    bytes.extend_from_slice(stream_id.as_bytes());
    bytes.extend_from_slice(&generation.to_le_bytes());
    bytes.extend_from_slice(&7u64.to_le_bytes());
    bytes.extend_from_slice(&1234i64.to_le_bytes());
    bytes.extend_from_slice(&width.to_le_bytes());
    bytes.extend_from_slice(&height.to_le_bytes());
    bytes.extend_from_slice(&stride.to_le_bytes());
    bytes.extend_from_slice(&(payload_len as u32).to_le_bytes());
    bytes.resize(60 + payload_len, 255);
    bytes
}

fn frame(stream_id: &str, generation: u64, frame_id: u64) -> VisionFrame {
    VisionFrame {
        stream_id: stream_id.to_string(),
        stream_generation: generation,
        frame_id,
        captured_at_ms: 1,
        width: 2,
        height: 2,
        stride: 8,
        rgba: vec![0; 16],
    }
}

#[test]
fn mailbox_replaces_stale_frame_and_counts_drop() {
    let mailbox = LatestFrameMailbox::default();
    mailbox.submit(frame("stream-a", 1, 1));
    mailbox.submit(frame("stream-a", 1, 2));

    assert_eq!(mailbox.take().expect("latest frame").frame_id, 2);
    assert_eq!(mailbox.dropped_frames(), 1);
}

#[test]
fn new_stream_discards_previous_mailbox_contents() {
    let mailbox = LatestFrameMailbox::default();
    mailbox.submit(frame("stream-a", 1, 1));
    mailbox.submit(frame("stream-b", 2, 1));

    let latest = mailbox.take().expect("new stream frame");
    assert_eq!(latest.stream_id, "stream-b");
    assert_eq!(mailbox.stream_reset_count(), 1);
}

#[test]
fn raw_envelope_decodes_the_stream_identity_and_rgba_payload() {
    let stream_id = uuid::Uuid::new_v4();
    let frame =
        super::worker::decode_raw_frame(&raw_frame(stream_id, 3, 2, 4)).expect("valid raw frame");

    assert_eq!(frame.stream_id, stream_id.to_string());
    assert_eq!(frame.stream_generation, 3);
    assert_eq!(frame.width, 2);
    assert_eq!(frame.height, 4);
    assert_eq!(frame.stride, 8);
    assert_eq!(frame.rgba.len(), 32);
}

#[test]
fn raw_envelope_rejects_mismatched_payload_length() {
    let stream_id = uuid::Uuid::new_v4();
    let mut bytes = raw_frame(stream_id, 1, 2, 2);
    bytes[56..60].copy_from_slice(&999u32.to_le_bytes());

    assert_eq!(
        super::worker::decode_raw_frame(&bytes).unwrap_err(),
        "VISION_FRAME_LENGTH_INVALID"
    );
}
