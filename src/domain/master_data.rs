use std::error::Error;
use std::fs::File;
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct MasterData {
    pub pokemon: Vec<String>,
    pub moves: Vec<String>,
    pub natures: Vec<String>,
    pub items: Vec<String>,
    pub abilities: Vec<String>,
}

impl MasterData {
    pub fn load<P: AsRef<Path>>(dir: P) -> Result<Self, Box<dyn Error>> {
        let dir = dir.as_ref();
        
        let usage_path = dir.join("usage.json");
        let mut pokemon = if usage_path.exists() {
            let data = std::fs::read_to_string(usage_path)?;
            let json: serde_json::Value = serde_json::from_str(&data)?;
            json.as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|p| p["name"].as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        pokemon.sort();
        pokemon.dedup();

        let moves = Self::load_names_csv(dir.join("move_names.csv"), "move_id")?;
        let natures = Self::load_names_csv(dir.join("nature_names.csv"), "nature_id")?;
        let items = Self::load_names_csv(dir.join("item_names.csv"), "item_id")?;
        
        let ability_path = dir.join("ability_names.csv");
        let abilities = if ability_path.exists() {
            Self::load_names_csv(ability_path, "ability_id")?
        } else {
            Vec::new()
        };

        println!("[MasterData] Loaded: {} pokemon, {} moves, {} natures, {} items, {} abilities",
            pokemon.len(), moves.len(), natures.len(), items.len(), abilities.len());

        Ok(Self {
            pokemon,
            moves,
            natures,
            items,
            abilities,
        })
    }

    fn load_names_csv<P: AsRef<Path>>(path: P, id_col: &str) -> Result<Vec<String>, Box<dyn Error>> {
        let file = File::open(path)?;
        let mut rdr = csv::Reader::from_reader(file);
        let headers = rdr.headers()?.clone();
        
        let name_idx = headers.iter().position(|h| h == "name").ok_or("name column not found")?;
        let lang_idx = headers.iter().position(|h| h == "local_language_id").ok_or("local_language_id column not found")?;
        let _id_idx = headers.iter().position(|h| h == id_col).ok_or(format!("{} column not found", id_col))?;

        let mut names = Vec::new();
        for result in rdr.records() {
            let record = result?;
            if record.get(lang_idx) == Some("1") {
                if let Some(name) = record.get(name_idx) {
                    names.push(name.to_string());
                }
            }
        }
        
        names.sort();
        names.dedup();
        
        Ok(names)
    }
}
