use super::atomic_write::atomic_write;
use crate::usage_id_mapping::resolve_master_pokemon_id;
use champions_application::errors::UsageError;
use champions_application::ports::UsageRepository;
use champions_domain::usage::{
    AbilityUsage, EffortValueUsage, ItemUsage, MoveUsage, NatureUsage, PokemonUsageSummary,
};
use std::path::PathBuf;
use std::sync::RwLock;

pub struct JsonUsageRepository {
    path: PathBuf,
    cache: RwLock<Option<Vec<PokemonUsageSummary>>>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct LegacyPokemonUsageSummary {
    id: String,
    name: String,
    types: Vec<String>,
    moves: Vec<MoveUsage>,
    items: Vec<ItemUsage>,
    #[serde(default)]
    abilities: Vec<AbilityUsage>,
    effort_values: Vec<EffortValueUsage>,
    natures: Vec<NatureUsage>,
}

impl JsonUsageRepository {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            cache: RwLock::new(None),
        }
    }

    fn ensure_loaded(&self) -> Result<(), UsageError> {
        {
            let read = self.cache.read().unwrap();
            if read.is_some() {
                return Ok(());
            }
        }
        let data = self.load_from_disk()?;
        let mut write = self.cache.write().unwrap();
        *write = Some(data);
        Ok(())
    }

    fn load_from_disk(&self) -> Result<Vec<PokemonUsageSummary>, UsageError> {
        if !self.path.exists() {
            tracing::info!(
                path = %self.path.display(),
                "usage cache file is missing; starting with empty data",
            );
            return Ok(Vec::new());
        }
        let data = std::fs::read_to_string(&self.path)
            .map_err(|e| UsageError::LoadFailed(e.to_string()))?;

        match serde_json::from_str::<Vec<PokemonUsageSummary>>(&data) {
            Ok(usages) => {
                tracing::info!(
                    path = %self.path.display(),
                    count = usages.len(),
                    "usage data loaded from disk",
                );
                Ok(usages)
            }
            Err(error) => {
                tracing::warn!(
                    path = %self.path.display(),
                    %error,
                    "failed to parse usage cache as current schema; attempting legacy GameWith ID migration",
                );
                let usages = self.load_legacy_from_disk(&data)?;
                tracing::info!(
                    path = %self.path.display(),
                    count = usages.len(),
                    "legacy usage data migrated from disk",
                );
                Ok(usages)
            }
        }
    }

    fn load_legacy_from_disk(&self, data: &str) -> Result<Vec<PokemonUsageSummary>, UsageError> {
        let legacy: Vec<LegacyPokemonUsageSummary> = serde_json::from_str(data)
            .map_err(|e| UsageError::LoadFailed(format!("usage.json legacy parse: {e}")))?;

        let mut migrated = Vec::new();
        for entry in legacy {
            let Some(pokemon_id) = resolve_master_pokemon_id(&entry.id, &entry.name) else {
                tracing::warn!(
                    path = %self.path.display(),
                    gamewith_id = %entry.id,
                    name = %entry.name,
                    "skipping legacy usage entry because it could not be mapped to a master pokemon_id",
                );
                continue;
            };

            migrated.push(PokemonUsageSummary {
                pokemon_id,
                name: entry.name,
                types: entry.types,
                moves: entry.moves,
                items: entry.items,
                abilities: entry.abilities,
                effort_values: entry.effort_values,
                natures: entry.natures,
            });
        }

        Ok(migrated)
    }
}

impl UsageRepository for JsonUsageRepository {
    fn find_by_pokemon_id(
        &self,
        pokemon_id: u32,
    ) -> Result<Option<PokemonUsageSummary>, UsageError> {
        self.ensure_loaded()?;
        let read = self.cache.read().unwrap();
        let data = read.as_ref().unwrap();
        Ok(data.iter().find(|u| u.pokemon_id == pokemon_id).cloned())
    }

    fn find_many_by_pokemon_ids(
        &self,
        pokemon_ids: &[u32],
    ) -> Result<Vec<PokemonUsageSummary>, UsageError> {
        self.ensure_loaded()?;
        let read = self.cache.read().unwrap();
        let data = read.as_ref().unwrap();
        let results: Vec<PokemonUsageSummary> = data
            .iter()
            .filter(|u| pokemon_ids.contains(&u.pokemon_id))
            .cloned()
            .collect();
        Ok(results)
    }

    fn find_by_pokemon_name(&self, name: &str) -> Result<Option<PokemonUsageSummary>, UsageError> {
        self.ensure_loaded()?;
        let read = self.cache.read().unwrap();
        let data = read.as_ref().unwrap();
        Ok(data.iter().find(|u| u.name == name).cloned())
    }

