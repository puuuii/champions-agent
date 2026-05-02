use serde::Deserialize;
use std::collections::HashMap;

// --- CSVレコードの構造定義 ---

#[derive(Debug, Deserialize, Clone)]
pub struct PokemonStatRecord {
    pub pokemon_id: u32,
    pub stat_id: u32,
    pub base_stat: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MoveRecord {
    pub id: u32,
    pub identifier: String,
    pub type_id: u32,
    pub power: Option<u32>,
    pub damage_class_id: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MoveMetaRecord {
    pub move_id: u32,
    pub meta_category_id: u32,
    pub meta_ailment_id: i32,
    pub min_hits: Option<u32>,
    pub max_hits: Option<u32>,
    pub crit_rate: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MoveStatChangeRecord {
    pub move_id: u32,
    pub stat_id: u32,
    pub change: i8,
}

#[derive(Debug, Deserialize, Clone)]
pub struct NatureRecord {
    pub id: u32,
    pub identifier: String,
    pub decreased_stat_id: u32,
    pub increased_stat_id: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PokemonTypeRecord {
    pub pokemon_id: u32,
    pub type_id: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TypeEfficacyRecord {
    pub damage_type_id: u32,
    pub target_type_id: u32,
    pub damage_factor: u32,
}

// --- オンメモリ展開用リポジトリ ---

pub struct MasterData {
    pub pokemon_stats: HashMap<u32, [u32; 6]>, // pokemon_id -> [H,A,B,C,D,S]
    pub moves: HashMap<u32, MoveRecord>,
    pub move_metas: HashMap<u32, MoveMetaRecord>,
    pub move_stat_changes: HashMap<u32, Vec<MoveStatChangeRecord>>,
    pub natures: HashMap<u32, NatureRecord>,
    pub type_efficacy: HashMap<(u32, u32), u32>,
    pub pokemon_types: HashMap<u32, Vec<u32>>,
    pub abilities: HashMap<u32, String>, // id -> identifier
    pub items: HashMap<u32, String>,     // id -> identifier
}

// --- 計算用引数 ---

pub struct DamageArgs {
    pub attacker_id: u32,
    pub defender_id: u32,
    pub move_id: u32,
    pub attacker_ap: [u32; 6],
    pub defender_ap: [u32; 6],
    pub attacker_nature_id: u32,
    pub defender_nature_id: u32,
    pub attacker_stages: [i8; 8],
    pub defender_stages: [i8; 8],
    pub attacker_status_id: Option<u32>,
    pub is_critical: bool,
    pub rng_roll: f64,
}
