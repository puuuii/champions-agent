use crate::recognition_view::OpponentPartyView;
use crate::types::{
    CaptureStatus, EventSequence, FrameSequence, RecognitionAttemptId, RecognitionStatus, RgbaColor,
};

#[derive(Debug, Clone)]
pub enum RuntimeEvent {
    CaptureStatusChanged {
        event_sequence: EventSequence,
        status: CaptureStatus,
    },
    RecognitionStatusChanged {
        event_sequence: EventSequence,
        status: RecognitionStatus,
    },
    OpponentPartyRecognized {
        event_sequence: EventSequence,
        frame_sequence: FrameSequence,
        attempt_id: RecognitionAttemptId,
        party: OpponentPartyView,
    },
    PixelSampled {
        event_sequence: EventSequence,
        frame_sequence: FrameSequence,
        color: RgbaColor,
    },
    Error {
        event_sequence: EventSequence,
        error: RuntimeError,
    },
    RuntimeStopped {
        event_sequence: EventSequence,
    },
}

#[derive(Debug, Clone)]
pub enum RuntimeError {
    CaptureDeviceNotFound,
    CaptureReadFailed(String),
    RecognitionFailed(String),
    Internal(String),
}
