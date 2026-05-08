use champions_application::errors::CatalogError;
use champions_application::ports::CatalogRepository;
use champions_domain::catalog::{BattleMasterData, MoveData, NatureData};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct CsvCatalogRepository {
    master_data_dir: PathBuf,
    species_names: Vec<String>,
    move_names: Vec<String>,
    item_names: Vec<String>,
    nature_names: Vec<String>,
    ability_names: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct PokemonStatRecord {
    pokemon_id: u32,
    stat_id: u32,
    base_stat: u32,
}

#[derive(Debug, Deserialize)]
struct MoveRecord {
    id: u32,
    #[allow(dead_code)]
    identifier: String,
    type_id: u32,
    power: Option<u32>,
    damage_class_id: u32,
}

#[derive(Debug, Deserialize)]
struct NatureRecord {
    id: u32,
    #[allow(dead_code)]
    identifier: String,
    decreased_stat_id: u32,
    increased_stat_id: u32,
}

#[derive(Debug, Deserialize)]
struct PokemonTypeRecord {
    pokemon_id: u32,
    type_id: u32,
}

#[derive(Debug, Deserialize)]
struct TypeEfficacyRecord {
    damage_type_id: u32,
    target_type_id: u32,
    damage_factor: u32,
}

impl CsvCatalogRepository {
    pub fn new(
        master_data_dir: &Path,
        usage_json_path: Option<&Path>,
    ) -> Result<Self, CatalogError> {
        let species_names = Self::load_species_names(master_data_dir, usage_json_path)?;
        let move_names = Self::load_names_csv(&master_data_dir.join("move_names.csv"))?;
        let nature_names = Self::load_names_csv(&master_data_dir.join("nature_names.csv"))?;
        let item_names = Self::load_names_csv(&master_data_dir.join("item_names.csv"))?;
        let ability_names =
            Self::load_names_csv_optional(&master_data_dir.join("ability_names.csv"));

        Ok(Self {
            master_data_dir: master_data_dir.to_path_buf(),
            species_names,
            move_names,
            item_names,
            nature_names,
            ability_names,
        })
    }

    fn load_species_names(
        master_data_dir: &Path,
        usage_json_path: Option<&Path>,
    ) -> Result<Vec<String>, CatalogError> {
        let default_path = master_data_dir.join("usage.json");
        let path = usage_json_path.unwrap_or(&default_path);
        if !path.exists() {
            return Ok(Vec::new());
        }
        let data = std::fs::read_to_string(path)
            .map_err(|e| CatalogError::LoadFailed(format!("usage.json: {e}")))?;
        let json: serde_json::Value = serde_json::from_str(&data)
            .map_err(|e| CatalogError::LoadFailed(format!("usage.json parse: {e}")))?;
        let mut names: Vec<String> = json
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|p| p["name"].as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names.dedup();
        Ok(names)
    }

    fn load_names_csv(path: &Path) -> Result<Vec<String>, CatalogError> {
        if !path.exists() {
            return Err(CatalogError::NotFound(path.display().to_string()));
        }
        let file = std::fs::File::open(path)
            .map_err(|e| CatalogError::LoadFailed(format!("{}: {e}", path.display())))?;
        let mut rdr = csv::Reader::from_reader(file);
        let headers = rdr
            .headers()
            .map_err(|e| CatalogError::LoadFailed(format!("headers: {e}")))?
            .clone();

        let name_idx = headers
            .iter()
            .position(|h| h == "name")
            .ok_or_else(|| CatalogError::LoadFailed("name column not found".into()))?;
        let lang_idx = headers
            .iter()
            .position(|h| h == "local_language_id")
            .ok_or_else(|| CatalogError::LoadFailed("local_language_id column not found".into()))?;

        let mut names = Vec::new();
        for result in rdr.records() {
            let record = result.map_err(|e| CatalogError::LoadFailed(e.to_string()))?;
            if record.get(lang_idx) == Some("1")
                && let Some(name) = record.get(name_idx)
            {
                names.push(name.to_string());
            }
        }
        names.sort();
        names.dedup();
        Ok(names)
    }

    fn load_names_csv_optional(path: &Path) -> Vec<String> {
        Self::load_names_csv(path).unwrap_or_default()
    }

    fn prefix_match(names: &[String], query: &str, limit: usize) -> Vec<String> {
        if query.is_empty() {
            return Vec::new();
        }
        names
            .iter()
            .filter(|n| n.starts_with(query))
            .take(limit)
            .cloned()
            .collect()
    }
}

impl CatalogRepository for CsvCatalogRepository {
    fn suggest_species(&self, query: &str, limit: usize) -> Result<Vec<String>, CatalogError> {
        Ok(Self::prefix_match(&self.species_names, query, limit))
    }

    fn suggest_moves(&self, query: &str, limit: usize) -> Result<Vec<String>, CatalogError> {
        Ok(Self::prefix_match(&self.move_names, query, limit))
    }

    fn suggest_items(&self, query: &str, limit: usize) -> Result<Vec<String>, CatalogError> {
        Ok(Self::prefix_match(&self.item_names, query, limit))
    }

    fn suggest_natures(&self, query: &str, limit: usize) -> Result<Vec<String>, CatalogError> {
        Ok(Self::prefix_match(&self.nature_names, query, limit))
    }

    fn suggest_abilities(&self, query: &str, limit: usize) -> Result<Vec<String>, CatalogError> {
        Ok(Self::prefix_match(&self.ability_names, query, limit))
    }

    fn load_battle_master_data(&self) -> Result<BattleMasterData, CatalogError> {
        let p = &self.master_data_dir;

        let pokemon_stats = Self::load_pokemon_stats(p)?;
        let moves = Self::load_moves(p)?;
        let natures = Self::load_natures(p)?;
        let type_efficacy = Self::load_type_efficacy(p)?;
        let pokemon_types = Self::load_pokemon_types(p)?;

        Ok(BattleMasterData {
            pokemon_stats,
            moves,
            natures,
            type_efficacy,
            pokemon_types,
        })
    }
}

impl CsvCatalogRepository {
    fn load_pokemon_stats(dir: &Path) -> Result<HashMap<u32, [u32; 6]>, CatalogError> {
        let path = dir.join("pokemon_stats.csv");
        let mut rdr = csv::Reader::from_path(&path)
            .map_err(|e| CatalogError::LoadFailed(format!("pokemon_stats.csv: {e}")))?;
        let mut stats = HashMap::new();
        for result in rdr.deserialize() {
            let rec: PokemonStatRecord =
                result.map_err(|e| CatalogError::LoadFailed(e.to_string()))?;
            let entry = stats.entry(rec.pokemon_id).or_insert([0u32; 6]);
            if rec.stat_id >= 1 && rec.stat_id <= 6 {
                entry[(rec.stat_id - 1) as usize] = rec.base_stat;
            }
        }
        Ok(stats)
    }

    fn load_moves(dir: &Path) -> Result<HashMap<u32, MoveData>, CatalogError> {
        let path = dir.join("moves.csv");
        let mut rdr = csv::Reader::from_path(&path)
            .map_err(|e| CatalogError::LoadFailed(format!("moves.csv: {e}")))?;
        let mut moves = HashMap::new();
        for result in rdr.deserialize() {
            let rec: MoveRecord = result.map_err(|e| CatalogError::LoadFailed(e.to_string()))?;
            moves.insert(
                rec.id,
                MoveData {
                    id: rec.id,
                    type_id: rec.type_id,
                    power: rec.power,
                    damage_class_id: rec.damage_class_id,
                },
            );
        }
        Ok(moves)
    }

    fn load_natures(dir: &Path) -> Result<HashMap<u32, NatureData>, CatalogError> {
        let path = dir.join("natures.csv");
        let mut rdr = csv::Reader::from_path(&path)
            .map_err(|e| CatalogError::LoadFailed(format!("natures.csv: {e}")))?;
        let mut natures = HashMap::new();
        for result in rdr.deserialize() {
            let rec: NatureRecord = result.map_err(|e| CatalogError::LoadFailed(e.to_string()))?;
            natures.insert(
                rec.id,
                NatureData {
                    id: rec.id,
                    increased_stat_id: rec.increased_stat_id,
                    decreased_stat_id: rec.decreased_stat_id,
                },
            );
        }
        Ok(natures)
    }

    fn load_type_efficacy(dir: &Path) -> Result<HashMap<(u32, u32), u32>, CatalogError> {
        let path = dir.join("type_efficacy.csv");
        let mut rdr = csv::Reader::from_path(&path)
            .map_err(|e| CatalogError::LoadFailed(format!("type_efficacy.csv: {e}")))?;
        let mut efficacy = HashMap::new();
        for result in rdr.deserialize() {
            let rec: TypeEfficacyRecord =
                result.map_err(|e| CatalogError::LoadFailed(e.to_string()))?;
            efficacy.insert((rec.damage_type_id, rec.target_type_id), rec.damage_factor);
        }
        Ok(efficacy)
    }

    fn load_pokemon_types(dir: &Path) -> Result<HashMap<u32, Vec<u32>>, CatalogError> {
        let path = dir.join("pokemon_types.csv");
        let mut rdr = csv::Reader::from_path(&path)
            .map_err(|e| CatalogError::LoadFailed(format!("pokemon_types.csv: {e}")))?;
        let mut types: HashMap<u32, Vec<u32>> = HashMap::new();
        for result in rdr.deserialize() {
            let rec: PokemonTypeRecord =
                result.map_err(|e| CatalogError::LoadFailed(e.to_string()))?;
            types.entry(rec.pokemon_id).or_default().push(rec.type_id);
        }
        Ok(types)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prefix_match_empty_query() {
        let names = vec!["Pikachu".to_string(), "Pidgey".to_string()];
        let result = CsvCatalogRepository::prefix_match(&names, "", 10);
        assert!(result.is_empty());
    }

    #[test]
    fn test_prefix_match_finds_matches() {
        let names = vec![
            "ピカチュウ".to_string(),
            "ピジョット".to_string(),
            "フシギダネ".to_string(),
        ];
        let result = CsvCatalogRepository::prefix_match(&names, "ピ", 10);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_prefix_match_respects_limit() {
        let names = vec!["ア".to_string(), "アイ".to_string(), "アウ".to_string()];
        let result = CsvCatalogRepository::prefix_match(&names, "ア", 2);
        assert_eq!(result.len(), 2);
    }
}
