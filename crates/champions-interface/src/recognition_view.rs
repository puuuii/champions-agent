#[derive(Debug, Clone, PartialEq)]
pub struct OpponentPartyView {
    pub pokemons: Vec<RecognizedPokemonView>,
    pub conflicts: Vec<ConflictView>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecognizedPokemonView {
    pub slot_index: u8,
    pub display_name: Option<String>,
    pub confidence: ConfidenceView,
    pub candidates: Vec<CandidateView>,
    pub usage: Option<PokemonUsageSummaryView>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConfidenceView {
    High(f32),
    Medium(f32),
    Low(f32),
    Unknown,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CandidateView {
    pub display_name: String,
    pub score: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConflictView {
    pub species_name: String,
    pub slot_indices: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PokemonUsageSummaryView {
    pub name: String,
    pub types: Vec<String>,
    pub moves: Vec<MoveUsageView>,
    pub items: Vec<ItemUsageView>,
    pub effort_values: Vec<EffortValueUsageView>,
    pub natures: Vec<NatureUsageView>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MoveUsageView {
    pub name: String,
    pub rate: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ItemUsageView {
    pub name: String,
    pub rate: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EffortValueUsageView {
    pub h: u32,
    pub a: u32,
    pub b: u32,
    pub c: u32,
    pub d: u32,
    pub s: u32,
    pub rate: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NatureUsageView {
    pub name: String,
    pub rate: String,
}
