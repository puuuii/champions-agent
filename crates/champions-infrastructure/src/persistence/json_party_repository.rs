use super::atomic_write::atomic_write;
use champions_application::errors::PartyRepositoryError;
use champions_application::ports::PartyRepository;
use champions_domain::party::SavedParty;
use std::path::PathBuf;

pub struct JsonPartyRepository {
    path: PathBuf,
}

impl JsonPartyRepository {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl PartyRepository for JsonPartyRepository {
    fn load_my_party(&self) -> Result<SavedParty, PartyRepositoryError> {
        if !self.path.exists() {
            return Ok(SavedParty::default());
        }
        let data = std::fs::read_to_string(&self.path)
            .map_err(|e| PartyRepositoryError::LoadFailed(e.to_string()))?;
        let party: SavedParty = serde_json::from_str(&data)
            .map_err(|e| PartyRepositoryError::LoadFailed(format!("JSON parse: {e}")))?;
        Ok(party)
    }

    fn save_my_party(&self, party: &SavedParty) -> Result<(), PartyRepositoryError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| PartyRepositoryError::SaveFailed(e.to_string()))?;
        }
        let json = serde_json::to_string_pretty(party)
            .map_err(|e| PartyRepositoryError::SaveFailed(e.to_string()))?;
        atomic_write(&self.path, json.as_bytes())
            .map_err(|e| PartyRepositoryError::SaveFailed(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use champions_domain::party::{EffortValueSpread, MoveSet, PokemonBuild};
    use std::fs;

    fn test_dir() -> PathBuf {
        let dir = std::env::temp_dir().join("champions_test_party_repo");
        let _ = fs::create_dir_all(&dir);
        dir
    }

    #[test]
    fn test_load_missing_returns_default() {
        let path = test_dir().join("nonexistent.json");
        let repo = JsonPartyRepository::new(path);
        let party = repo.load_my_party().unwrap();
        assert!(party.pokemons.is_empty());
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = test_dir();
        let path = dir.join("party_roundtrip.json");
        let repo = JsonPartyRepository::new(path.clone());

        let party = SavedParty {
            pokemons: vec![PokemonBuild {
                species_name: "ピカチュウ".to_string(),
                item_name: Some("きあいのタスキ".to_string()),
                ability_name: Some("せいでんき".to_string()),
                nature_name: Some("ようき".to_string()),
                effort_values: EffortValueSpread {
                    h: 0,
                    a: 252,
                    b: 0,
                    c: 0,
                    d: 4,
                    s: 252,
                },
                moves: MoveSet {
                    moves: [
                        "ボルテッカー".to_string(),
                        "アイアンテール".to_string(),
                        "でんこうせっか".to_string(),
                        "かわらわり".to_string(),
                    ],
                },
            }],
        };

        repo.save_my_party(&party).unwrap();
        let loaded = repo.load_my_party().unwrap();
        assert_eq!(loaded.pokemons.len(), 1);
        assert_eq!(loaded.pokemons[0].species_name, "ピカチュウ");

        let _ = fs::remove_file(&path);
    }
}
