use std::path::{Path, PathBuf};

use champions_application::{OcrEngine, OcrError, OcrImage};
use manga_ocr_rs::MangaOcr;

pub struct MangaOcrEngine {
    inner: MangaOcr,
}

impl MangaOcrEngine {
    pub fn new(model_dir: impl AsRef<Path>) -> Result<Self, OcrError> {
        let model_path: PathBuf = model_dir.as_ref().to_path_buf();
        let inner = MangaOcr::new(&model_path)
            .map_err(|e| OcrError::ModelNotFound(format!("{}: {e}", model_path.display())))?;
        Ok(Self { inner })
    }
}

impl OcrEngine for MangaOcrEngine {
    fn recognize_selection_text(&self, image: &OcrImage) -> Result<String, OcrError> {
        if image.width == 0 || image.height == 0 || image.rgb_bytes.is_empty() {
            return Ok(String::new());
        }

        let rgb_image =
            image::RgbImage::from_raw(image.width, image.height, image.rgb_bytes.clone())
                .ok_or_else(|| OcrError::InferenceFailed("failed to create RGB image".into()))?;

        let dynamic_img = image::DynamicImage::ImageRgb8(rgb_image);

        self.inner
            .recognize(&dynamic_img)
            .map_err(|e| OcrError::InferenceFailed(format!("OCR inference error: {e}")))
    }
}
