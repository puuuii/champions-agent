use champions_infrastructure::capture::{
    CaptureReadError, OpenCvCaptureDevice, RawFrame, RawPixelFormat,
};
use champions_interface::FrameSequence;
use champions_runtime::{CaptureError, CapturedFrame, FrameSource, ImageBuffer, PixelFormat};

pub use champions_infrastructure::capture::CaptureConfig;

pub struct OpenCvFrameSource {
    device: OpenCvCaptureDevice,
}

impl OpenCvFrameSource {
    pub fn open(config: &CaptureConfig) -> Result<Self, CaptureError> {
        let device = OpenCvCaptureDevice::open(config).map_err(map_capture_error)?;
        Ok(Self { device })
    }
}

impl FrameSource for OpenCvFrameSource {
    fn read_frame(&mut self) -> Result<Option<CapturedFrame>, CaptureError> {
        self.device
            .read_frame()
            .map(|frame| frame.map(map_frame))
            .map_err(map_capture_error)
    }
}

fn map_frame(frame: RawFrame) -> CapturedFrame {
    CapturedFrame {
        frame_sequence: FrameSequence(0),
        captured_at_millis: frame.captured_at_millis,
        image: ImageBuffer {
            width: frame.image.width,
            height: frame.image.height,
            pixel_format: map_pixel_format(frame.image.pixel_format),
            bytes: frame.image.bytes,
        },
    }
}

fn map_pixel_format(format: RawPixelFormat) -> PixelFormat {
    match format {
        RawPixelFormat::Bgr8 => PixelFormat::Bgr8,
        RawPixelFormat::Rgb8 => PixelFormat::Rgb8,
        RawPixelFormat::Rgba8 => PixelFormat::Rgba8,
        RawPixelFormat::Gray8 => PixelFormat::Gray8,
    }
}

fn map_capture_error(error: CaptureReadError) -> CaptureError {
    match error {
        CaptureReadError::DeviceNotFound => CaptureError::DeviceNotFound,
        CaptureReadError::ReadFailed(message) => CaptureError::ReadFailed(message),
    }
}
