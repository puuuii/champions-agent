use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use image::RgbaImage;
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
    pub debug_mode: bool,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            device_index: 0,
            backend: CaptureBackend::Auto,
            width: 1920,
            height: 1080,
            fps: 60,
            debug_mode: false,
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

const DEBUG_DUMP_INTERVAL: Duration = Duration::from_secs(1);

pub struct OpenCvCaptureDevice {
    capture: videoio::VideoCapture,
    frame_buf: core::Mat,
    bgra_buf: core::Mat,
    debug_mode: bool,
    last_debug_dump_at: Option<Instant>,
}

impl OpenCvCaptureDevice {
    pub fn open(config: &CaptureConfig) -> Result<Self, CaptureReadError> {
        tracing::info!(
            device_index = config.device_index,
            backend = ?config.backend,
            width = config.width,
            height = config.height,
            fps = config.fps,
            debug_mode = config.debug_mode,
            "opening capture device",
        );
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
        tracing::info!("capture device opened");

        Ok(Self {
            capture,
            frame_buf: core::Mat::default(),
            bgra_buf: core::Mat::default(),
            debug_mode: config.debug_mode,
            last_debug_dump_at: None,
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
                    core::AlgorithmHint::ALGO_HINT_DEFAULT,
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
                    core::AlgorithmHint::ALGO_HINT_DEFAULT,
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
        let is_continuous = frame_buf.is_continuous();
        let row_stride_elements = frame_buf
            .step1(0)
            .map_err(|e| CaptureReadError::ReadFailed(format!("failed to read Mat step1: {e}")))?;
        let row_stride_bytes = row_stride_elements * frame_buf.elem_size1();

        let expected_len = (rows as usize) * (cols as usize) * channels;
        let bytes: Arc<[u8]> = {
            let data = frame_buf
                .data_bytes()
                .map_err(|e| CaptureReadError::ReadFailed(format!("failed to access Mat data: {e}")))?;

            if data.len() < expected_len {
                return Err(CaptureReadError::ReadFailed(
                    "Mat data size mismatch".to_string(),
                ));
            }
            data[..expected_len].into()
        };

        self.debug_dump_capture_frame(
            rows,
            cols,
            channels,
            is_continuous,
            row_stride_bytes,
            row_stride_elements,
            expected_len,
            &bytes,
        );

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
                bytes,
            },
        }))
    }

    fn debug_dump_capture_frame(
        &mut self,
        rows: u32,
        cols: u32,
        channels: usize,
        is_continuous: bool,
        row_stride_bytes: usize,
        row_stride_elements: usize,
        expected_len: usize,
        data: &[u8],
    ) {
        if !self.debug_mode {
            return;
        }

        let should_dump = match self.last_debug_dump_at {
            Some(last) => last.elapsed() >= DEBUG_DUMP_INTERVAL,
            None => true,
        };
        if !should_dump {
            return;
        }
        self.last_debug_dump_at = Some(Instant::now());

        tracing::info!(
            frame_width = cols,
            frame_height = rows,
            channels,
            is_continuous,
            row_stride_bytes,
            row_stride_elements,
            data_len = data.len(),
            expected_len,
            "capture frame debug",
        );

        let Some(image) = convert_bgra_to_rgba_image(cols, rows, data) else {
            tracing::warn!(
                frame_width = cols,
                frame_height = rows,
                data_len = data.len(),
                "failed to build capture debug image",
            );
            return;
        };

        let output_dir = Path::new("tmp");
        if let Err(error) = std::fs::create_dir_all(output_dir) {
            tracing::warn!(
                path = %output_dir.display(),
                %error,
                "failed to create capture debug directory",
            );
            return;
        }

        let path = output_dir.join("full_frame.png");
        if let Err(error) = image.save(&path) {
            tracing::warn!(
                path = %path.display(),
                %error,
                "failed to save capture debug image",
            );
        }
    }
}

fn convert_bgra_to_rgba_image(width: u32, height: u32, bgra_bytes: &[u8]) -> Option<RgbaImage> {
    let expected_len = width as usize * height as usize * 4;
    if bgra_bytes.len() < expected_len {
        return None;
    }

    let mut rgba_bytes = Vec::with_capacity(expected_len);
    for px in bgra_bytes[..expected_len].chunks_exact(4) {
        rgba_bytes.push(px[2]);
        rgba_bytes.push(px[1]);
        rgba_bytes.push(px[0]);
        rgba_bytes.push(px[3]);
    }

    RgbaImage::from_raw(width, height, rgba_bytes)
}
