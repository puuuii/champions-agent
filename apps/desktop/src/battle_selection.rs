use std::sync::Arc;

use champions_application::{RecognitionConfig, SlotImage};
use champions_domain::recognition::{ConfidenceScore, RecognizedPokemon};
use champions_infrastructure::{OnnxPartyIdentifier, OpenCvCropper};
use champions_runtime::PixelFormat;

#[derive(Debug, Clone)]
pub struct BattleSelectionCandidate {
    pub name: String,
    pub score: f32,
    pub is_high_confidence: bool,
}

#[derive(Debug, Clone, Default)]
pub struct BattleSelectionObservation {
    pub my_pokemon: Option<BattleSelectionCandidate>,
    pub opponent_pokemon: Option<BattleSelectionCandidate>,
}

pub struct BattleSelectionInferer {
    party_identifier: Arc<OnnxPartyIdentifier>,
    cropper: Arc<OpenCvCropper>,
    recognition_config: RecognitionConfig,
}

impl BattleSelectionInferer {
    pub fn new(party_identifier: Arc<OnnxPartyIdentifier>, cropper: Arc<OpenCvCropper>) -> Self {
        Self {
            party_identifier,
            cropper,
            recognition_config: RecognitionConfig::default(),
        }
    }

    pub fn infer_from_frame(
        &self,
        frame_width: u32,
        frame_height: u32,
        pixel_format: PixelFormat,
        frame_bytes: &[u8],
        my_candidates: &[String],
        opponent_candidates: &[String],
    ) -> Result<BattleSelectionObservation, String> {
        let normalized_bytes = normalize_frame_bytes_for_cropper(pixel_format, frame_bytes);

        let my_pokemon = self
            .cropper
            .extract_battle_my_pokemon(frame_width, frame_height, &normalized_bytes)
            .map(|slot| self.identify_from_candidates(slot, my_candidates))
            .transpose()?;

        let opponent_pokemon = self
            .cropper
            .extract_battle_opponent_pokemon(frame_width, frame_height, &normalized_bytes)
            .map(|slot| self.identify_from_candidates(slot, opponent_candidates))
            .transpose()?;

        Ok(BattleSelectionObservation {
            my_pokemon: my_pokemon.flatten(),
            opponent_pokemon: opponent_pokemon.flatten(),
        })
    }

    fn identify_from_candidates(
        &self,
        slot_image: SlotImage,
        candidate_names: &[String],
    ) -> Result<Option<BattleSelectionCandidate>, String> {
        if candidate_names.is_empty() {
            return Ok(None);
        }

        let recognized = self
            .party_identifier
            .identify_from_candidate_names(&slot_image, candidate_names, &self.recognition_config)
            .map_err(|error| error.to_string())?;

        Ok(recognized.and_then(map_recognized_candidate))
    }
}

fn map_recognized_candidate(recognized: RecognizedPokemon) -> Option<BattleSelectionCandidate> {
    let name = recognized.display_name?;
    let (score, is_high_confidence) = match recognized.confidence {
        ConfidenceScore::High(score) => (score, true),
        ConfidenceScore::Medium(score) => (score, false),
        ConfidenceScore::Low(score) => (score, false),
        ConfidenceScore::Unknown => (0.0, false),
    };

    Some(BattleSelectionCandidate {
        name,
        score,
        is_high_confidence,
    })
}

fn normalize_frame_bytes_for_cropper(pixel_format: PixelFormat, frame_bytes: &[u8]) -> Vec<u8> {
    match pixel_format {
        PixelFormat::Bgr8 | PixelFormat::Bgra8 | PixelFormat::Gray8 => frame_bytes.to_vec(),
        PixelFormat::Rgb8 => frame_bytes
            .chunks_exact(3)
            .flat_map(|chunk| [chunk[2], chunk[1], chunk[0]])
            .collect(),
        PixelFormat::Rgba8 => frame_bytes
            .chunks_exact(4)
            .flat_map(|chunk| [chunk[2], chunk[1], chunk[0], chunk[3]])
            .collect(),
    }
}
