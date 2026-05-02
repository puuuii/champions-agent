use super::models::*;
use std::collections::HashMap;
use std::error::Error;
use std::path::Path;

impl MasterData {
    pub fn load_from_dir<P: AsRef<Path>>(dir: P) -> Result<Self, Box<dyn Error>> {
        let p = dir.as_ref();

        // 1. 種族値 (pokemon_stats.csv)
        let mut pokemon_stats = HashMap::new();
        let mut rdr = csv::Reader::from_path(p.join("pokemon_stats.csv"))?;
        for result in rdr.deserialize() {
            let rec: PokemonStatRecord = result?;
            let entry = pokemon_stats.entry(rec.pokemon_id).or_insert([0u32; 6]);
            if rec.stat_id >= 1 && rec.stat_id <= 6 {
                entry[(rec.stat_id - 1) as usize] = rec.base_stat;
            }
        }

        // 2. 技 (moves.csv)
        let mut moves = HashMap::new();
        let mut rdr = csv::Reader::from_path(p.join("moves.csv"))?;
        for result in rdr.deserialize() {
            let rec: MoveRecord = result?;
            moves.insert(rec.id, rec);
        }

        // 3. 技ランク変化 (move_meta_stat_changes.csv)
        let mut move_stat_changes: HashMap<u32, Vec<MoveStatChangeRecord>> = HashMap::new();
        let mut rdr = csv::Reader::from_path(p.join("move_meta_stat_changes.csv"))?;
        for result in rdr.deserialize() {
            let rec: MoveStatChangeRecord = result?;
            move_stat_changes.entry(rec.move_id).or_default().push(rec);
        }

        // 4. ポケモンのタイプ (pokemon_types.csv)
        let mut pokemon_types: HashMap<u32, Vec<u32>> = HashMap::new();
        let mut rdr = csv::Reader::from_path(p.join("pokemon_types.csv"))?;
        for result in rdr.deserialize() {
            let rec: PokemonTypeRecord = result?;
            pokemon_types
                .entry(rec.pokemon_id)
                .or_default()
                .push(rec.type_id);
        }

        // 5. タイプ相性 (type_efficacy.csv)
        let mut type_efficacy: HashMap<(u32, u32), u32> = HashMap::new();
        let mut rdr = csv::Reader::from_path(p.join("type_efficacy.csv"))?;
        for result in rdr.deserialize() {
            let rec: TypeEfficacyRecord = result?;
            type_efficacy.insert((rec.damage_type_id, rec.target_type_id), rec.damage_factor);
        }

        // 6. 性格 (追加)[cite: 19, 21]
        let mut natures = HashMap::new();
        let mut rdr = csv::Reader::from_path(p.join("natures.csv"))?;
        for result in rdr.deserialize() {
            let rec: NatureRecord = result?;
            natures.insert(rec.id, rec);
        }

        Ok(MasterData {
            pokemon_stats,
            moves,
            move_metas: HashMap::new(),
            move_stat_changes,
            natures, // HashMap::new() から変更
            type_efficacy,
            pokemon_types,
            abilities: HashMap::new(),
            items: HashMap::new(),
        })
    }
}
