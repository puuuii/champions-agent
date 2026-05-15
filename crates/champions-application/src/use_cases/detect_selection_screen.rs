use crate::ports::ocr_engine::{OcrEngine, OcrError, OcrImage, SelectionDetectionResult};
use champions_domain::recognition::ScreenState;

const SELECTION_PHASE_HINT_CHARS: &[char] = &['シ', 'ン', 'グ', 'ル', 'ラ', 'ク', 'バ', 'ト'];
const MIN_SELECTION_PHASE_HINT_MATCHES: usize = 3;

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
    SELECTION_PHASE_HINT_CHARS
        .iter()
        .filter(|&&ch| normalized.contains(ch))
        .count()
        >= MIN_SELECTION_PHASE_HINT_MATCHES
}

fn normalize_ocr_text(raw_text: &str) -> String {
    raw_text.chars().filter(|ch| !ch.is_whitespace()).collect()
}
