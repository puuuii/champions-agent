use std::path::{Path, PathBuf};

use champions_application::{OcrEngine, OcrError, OcrImage};
use manga_ocr_rs::{MangaOcr, default_model_dir};

const REQUIRED_MODEL_FILES: [&str; 3] = ["encoder_model.onnx", "decoder_model.onnx", "vocab.txt"];

pub struct MangaOcrEngine {
    inner: MangaOcr,
}

impl MangaOcrEngine {
    pub fn new(model_dir: impl AsRef<Path>) -> Result<Self, OcrError> {
        let requested_path: PathBuf = model_dir.as_ref().to_path_buf();
        let fallback_path = default_model_dir().to_path_buf();
        let mut errors = Vec::new();
        tracing::info!(
            requested_path = %requested_path.display(),
            fallback_path = %fallback_path.display(),
            "initializing Manga OCR engine",
        );

        for candidate in candidate_model_dirs(&requested_path, &fallback_path) {
            tracing::debug!(model_dir = %candidate.display(), "attempting Manga OCR model directory");
            match MangaOcr::new(&candidate) {
                Ok(inner) => {
                    tracing::info!(model_dir = %candidate.display(), "Manga OCR engine initialized");
                    return Ok(Self { inner });
                }
                Err(e) => {
                    tracing::warn!(
                        model_dir = %candidate.display(),
                        error = %e,
                        "failed to initialize Manga OCR model directory",
                    );
                    errors.push(format!("{}: {e}", candidate.display()));
                }
            }
        }

        Err(OcrError::ModelNotFound(build_model_not_found_message(
            &requested_path,
            &fallback_path,
            &errors,
        )))
    }
}

fn candidate_model_dirs(requested_path: &Path, fallback_path: &Path) -> Vec<PathBuf> {
    let mut candidates = vec![requested_path.to_path_buf()];
    if requested_path != fallback_path && !has_required_model_files(requested_path) {
        candidates.push(fallback_path.to_path_buf());
    }
    candidates
}

fn has_required_model_files(model_dir: &Path) -> bool {
    REQUIRED_MODEL_FILES
        .iter()
        .all(|file_name| model_dir.join(file_name).is_file())
}

fn build_model_not_found_message(
    requested_path: &Path,
    fallback_path: &Path,
    errors: &[String],
) -> String {
    let required_files = REQUIRED_MODEL_FILES.join(", ");
    let attempted_paths = if requested_path == fallback_path {
        format!("{}", requested_path.display())
    } else {
        format!("{}, {}", requested_path.display(), fallback_path.display())
    };

    if errors.is_empty() {
        format!(
            "expected [{}] under one of: {}",
            required_files, attempted_paths
        )
    } else {
        format!(
            "expected [{}] under one of: {}. errors: {}",
            required_files,
            attempted_paths,
            errors.join(" | ")
        )
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
