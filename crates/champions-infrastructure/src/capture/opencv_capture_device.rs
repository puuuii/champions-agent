use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use opencv::{core, imgproc, prelude::*, videoio};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureBackend {
    Auto,
}

impl CaptureBackend {
    fn to_opencv_api(self) -> i32 {
        match self {
            Self::Auto => videoio::CAP_ANY,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CaptureConfig {
    pub device_index: i32,
    pub backend: CaptureBackend,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            device_index: 0,
            backend: CaptureBackend::Auto,
            width: 1920,
            height: 1080,
            fps: 60,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawPixelFormat {
    Bgr8,
    Bgra8,
    Rgb8,
    Rgba8,
    Gray8,
}

#[derive(Debug, Clone)]
pub struct RawImageBuffer {
    pub width: u32,
    pub height: u32,
    pub pixel_format: RawPixelFormat,
    pub bytes: Arc<[u8]>,
}

#[derive(Debug, Clone)]
pub struct RawFrame {
    pub captured_at_millis: u64,
    pub image: RawImageBuffer,
}

#[derive(Debug, thiserror::Error)]
pub enum CaptureReadError {
    #[error("capture device not found")]
    DeviceNotFound,
    #[error("read failed: {0}")]
    ReadFailed(String),
}

pub struct OpenCvCaptureDevice {
    capture: videoio::VideoCapture,
    frame_buf: core::Mat,
    bgra_buf: core::Mat,
}

impl OpenCvCaptureDevice {
    pub fn open(config: &CaptureConfig) -> Result<Self, CaptureReadError> {
        let mut capture =
            videoio::VideoCapture::new(config.device_index, config.backend.to_opencv_api())
                .map_err(|e| CaptureReadError::ReadFailed(e.to_string()))?;

        if !capture
            .is_opened()
            .map_err(|e| CaptureReadError::ReadFailed(e.to_string()))?
        {
            return Err(CaptureReadError::DeviceNotFound);
        }

        let _ = capture.set(videoio::CAP_PROP_FRAME_WIDTH, config.width as f64);
        let _ = capture.set(videoio::CAP_PROP_FRAME_HEIGHT, config.height as f64);
        let _ = capture.set(videoio::CAP_PROP_FPS, config.fps as f64);
        let _ = capture.set(videoio::CAP_PROP_BUFFERSIZE, 1.0);

        Ok(Self {
            capture,
            frame_buf: core::Mat::default(),
            bgra_buf: core::Mat::default(),
        })
    }

    pub fn read_frame(&mut self) -> Result<Option<RawFrame>, CaptureReadError> {
        let ok = self
            .capture
            .read(&mut self.frame_buf)
            .map_err(|e| CaptureReadError::ReadFailed(e.to_string()))?;

        if !ok || self.frame_buf.empty() {
            return Ok(None);
        }

        let source_channels = self.frame_buf.channels() as usize;

        let (frame_buf, pixel_format) = match source_channels {
            3 => {
                imgproc::cvt_color(
                    &self.frame_buf,
                    &mut self.bgra_buf,
                    imgproc::COLOR_BGR2BGRA,
                    0,
                )
                .map_err(|e| CaptureReadError::ReadFailed(e.to_string()))?;
                (&self.bgra_buf, RawPixelFormat::Bgra8)
            }
            4 => (&self.frame_buf, RawPixelFormat::Bgra8),
            1 => {
                imgproc::cvt_color(
                    &self.frame_buf,
                    &mut self.bgra_buf,
                    imgproc::COLOR_GRAY2BGRA,
                    0,
                )
                .map_err(|e| CaptureReadError::ReadFailed(e.to_string()))?;
                (&self.bgra_buf, RawPixelFormat::Bgra8)
            }
            _ => {
                return Err(CaptureReadError::ReadFailed(format!(
                    "unsupported channel count: {source_channels}"
                )));
            }
        };

        let rows = frame_buf.rows() as u32;
        let cols = frame_buf.cols() as u32;
        let channels = frame_buf.channels() as usize;

        let expected_len = (rows as usize) * (cols as usize) * channels;
        let data = frame_buf
            .data_bytes()
            .map_err(|e| CaptureReadError::ReadFailed(format!("failed to access Mat data: {e}")))?;

        if data.len() < expected_len {
            return Err(CaptureReadError::ReadFailed(
                "Mat data size mismatch".to_string(),
            ));
        }

        let now_millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Ok(Some(RawFrame {
            captured_at_millis: now_millis,
            image: RawImageBuffer {
                width: cols,
                height: rows,
                pixel_format,
                bytes: data[..expected_len].into(),
            },
        }))
    }
}
