use crate::ports::ocr_engine::{OcrEngine, OcrError, OcrImage};

const WIN_KEYWORDS: &[&str] = &["WIN"];
const LOSE_KEYWORDS: &[&str] = &["LOSE"];

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
    WIN_KEYWORDS.iter().any(|kw| normalized.contains(kw))
        || LOSE_KEYWORDS.iter().any(|kw| normalized.contains(kw))
}

fn normalize_ocr_text(raw_text: &str) -> String {
    raw_text.chars().filter(|ch| !ch.is_whitespace()).collect()
}
