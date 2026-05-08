use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct MoveData {
    pub id: u32,
    pub type_id: u32,
    pub power: Option<u32>,
    pub damage_class_id: u32,
}

#[derive(Debug, Clone)]
pub struct NatureData {
    pub id: u32,
    pub increased_stat_id: u32,
    pub decreased_stat_id: u32,
}

#[derive(Debug, Clone)]
pub struct BattleMasterData {
    pub pokemon_stats: HashMap<u32, [u32; 6]>,
    pub moves: HashMap<u32, MoveData>,
    pub natures: HashMap<u32, NatureData>,
    pub type_efficacy: HashMap<(u32, u32), u32>,
    pub pokemon_types: HashMap<u32, Vec<u32>>,
}
