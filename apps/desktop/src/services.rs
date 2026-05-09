use champions_application::ports::{
    CatalogRepository, PartyRepository, UsageFetcher, UsageRepository, UsageSource,
};
use champions_application::use_cases::{
    GetPokemonUsageQuery, GetPokemonUsageUseCase, LoadPartyUseCase, RefreshUsageDataCommand,
    RefreshUsageDataUseCase, SavePartyCommand, SavePartyUseCase, SuggestKind, SuggestNamesQuery,
    SuggestNamesUseCase,
};
use champions_domain::{party::SavedParty, usage::PokemonUsageSummary};
use champions_interface::{
    EffortValueUsageView, ItemUsageView, MoveUsageView, NatureUsageView, PokemonUsageSummaryView,
};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuggestionKind {
    Species,
    Move,
    Item,
    Nature,
    Ability,
}

impl From<SuggestionKind> for SuggestKind {
    fn from(value: SuggestionKind) -> Self {
        match value {
            SuggestionKind::Species => SuggestKind::Species,
            SuggestionKind::Move => SuggestKind::Move,
            SuggestionKind::Item => SuggestKind::Item,
            SuggestionKind::Nature => SuggestKind::Nature,
            SuggestionKind::Ability => SuggestKind::Ability,
        }
    }
}

#[derive(Clone)]
pub struct DesktopAppServices {
    catalog_repo: Arc<dyn CatalogRepository>,
    party_repo: Arc<dyn PartyRepository>,
    usage_fetcher: Arc<dyn UsageFetcher>,
    usage_repo: Arc<dyn UsageRepository>,
}

impl DesktopAppServices {
    pub fn new(
        catalog_repo: Arc<dyn CatalogRepository>,
        party_repo: Arc<dyn PartyRepository>,
        usage_fetcher: Arc<dyn UsageFetcher>,
        usage_repo: Arc<dyn UsageRepository>,
    ) -> Self {
        Self {
            catalog_repo,
            party_repo,
            usage_fetcher,
            usage_repo,
        }
    }

    pub fn load_party(&self) -> Result<SavedParty, String> {
        LoadPartyUseCase::new(self.party_repo.as_ref())
            .execute()
            .map(|result| result.party)
            .map_err(|error| error.to_string())
    }

    pub fn save_party(&self, party: SavedParty) -> Result<(), String> {
        SavePartyUseCase::new(self.party_repo.as_ref())
            .execute(SavePartyCommand { party })
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub fn suggest_names(&self, kind: SuggestionKind, query: &str, limit: usize) -> Vec<String> {
        let query = query.trim();
        if query.is_empty() {
            return Vec::new();
        }

        SuggestNamesUseCase::new(self.catalog_repo.as_ref())
            .execute(SuggestNamesQuery {
                kind: kind.into(),
                query: query.to_string(),
                limit,
            })
            .map(|result| result.suggestions)
            .unwrap_or_default()
    }

    pub fn lookup_usage_summary_view(&self, name: &str) -> Option<PokemonUsageSummaryView> {
        let name = name.trim();
        if name.is_empty() {
            return None;
        }

        GetPokemonUsageUseCase::new(self.usage_repo.as_ref())
            .execute(GetPokemonUsageQuery {
                name: name.to_string(),
            })
            .ok()
            .and_then(|result| result.usage.as_ref().map(map_usage_summary_view))
    }

    pub fn refresh_usage_data(&self) -> Result<usize, String> {
        RefreshUsageDataUseCase::new(self.usage_fetcher.as_ref(), self.usage_repo.as_ref())
            .execute(RefreshUsageDataCommand {
                source: UsageSource::GameWith,
            })
            .map(|result| result.count)
            .map_err(|error| error.to_string())
    }
}

fn map_usage_summary_view(usage: &PokemonUsageSummary) -> PokemonUsageSummaryView {
    PokemonUsageSummaryView {
        name: usage.name.clone(),
        types: usage.types.clone(),
        moves: usage
            .moves
            .iter()
            .map(|move_usage| MoveUsageView {
                name: move_usage.name.clone(),
                rate: move_usage.rate.clone(),
            })
            .collect(),
        items: usage
            .items
            .iter()
            .map(|item_usage| ItemUsageView {
                name: item_usage.name.clone(),
                rate: item_usage.rate.clone(),
            })
            .collect(),
        effort_values: usage
            .effort_values
            .iter()
            .map(|effort_value_usage| EffortValueUsageView {
                h: effort_value_usage.h,
                a: effort_value_usage.a,
                b: effort_value_usage.b,
                c: effort_value_usage.c,
                d: effort_value_usage.d,
                s: effort_value_usage.s,
                rate: effort_value_usage.rate.clone(),
            })
            .collect(),
        natures: usage
            .natures
            .iter()
            .map(|nature_usage| NatureUsageView {
                name: nature_usage.name.clone(),
                rate: nature_usage.rate.clone(),
            })
            .collect(),
    }
}
