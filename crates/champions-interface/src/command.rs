use crate::event::MatchPhase;

#[derive(Debug, Clone)]
pub enum RuntimeCommand {
    StartCapture,
    StopCapture,
    StartRecognition,
    StopRecognition,
    ScanOpponentSelection,
    SetMatchPhase(MatchPhase),
    SetPreviewEnabled(bool),
    SetPreviewTargetFps(u8),
    SetPreviewMaxWidth(u32),
    Shutdown,
}
