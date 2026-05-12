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
            tracing::info!(
                path = %self.path.display(),
                "party file is missing; returning default party",
            );
            return Ok(SavedParty::default());
        }
        let data = std::fs::read_to_string(&self.path)
            .map_err(|e| PartyRepositoryError::LoadFailed(e.to_string()))?;
        let party: SavedParty = serde_json::from_str(&data)
            .map_err(|e| PartyRepositoryError::LoadFailed(format!("JSON parse: {e}")))?;
        tracing::info!(
            path = %self.path.display(),
            pokemons = party.pokemons.len(),
            saved_pokemons = party.saved_pokemons.len(),
            "party loaded from disk",
        );
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
        tracing::info!(
            path = %self.path.display(),
            pokemons = party.pokemons.len(),
            saved_pokemons = party.saved_pokemons.len(),
            bytes = json.len(),
            "party saved to disk",
        );
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
            saved_pokemons: vec![PokemonBuild {
                species_name: "ミミッキュ".to_string(),
                item_name: Some("いのちのたま".to_string()),
                ability_name: Some("ばけのかわ".to_string()),
                nature_name: Some("ようき".to_string()),
                effort_values: EffortValueSpread {
                    h: 4,
                    a: 252,
                    b: 0,
                    c: 0,
                    d: 0,
                    s: 252,
                },
                moves: MoveSet {
                    moves: [
                        "じゃれつく".to_string(),
                        "かげうち".to_string(),
                        "つるぎのまい".to_string(),
                        "ドレインパンチ".to_string(),
                    ],
                },
            }],
        };

        repo.save_my_party(&party).unwrap();
        let loaded = repo.load_my_party().unwrap();
        assert_eq!(loaded.pokemons.len(), 1);
        assert_eq!(loaded.pokemons[0].species_name, "ピカチュウ");
        assert_eq!(loaded.saved_pokemons.len(), 1);
        assert_eq!(loaded.saved_pokemons[0].species_name, "ミミッキュ");

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_load_old_format_without_saved_pokemons() {
        let dir = test_dir();
        let path = dir.join("party_old_format.json");
        fs::write(
            &path,
            r#"{
  "pokemons": [
    {
      "species_name": "ピカチュウ",
      "item_name": null,
      "ability_name": null,
      "nature_name": null,
      "effort_values": { "h": 0, "a": 0, "b": 0, "c": 0, "d": 0, "s": 0 },
      "moves": { "moves": ["", "", "", ""] }
    }
  ]
}"#,
        )
        .unwrap();

        let repo = JsonPartyRepository::new(path.clone());
        let loaded = repo.load_my_party().unwrap();
        assert_eq!(loaded.pokemons.len(), 1);
        assert!(loaded.saved_pokemons.is_empty());

        let _ = fs::remove_file(&path);
    }
}
