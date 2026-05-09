use crate::frame::{CapturedFrame, PreviewFrame};

#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error("capture device not found")]
    DeviceNotFound,
    #[error("read failed: {0}")]
    ReadFailed(String),
}

pub trait FrameSource: Send {
    fn read_frame(&mut self) -> Result<Option<CapturedFrame>, CaptureError>;
}

pub trait PreviewFrameConverter: Send {
    fn convert(&self, frame: &CapturedFrame, max_width: u32) -> PreviewFrame;
}
