use crate::ports::ocr_engine::{OcrEngine, OcrError, OcrImage, SelectionDetectionResult};
use champions_domain::recognition::ScreenState;

const SELECTION_KEYWORDS: &[&str] = &["せんぱつ", "選出", "バトルチームを"];

pub struct DetectSelectionScreenCommand {
    pub target_text_image: OcrImage,
}

pub struct DetectSelectionScreenUseCase<'a> {
    ocr_engine: &'a dyn OcrEngine,
}

impl<'a> DetectSelectionScreenUseCase<'a> {
    pub fn new(ocr_engine: &'a dyn OcrEngine) -> Self {
        Self { ocr_engine }
    }

    pub fn execute(
        &self,
        command: DetectSelectionScreenCommand,
    ) -> Result<SelectionDetectionResult, OcrError> {
        let raw_text = self
            .ocr_engine
            .recognize_selection_text(&command.target_text_image)?;

        let screen_state = if SELECTION_KEYWORDS.iter().any(|kw| raw_text.contains(kw)) {
            ScreenState::SelectionScreen
        } else {
            ScreenState::Other
        };

        Ok(SelectionDetectionResult {
            raw_text,
            screen_state,
        })
    }
}
