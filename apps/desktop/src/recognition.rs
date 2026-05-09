use std::sync::Arc;

use champions_application::use_cases::{
    DetectSelectionScreenCommand, DetectSelectionScreenUseCase, IdentifyOpponentPartyCommand,
    IdentifyOpponentPartyUseCase, OpponentPartyIdentificationResult,
};
use champions_application::{
    OcrImage, PartyImageSet, RecognitionConfig, RecognitionImageExtractor,
    SelectionDetectionResult, UsageRepository,
};
use champions_infrastructure::{MangaOcrEngine, OnnxPartyIdentifier, OpenCvCropper};
use champions_runtime::RecognitionPort;

pub struct RecognitionRuntimePort {
    ocr_engine: MangaOcrEngine,
    party_identifier: OnnxPartyIdentifier,
    image_extractor: OpenCvCropper,
    usage_repo: Arc<dyn UsageRepository>,
    recognition_config: RecognitionConfig,
}

impl RecognitionRuntimePort {
    pub fn new(
        ocr_engine: MangaOcrEngine,
        party_identifier: OnnxPartyIdentifier,
        image_extractor: OpenCvCropper,
        usage_repo: Arc<dyn UsageRepository>,
    ) -> Self {
        Self {
            ocr_engine,
            party_identifier,
            image_extractor,
            usage_repo,
            recognition_config: RecognitionConfig::default(),
        }
    }
}

impl RecognitionPort for RecognitionRuntimePort {
    fn detect_selection_screen(&self, image: OcrImage) -> Result<SelectionDetectionResult, String> {
        let use_case = DetectSelectionScreenUseCase::new(&self.ocr_engine);
        use_case
            .execute(DetectSelectionScreenCommand {
                target_text_image: image,
            })
            .map_err(|e| e.to_string())
    }

    fn identify_opponent_party(
        &self,
        images: PartyImageSet,
    ) -> Result<OpponentPartyIdentificationResult, String> {
        let use_case =
            IdentifyOpponentPartyUseCase::new(&self.party_identifier, self.usage_repo.as_ref());
        use_case
            .execute(IdentifyOpponentPartyCommand {
                party_images: images,
                config: self.recognition_config.clone(),
            })
            .map_err(|e| e.to_string())
    }

    fn extract_target_text_image(
        &self,
        frame_width: u32,
        frame_height: u32,
        frame_bytes: &[u8],
    ) -> OcrImage {
        self.image_extractor
            .extract_target_text_image(frame_width, frame_height, frame_bytes)
    }

    fn extract_party_slots(
        &self,
        frame_width: u32,
        frame_height: u32,
        frame_bytes: &[u8],
    ) -> PartyImageSet {
        self.image_extractor
            .extract_party_slots(frame_width, frame_height, frame_bytes)
    }
}
