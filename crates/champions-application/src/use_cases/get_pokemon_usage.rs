use crate::errors::{CatalogError, UsageError};
use crate::ports::{CatalogRepository, UsageRepository};
use champions_domain::usage::PokemonUsageSummary;

pub struct GetPokemonUsageQuery {
    pub name: String,
}

pub struct GetPokemonUsageResult {
    pub usage: Option<PokemonUsageSummary>,
}

#[derive(Debug, thiserror::Error)]
pub enum GetPokemonUsageError {
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error(transparent)]
    Usage(#[from] UsageError),
}

pub struct GetPokemonUsageUseCase<'a> {
    catalog_repo: &'a dyn CatalogRepository,
    usage_repo: &'a dyn UsageRepository,
}

impl<'a> GetPokemonUsageUseCase<'a> {
    pub fn new(
        catalog_repo: &'a dyn CatalogRepository,
        usage_repo: &'a dyn UsageRepository,
    ) -> Self {
        Self {
            catalog_repo,
            usage_repo,
        }
    }

    pub fn execute(
        &self,
        query: GetPokemonUsageQuery,
    ) -> Result<GetPokemonUsageResult, GetPokemonUsageError> {
        let usage = match self.catalog_repo.find_species_id_by_name(&query.name)? {
            Some(pokemon_id) => self
                .usage_repo
                .find_by_pokemon_id(pokemon_id)?
                .or(self.usage_repo.find_by_pokemon_name(&query.name)?),
            None => self.usage_repo.find_by_pokemon_name(&query.name)?,
        };

        Ok(GetPokemonUsageResult { usage })
    }
}
