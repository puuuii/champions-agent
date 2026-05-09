use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FrameSequence(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EventSequence(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RecognitionAttemptId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Bgr8,
    Rgb8,
    Rgba8,
    Gray8,
}

impl PixelFormat {
    pub fn bytes_per_pixel(&self) -> usize {
        match self {
            Self::Bgr8 | Self::Rgb8 => 3,
            Self::Rgba8 => 4,
            Self::Gray8 => 1,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ImageBuffer {
    pub width: u32,
    pub height: u32,
    pub pixel_format: PixelFormat,
    pub bytes: Arc<[u8]>,
}

#[derive(Debug, Clone)]
pub struct CapturedFrame {
    pub frame_sequence: FrameSequence,
    pub captured_at_millis: u64,
    pub image: ImageBuffer,
}

#[derive(Debug, Clone)]
pub struct PreviewFrame {
    pub frame_sequence: FrameSequence,
    pub timestamp_millis: u64,
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<[u8]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureStatus {
    Idle,
    Running,
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecognitionStatus {
    Idle,
    Running,
    Stopped,
}
