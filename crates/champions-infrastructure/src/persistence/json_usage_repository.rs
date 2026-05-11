use super::atomic_write::atomic_write;
use champions_application::errors::UsageError;
use champions_application::ports::UsageRepository;
use champions_domain::usage::PokemonUsageSummary;
use std::path::PathBuf;
use std::sync::{PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};

pub struct JsonUsageRepository {
    path: PathBuf,
    cache: RwLock<Option<Vec<PokemonUsageSummary>>>,
}

impl JsonUsageRepository {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            cache: RwLock::new(None),
        }
    }

    fn map_load_poison<T>(error: PoisonError<T>) -> UsageError {
        UsageError::LoadFailed(format!("usage cache poisoned: {error}"))
    }

    fn map_save_poison<T>(error: PoisonError<T>) -> UsageError {
        UsageError::SaveFailed(format!("usage cache poisoned: {error}"))
    }

    fn read_cache(
        &self,
    ) -> Result<RwLockReadGuard<'_, Option<Vec<PokemonUsageSummary>>>, UsageError> {
        self.cache.read().map_err(Self::map_load_poison)
    }

    fn write_cache_for_load(
        &self,
    ) -> Result<RwLockWriteGuard<'_, Option<Vec<PokemonUsageSummary>>>, UsageError> {
        self.cache.write().map_err(Self::map_load_poison)
    }

    fn write_cache_for_save(
        &self,
    ) -> Result<RwLockWriteGuard<'_, Option<Vec<PokemonUsageSummary>>>, UsageError> {
        self.cache.write().map_err(Self::map_save_poison)
    }

    fn ensure_loaded(&self) -> Result<(), UsageError> {
        {
            let read = self.read_cache()?;
            if read.is_some() {
                return Ok(());
            }
        }
        let data = self.load_from_disk()?;
        let mut write = self.write_cache_for_load()?;
        if write.is_none() {
            *write = Some(data);
        }
        Ok(())
    }

    fn load_from_disk(&self) -> Result<Vec<PokemonUsageSummary>, UsageError> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let data = std::fs::read_to_string(&self.path)
            .map_err(|e| UsageError::LoadFailed(e.to_string()))?;
        let usages: Vec<PokemonUsageSummary> = serde_json::from_str(&data)
            .map_err(|e| UsageError::LoadFailed(format!("usage.json parse: {e}")))?;
        Ok(usages)
    }
}

impl UsageRepository for JsonUsageRepository {
    fn find_by_pokemon_name(&self, name: &str) -> Result<Option<PokemonUsageSummary>, UsageError> {
        self.ensure_loaded()?;
        let read = self.read_cache()?;
        Ok(read
            .as_ref()
            .and_then(|data| data.iter().find(|u| u.name == name).cloned()))
    }

    fn find_many_by_names(&self, names: &[String]) -> Result<Vec<PokemonUsageSummary>, UsageError> {
        self.ensure_loaded()?;
        let read = self.read_cache()?;
        let results = read
            .as_ref()
            .map(|data| {
                data.iter()
                    .filter(|u| names.contains(&u.name))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        Ok(results)
    }

    fn replace_all(&self, data: Vec<PokemonUsageSummary>) -> Result<(), UsageError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| UsageError::SaveFailed(e.to_string()))?;
        }
        let json = serde_json::to_string_pretty(&data)
            .map_err(|e| UsageError::SaveFailed(e.to_string()))?;
        let mut write = self.write_cache_for_save()?;
        atomic_write(&self.path, json.as_bytes())
            .map_err(|e| UsageError::SaveFailed(e.to_string()))?;
        *write = Some(data);
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

    fn sample_usage(name: &str) -> PokemonUsageSummary {
        PokemonUsageSummary {
            id: name.to_string(),
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

        let data = vec![sample_usage("ピカチュウ"), sample_usage("リザードン")];
        repo.replace_all(data).unwrap();

        let found = repo.find_by_pokemon_name("ピカチュウ").unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "ピカチュウ");

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
            sample_usage("ピカチュウ"),
            sample_usage("リザードン"),
            sample_usage("フシギダネ"),
        ];
        repo.replace_all(data).unwrap();

        let names = vec!["ピカチュウ".to_string(), "フシギダネ".to_string()];
        let results = repo.find_many_by_names(&names).unwrap();
        assert_eq!(results.len(), 2);

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_replace_all_replaces_preloaded_cache() {
        let dir = test_dir();
        let path = dir.join("usage_replace_loaded.json");
        let repo = JsonUsageRepository::new(path.clone());

        repo.replace_all(vec![sample_usage("ピカチュウ")]).unwrap();
        let initial = repo.find_by_pokemon_name("ピカチュウ").unwrap();
        assert!(initial.is_some());

        repo.replace_all(vec![sample_usage("ミュウ")]).unwrap();

        assert!(repo.find_by_pokemon_name("ピカチュウ").unwrap().is_none());
        assert_eq!(
            repo.find_by_pokemon_name("ミュウ")
                .unwrap()
                .as_ref()
                .map(|usage| usage.name.as_str()),
            Some("ミュウ")
        );

        let _ = fs::remove_file(&path);
    }
}
