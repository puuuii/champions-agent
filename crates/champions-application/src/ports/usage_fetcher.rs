use crate::errors::UsageFetchError;
use champions_domain::usage::PokemonUsageSummary;

#[derive(Debug, Clone)]
pub enum UsageSource {
    GameWith,
}

pub trait UsageFetcher: Send + Sync {
    fn fetch_usage(&self, source: UsageSource)
    -> Result<Vec<PokemonUsageSummary>, UsageFetchError>;
}