    fn find_many_by_names(&self, names: &[String]) -> Result<Vec<PokemonUsageSummary>, UsageError> {
        self.ensure_loaded()?;
        let read = self.cache.read().unwrap();
        let data = read.as_ref().unwrap();
        let results: Vec<PokemonUsageSummary> = data
            .iter()
            .filter(|u| names.contains(&u.name))
            .cloned()
            .collect();
        Ok(results)
    }

    fn replace_all(&self, data: Vec<PokemonUsageSummary>) -> Result<(), UsageError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| UsageError::SaveFailed(e.to_string()))?;
        }
        let json = serde_json::to_string_pretty(&data)
            .map_err(|e| UsageError::SaveFailed(e.to_string()))?;
        atomic_write(&self.path, json.as_bytes())
            .map_err(|e| UsageError::SaveFailed(e.to_string()))?;
        let mut write = self.cache.write().unwrap();
        let count = data.len();
        *write = Some(data);
        tracing::info!(
            path = %self.path.display(),
            count,
            bytes = json.len(),
            "usage data replaced and cache refreshed",
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use champions_domain::usage::{MoveUsage, PokemonUsageSummary};
    use std::fs;

    fn test_dir() -> PathBuf {
        let dir = std::env::temp_dir().join("champions_test_usage_repo");
        let _ = fs::create_dir_all(&dir);
        dir
    }

    fn sample_usage(pokemon_id: u32, name: &str) -> PokemonUsageSummary {
        PokemonUsageSummary {
            pokemon_id,
            name: name.to_string(),
            types: vec!["でんき".to_string()],
            moves: vec![MoveUsage {
                name: "10まんボルト".to_string(),
                rate: "80%".to_string(),
            }],
            items: vec![],
            abilities: vec![],
            effort_values: vec![],
            natures: vec![],
        }
    }

    #[test]
    fn test_load_missing_returns_empty() {
        let path = test_dir().join("nonexistent_usage.json");
        let repo = JsonUsageRepository::new(path);
        let result = repo.find_by_pokemon_name("ピカチュウ").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_replace_all_and_find() {
        let dir = test_dir();
        let path = dir.join("usage_test.json");
        let repo = JsonUsageRepository::new(path.clone());

        let data = vec![sample_usage(25, "ピカチュウ"), sample_usage(6, "リザードン")];
        repo.replace_all(data).unwrap();

        let found = repo.find_by_pokemon_name("ピカチュウ").unwrap();
        assert!(found.is_some());
        assert_eq!(found.as_ref().unwrap().name, "ピカチュウ");
        assert_eq!(found.unwrap().pokemon_id, 25);

        let found_by_id = repo.find_by_pokemon_id(6).unwrap();
        assert_eq!(found_by_id.as_ref().map(|usage| usage.name.as_str()), Some("リザードン"));

        let not_found = repo.find_by_pokemon_name("フシギダネ").unwrap();
        assert!(not_found.is_none());

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_find_many_by_names() {
        let dir = test_dir();
        let path = dir.join("usage_many.json");
        let repo = JsonUsageRepository::new(path.clone());

        let data = vec![
            sample_usage(25, "ピカチュウ"),
            sample_usage(6, "リザードン"),
            sample_usage(1, "フシギダネ"),
        ];
        repo.replace_all(data).unwrap();

        let names = vec!["ピカチュウ".to_string(), "フシギダネ".to_string()];
        let results = repo.find_many_by_names(&names).unwrap();
        assert_eq!(results.len(), 2);

        let ids = vec![6, 1];
        let results = repo.find_many_by_pokemon_ids(&ids).unwrap();
        assert_eq!(results.len(), 2);

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_loads_legacy_gamewith_ids_and_migrates_to_master_ids() {
        let dir = test_dir();
        let path = dir.join("legacy_usage.json");
        std::fs::write(
            &path,
            r#"[
  {
    "id": "479_2",
    "name": "ウォッシュロトム",
    "types": ["electric", "water"],
    "moves": [],
    "items": [],
    "abilities": [],
    "effort_values": [],
    "natures": []
  },
  {
    "id": "670_2",
    "name": "フラエッテ(永遠)",
    "types": ["fairy"],
    "moves": [],
    "items": [],
    "abilities": [],
    "effort_values": [],
    "natures": []
  }
]"#,
        )
        .unwrap();

        let repo = JsonUsageRepository::new(path.clone());
        let rotom = repo.find_by_pokemon_name("ウォッシュロトム").unwrap().unwrap();
        let floette = repo.find_by_pokemon_name("フラエッテ(永遠)").unwrap().unwrap();

        assert_eq!(rotom.pokemon_id, 10009);
        assert_eq!(floette.pokemon_id, 10061);

        let _ = fs::remove_file(&path);
    }
}
