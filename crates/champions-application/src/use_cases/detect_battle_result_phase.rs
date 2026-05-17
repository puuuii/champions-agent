use crate::ports::ocr_engine::{OcrEngine, OcrError, OcrImage};

const RESULT_PHASE_HINT_CHARS: &[char] = &['W', 'I', 'N', 'L', 'O', 'S', 'E'];
const MIN_RESULT_PHASE_HINT_MATCHES: usize = 3;

pub struct DetectBattleResultPhaseCommand {
    pub target_text_image: OcrImage,
}

pub struct DetectBattleResultPhaseUseCase<'a> {
    ocr_engine: &'a dyn OcrEngine,
}

impl<'a> DetectBattleResultPhaseUseCase<'a> {
    pub fn new(ocr_engine: &'a dyn OcrEngine) -> Self {
        Self { ocr_engine }
    }

    pub fn execute(&self, command: DetectBattleResultPhaseCommand) -> Result<bool, OcrError> {
        let raw_text = self
            .ocr_engine
            .recognize_selection_text(&command.target_text_image)?;

        Ok(is_battle_result_phase_text(&raw_text))
    }
}

fn is_battle_result_phase_text(raw_text: &str) -> bool {
    let normalized = normalize_ocr_text(raw_text).to_uppercase();

    RESULT_PHASE_HINT_CHARS
        .iter()
        .filter(|&&ch| normalized.contains(ch))
        .count()
        >= MIN_RESULT_PHASE_HINT_MATCHES
}

fn normalize_ocr_text(raw_text: &str) -> String {
    raw_text.chars().filter(|ch| !ch.is_whitespace()).collect()
}
