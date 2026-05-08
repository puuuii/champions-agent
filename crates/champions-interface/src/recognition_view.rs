#[derive(Debug, Clone)]
pub struct OpponentPartyView {
    pub pokemons: Vec<RecognizedPokemonView>,
    pub conflicts: Vec<ConflictView>,
}

#[derive(Debug, Clone)]
pub struct RecognizedPokemonView {
    pub slot_index: u8,
    pub display_name: Option<String>,
    pub confidence: ConfidenceView,
    pub candidates: Vec<CandidateView>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConfidenceView {
    High(f32),
    Medium(f32),
    Low(f32),
    Unknown,
}

#[derive(Debug, Clone)]
pub struct CandidateView {
    pub display_name: String,
    pub score: f32,
}

#[derive(Debug, Clone)]
pub struct ConflictView {
    pub species_name: String,
    pub slot_indices: Vec<u8>,
}
