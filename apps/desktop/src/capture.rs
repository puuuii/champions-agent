use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use champions_interface::{CapturedFrame, FrameSequence, ImageBuffer, PixelFormat};
use champions_runtime::traits::{CaptureError, FrameSource};
use opencv::{core, prelude::*, videoio};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureBackend {
    Auto,
    V4l2,
}

impl CaptureBackend {
    fn to_opencv_api(self) -> i32 {
        match self {
            Self::Auto => videoio::CAP_ANY,
            Self::V4l2 => videoio::CAP_V4L2,
        }
    }
}

pub struct CaptureConfig {
    pub device_index: i32,
    pub backend: CaptureBackend,
    pub width: u32,
    pub height: u32,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            device_index: 0,
            backend: CaptureBackend::Auto,
            width: 1920,
            height: 1080,
        }
    }
}

pub struct OpenCvFrameSource {
    capture: videoio::VideoCapture,
    frame_buf: core::Mat,
    frame_seq: u64,
}

impl OpenCvFrameSource {
    pub fn open(config: &CaptureConfig) -> Result<Self, CaptureError> {
        let mut capture =
            videoio::VideoCapture::new(config.device_index, config.backend.to_opencv_api())
                .map_err(|e| CaptureError::ReadFailed(e.to_string()))?;

        if !capture
            .is_opened()
            .map_err(|e| CaptureError::ReadFailed(e.to_string()))?
        {
            return Err(CaptureError::DeviceNotFound);
        }

        let _ = capture.set(videoio::CAP_PROP_FRAME_WIDTH, config.width as f64);
        let _ = capture.set(videoio::CAP_PROP_FRAME_HEIGHT, config.height as f64);

        Ok(Self {
            capture,
            frame_buf: core::Mat::default(),
            frame_seq: 0,
        })
    }
}

impl FrameSource for OpenCvFrameSource {
    fn read_frame(&mut self) -> Result<Option<CapturedFrame>, CaptureError> {
        let ok = self
            .capture
            .read(&mut self.frame_buf)
            .map_err(|e| CaptureError::ReadFailed(e.to_string()))?;

        if !ok || self.frame_buf.empty() {
            return Ok(None);
        }

        let rows = self.frame_buf.rows() as u32;
        let cols = self.frame_buf.cols() as u32;
        let channels = self.frame_buf.channels() as usize;

        let expected_len = (rows as usize) * (cols as usize) * channels;
        let data = self
            .frame_buf
            .data_bytes()
            .map_err(|e| CaptureError::ReadFailed(format!("failed to access Mat data: {e}")))?;

        if data.len() < expected_len {
            return Err(CaptureError::ReadFailed(
                "Mat data size mismatch".to_string(),
            ));
        }

        let pixel_format = match channels {
            3 => PixelFormat::Bgr8,
            4 => PixelFormat::Rgba8,
            1 => PixelFormat::Gray8,
            _ => {
                return Err(CaptureError::ReadFailed(format!(
                    "unsupported channel count: {channels}"
                )));
            }
        };

        let owned_bytes: Arc<[u8]> = data[..expected_len].into();

        self.frame_seq += 1;
        let now_millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Ok(Some(CapturedFrame {
            frame_sequence: FrameSequence(self.frame_seq),
            captured_at_millis: now_millis,
            image: ImageBuffer {
                width: cols,
                height: rows,
                pixel_format,
                bytes: owned_bytes,
            },
        }))
    }
}
