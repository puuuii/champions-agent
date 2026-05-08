use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PokemonUsageSummary {
    pub name: String,
    pub types: Vec<String>,
    pub moves: Vec<MoveUsage>,
    pub items: Vec<ItemUsage>,
    pub effort_values: Vec<EffortValueUsage>,
    pub natures: Vec<NatureUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveUsage {
    pub name: String,
    pub rate: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemUsage {
    pub name: String,
    pub rate: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffortValueUsage {
    pub h: u32,
    pub a: u32,
    pub b: u32,
    pub c: u32,
    pub d: u32,
    pub s: u32,
    pub rate: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NatureUsage {
    pub name: String,
    pub rate: String,
}
