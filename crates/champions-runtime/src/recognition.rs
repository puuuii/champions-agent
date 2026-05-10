use champions_application::ports::ocr_engine::OcrImage;
use champions_application::use_cases::OpponentPartyIdentificationResult;
use champions_application::{PartyImageSet, SelectionDetectionResult};

pub trait RecognitionPort: Send + Sync {
    fn detect_selection_screen(&self, image: OcrImage) -> Result<SelectionDetectionResult, String>;
    fn detect_battle_result_phase(&self, image: OcrImage) -> Result<bool, String>;

    fn identify_opponent_party(
        &self,
        images: PartyImageSet,
    ) -> Result<OpponentPartyIdentificationResult, String>;

    fn extract_target_text_image(
        &self,
        frame_width: u32,
        frame_height: u32,
        frame_bytes: &[u8],
    ) -> OcrImage;
    fn extract_battle_result_text_image(
        &self,
        frame_width: u32,
        frame_height: u32,
        frame_bytes: &[u8],
    ) -> OcrImage;

    fn extract_party_slots(
        &self,
        frame_width: u32,
        frame_height: u32,
        frame_bytes: &[u8],
    ) -> PartyImageSet;
}
