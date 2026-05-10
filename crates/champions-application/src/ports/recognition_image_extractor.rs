use crate::ports::ocr_engine::OcrImage;
use crate::ports::party_identifier::PartyImageSet;

pub trait RecognitionImageExtractor: Send + Sync {
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
