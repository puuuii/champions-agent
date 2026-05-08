use crate::types::{FrameSequence, ImagePoint, ImageRect};

#[derive(Debug, Clone)]
pub enum RuntimeCommand {
    StartCapture,
    StopCapture,
    StartRecognition,
    StopRecognition,
    SetPreviewEnabled(bool),
    SetPreviewTargetFps(u8),
    SetPreviewMaxWidth(u32),
    SetCropRegion(ImageRect),
    SamplePixel {
        frame_sequence: FrameSequence,
        point: ImagePoint,
    },
    SaveDebugSnapshot {
        frame_sequence: FrameSequence,
    },
    Shutdown,
}
