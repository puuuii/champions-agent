use crate::errors::UsageError;
use crate::ports::UsageRepository;
use champions_domain::usage::PokemonUsageSummary;

pub struct GetPokemonUsageQuery {
    pub name: String,
}

pub struct GetPokemonUsageResult {
    pub usage: Option<PokemonUsageSummary>,
}

pub struct GetPokemonUsageUseCase<'a> {
    usage_repo: &'a dyn UsageRepository,
}

impl<'a> GetPokemonUsageUseCase<'a> {
    pub fn new(usage_repo: &'a dyn UsageRepository) -> Self {
        Self { usage_repo }
    }

    pub fn execute(
        &self,
        query: GetPokemonUsageQuery,
    ) -> Result<GetPokemonUsageResult, UsageError> {
        let usage = self.usage_repo.find_by_pokemon_name(&query.name)?;
        Ok(GetPokemonUsageResult { usage })
    }
}
