use super::atomic_write::atomic_write;
use champions_application::errors::UsageError;
use champions_application::ports::UsageRepository;
use champions_domain::usage::PokemonUsageSummary;
use std::path::PathBuf;
use std::sync::RwLock;

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
}
