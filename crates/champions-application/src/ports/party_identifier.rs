use thiserror::Error;

use champions_domain::recognition::{RecognizedParty, SelectionSlot};

#[derive(Debug, Error)]
pub enum PartyIdentifierError {
    #[error("ONNX model not found: {0}")]
    ModelNotFound(String),
    #[error("inference failed: {0}")]
    InferenceFailed(String),
    #[error("master data not loaded: {0}")]
    MasterDataNotLoaded(String),
}

#[derive(Debug, Clone)]
pub struct SlotImage {
    pub slot: SelectionSlot,
    pub width: u32,
    pub height: u32,
    pub rgb_bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct PartyImageSet {
    pub slots: Vec<SlotImage>,
}

#[derive(Debug, Clone)]
pub struct RecognitionConfig {
    pub top_candidates: usize,
}

impl Default for RecognitionConfig {
    fn default() -> Self {
<<<<<<< HEAD
        Self {
            min_confidence: 0.5,
            top_candidates: 3,
        }
=======
        Self { top_candidates: 3 }
>>>>>>> rearchitect
    }
}

pub trait PartyIdentifier: Send + Sync {
    fn identify_opponent_party(
        &self,
        input: &PartyImageSet,
        config: &RecognitionConfig,
    ) -> Result<RecognizedParty, PartyIdentifierError>;
}
