use crate::errors::UsageError;
use champions_domain::usage::PokemonUsageSummary;

pub trait UsageRepository: Send + Sync {
    fn find_by_pokemon_name(&self, name: &str) -> Result<Option<PokemonUsageSummary>, UsageError>;
    fn find_many_by_names(&self, names: &[String]) -> Result<Vec<PokemonUsageSummary>, UsageError>;
    fn replace_all(&self, data: Vec<PokemonUsageSummary>) -> Result<(), UsageError>;
}
