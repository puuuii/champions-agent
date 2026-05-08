#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpeciesId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SelectionSlot(pub u8);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConfidenceScore {
    High(f32),
    Medium(f32),
    Low(f32),
    Unknown,
}

impl ConfidenceScore {
    pub fn from_score(score: f32, high_threshold: f32, low_threshold: f32) -> Self {
        if score >= high_threshold {
            Self::High(score)
        } else if score >= low_threshold {
            Self::Medium(score)
        } else if score > 0.0 {
            Self::Low(score)
        } else {
            Self::Unknown
        }
    }

    pub fn raw_score(&self) -> Option<f32> {
        match self {
            Self::High(s) | Self::Medium(s) | Self::Low(s) => Some(*s),
            Self::Unknown => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RecognitionCandidate {
    pub species_id: Option<SpeciesId>,
    pub display_name: String,
    pub score: f32,
}

#[derive(Debug, Clone)]
pub struct RecognizedPokemon {
    pub slot: SelectionSlot,
    pub species_id: Option<SpeciesId>,
    pub display_name: Option<String>,
    pub confidence: ConfidenceScore,
    pub candidates: Vec<RecognitionCandidate>,
}

#[derive(Debug, Clone)]
pub struct RecognizedParty {
    pub pokemons: Vec<RecognizedPokemon>,
}

#[derive(Debug, Clone)]
pub struct RecognitionConflict {
    pub species_name: String,
    pub slots: Vec<SelectionSlot>,
}
