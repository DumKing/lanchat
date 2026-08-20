use super::worker::{LatestFrameMailbox, VisionFrame};

fn frame(stream_id: &str, generation: u64, frame_id: u64) -> VisionFrame {
    VisionFrame { stream_id: stream_id.to_string(), stream_generation: generation, frame_id, captured_at_ms: 1, width: 2, height: 2, rgba: vec![0; 16] }
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
