use thiserror::Error;

use champions_domain::recognition::ScreenState;

#[derive(Debug, Error)]
pub enum OcrError {
    #[error("OCR model not found: {0}")]
    ModelNotFound(String),
    #[error("OCR inference failed: {0}")]
    InferenceFailed(String),
}

#[derive(Debug, Clone)]
pub struct OcrImage {
    pub width: u32,
    pub height: u32,
    pub rgb_bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct SelectionDetectionResult {
    pub raw_text: String,
    pub screen_state: ScreenState,
}

pub trait OcrEngine: Send + Sync {
    fn recognize_selection_text(&self, image: &OcrImage) -> Result<String, OcrError>;
}
