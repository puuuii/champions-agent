use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SavedParty {
    pub pokemons: Vec<PokemonBuild>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PokemonBuild {
    pub species_name: String,
    pub item_name: Option<String>,
    pub ability_name: Option<String>,
    pub nature_name: Option<String>,
    pub effort_values: EffortValueSpread,
    pub moves: MoveSet,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EffortValueSpread {
    pub h: u32,
    pub a: u32,
    pub b: u32,
    pub c: u32,
    pub d: u32,
    pub s: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MoveSet {
    pub moves: [String; 4],
}
