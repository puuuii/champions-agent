use crate::ports::ocr_engine::{OcrEngine, OcrError, OcrImage, SelectionDetectionResult};
use champions_domain::recognition::ScreenState;

const MODE_KEYWORDS: &[&str] = &["シングル"];
const BATTLE_KEYWORDS: &[&str] = &["バトル"];

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

        let screen_state = if is_selection_screen_text(&raw_text) {
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

fn is_selection_screen_text(raw_text: &str) -> bool {
    let normalized = normalize_ocr_text(raw_text);
    MODE_KEYWORDS.iter().any(|kw| normalized.contains(kw))
        && BATTLE_KEYWORDS.iter().any(|kw| normalized.contains(kw))
}

fn normalize_ocr_text(raw_text: &str) -> String {
    raw_text.chars().filter(|ch| !ch.is_whitespace()).collect()
}
