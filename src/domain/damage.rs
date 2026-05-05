use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct PokemonStatRecord {
    pub pokemon_id: u32,
    pub stat_id: u32,
    pub base_stat: u32,
}

#[derive(Debug, Clone)]
pub struct MoveRecord {
    pub id: u32,
    pub identifier: String,
    pub type_id: u32,
    pub power: Option<u32>,
    pub damage_class_id: u32,
}

#[derive(Debug, Clone)]
pub struct MoveMetaRecord {
    pub move_id: u32,
    pub meta_category_id: u32,
    pub meta_ailment_id: i32,
    pub min_hits: Option<u32>,
    pub max_hits: Option<u32>,
    pub crit_rate: u32,
}

#[derive(Debug, Clone)]
pub struct MoveStatChangeRecord {
    pub move_id: u32,
    pub stat_id: u32,
    pub change: i8,
}

#[derive(Debug, Clone)]
pub struct NatureRecord {
    pub id: u32,
    pub identifier: String,
    pub decreased_stat_id: u32,
    pub increased_stat_id: u32,
}

#[derive(Debug, Clone)]
pub struct PokemonTypeRecord {
    pub pokemon_id: u32,
    pub type_id: u32,
}

#[derive(Debug, Clone)]
pub struct TypeEfficacyRecord {
    pub damage_type_id: u32,
    pub target_type_id: u32,
    pub damage_factor: u32,
}

pub struct MasterData {
    pub pokemon_stats: HashMap<u32, [u32; 6]>,
    pub moves: HashMap<u32, MoveRecord>,
    pub move_metas: HashMap<u32, MoveMetaRecord>,
    pub move_stat_changes: HashMap<u32, Vec<MoveStatChangeRecord>>,
    pub natures: HashMap<u32, NatureRecord>,
    pub type_efficacy: HashMap<(u32, u32), u32>,
    pub pokemon_types: HashMap<u32, Vec<u32>>,
    pub abilities: HashMap<u32, String>,
    pub items: HashMap<u32, String>,
}
// --- 省略 ---
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_damage_args_instantiation() {
        let args = DamageArgs {
            attacker_id: 1,
            defender_id: 2,
            move_id: 1,
            attacker_ap: [0; 6],
            defender_ap: [0; 6],
            attacker_nature_id: 1,
            defender_nature_id: 1,
            attacker_stages: [0; 8],
            defender_stages: [0; 8],
            attacker_status_id: None,
            is_critical: false,
            rng_roll: 1.0,
        };
        assert_eq!(args.attacker_id, 1);
    }
}
