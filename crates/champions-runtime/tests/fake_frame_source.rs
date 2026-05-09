use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use champions_interface::FrameSequence;
use champions_runtime::{
    CaptureError, CapturedFrame, FrameSource, ImageBuffer, PixelFormat, PreviewFrame,
    PreviewFrameConverter,
};

pub struct FakeFrameSource {
    width: u32,
    height: u32,
    frames_remaining: u32,
}

impl FakeFrameSource {
    pub fn new(width: u32, height: u32, frame_count: u32) -> Self {
        Self {
            width,
            height,
            frames_remaining: frame_count,
        }
    }
}

impl FrameSource for FakeFrameSource {
    fn read_frame(&mut self) -> Result<Option<CapturedFrame>, CaptureError> {
        if self.frames_remaining == 0 {
            return Ok(None);
        }
        self.frames_remaining -= 1;

        let pixel_count = (self.width * self.height * 3) as usize;
        let bytes: Arc<[u8]> = vec![128u8; pixel_count].into();

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        Ok(Some(CapturedFrame {
            frame_sequence: FrameSequence(0),
            captured_at_millis: now,
            image: ImageBuffer {
                width: self.width,
                height: self.height,
                pixel_format: PixelFormat::Bgr8,
                bytes,
            },
        }))
    }
}

pub struct FakePreviewConverter;

impl PreviewFrameConverter for FakePreviewConverter {
    fn convert(&self, frame: &CapturedFrame, max_width: u32) -> PreviewFrame {
        let scale = if frame.image.width > max_width {
            max_width as f64 / frame.image.width as f64
        } else {
            1.0
        };
        let out_w = (frame.image.width as f64 * scale) as u32;
        let out_h = (frame.image.height as f64 * scale) as u32;
        let pixel_count = (out_w * out_h * 4) as usize;
        let rgba: Arc<[u8]> = vec![200u8; pixel_count].into();

        PreviewFrame {
            frame_sequence: frame.frame_sequence,
            timestamp_millis: frame.captured_at_millis,
            width: out_w,
            height: out_h,
            rgba,
        }
    }
}
