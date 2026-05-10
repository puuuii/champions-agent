use crate::recognition_view::OpponentPartyView;
use crate::types::{
    CaptureStatus, EventSequence, FrameSequence, RecognitionAttemptId, RecognitionStatus,
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
    BattleResultPhaseChanged {
        event_sequence: EventSequence,
        is_battle_result_phase: bool,
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
