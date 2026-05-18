use crate::battle_selection::{BattleSelectionInferer, BattleSelectionObservation};
use champions_application::ports::{
    CatalogRepository, PartyRepository, UsageFetcher, UsageRepository, UsageSource,
};
use champions_application::use_cases::{
    BuildSelectionSupportQuery, BuildSelectionSupportResult, BuildSelectionSupportUseCase,
    GetPokemonUsageQuery, GetPokemonUsageUseCase, LoadPartyUseCase, OpponentSelectionInput,
    RefreshUsageDataCommand, RefreshUsageDataUseCase, SavePartyCommand, SavePartyUseCase,
    SuggestKind, SuggestNamesQuery, SuggestNamesUseCase,
};
use champions_domain::{
    party::{PokemonBuild, SavedParty},
    usage::PokemonUsageSummary,
};
use champions_interface::{
    AbilityUsageView, EffortValueUsageView, ItemUsageView, MoveUsageView, NatureUsageView,
    PokemonUsageSummaryView,
};
use champions_runtime::PixelFormat;
use std::sync::Arc;
use std::time::Instant;

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
    battle_selection_inferer: Option<Arc<BattleSelectionInferer>>,
}

impl DesktopAppServices {
    pub fn new(
        catalog_repo: Arc<dyn CatalogRepository>,
        party_repo: Arc<dyn PartyRepository>,
        usage_fetcher: Arc<dyn UsageFetcher>,
        usage_repo: Arc<dyn UsageRepository>,
        battle_selection_inferer: Option<Arc<BattleSelectionInferer>>,
    ) -> Self {
        Self {
            catalog_repo,
            party_repo,
            usage_fetcher,
            usage_repo,
            battle_selection_inferer,
        }
    }

    pub fn load_party(&self) -> Result<SavedParty, String> {
        match LoadPartyUseCase::new(self.party_repo.as_ref()).execute() {
            Ok(result) => {
                tracing::info!(
                    pokemons = result.party.pokemons.len(),
                    saved_pokemons = result.party.saved_pokemons.len(),
                    "saved party loaded",
                );
                Ok(result.party)
            }
            Err(error) => {
                tracing::error!(%error, "failed to load saved party");
                Err(error.to_string())
            }
        }
    }

    pub fn save_party(&self, party: SavedParty) -> Result<(), String> {
        let party_len = party.pokemons.len();
        let saved_pokemons_len = party.saved_pokemons.len();

        match SavePartyUseCase::new(self.party_repo.as_ref()).execute(SavePartyCommand { party }) {
            Ok(_) => {
                tracing::info!(
                    pokemons = party_len,
                    saved_pokemons = saved_pokemons_len,
                    "saved party persisted",
                );
                Ok(())
            }
            Err(error) => {
                tracing::error!(%error, "failed to persist saved party");
                Err(error.to_string())
            }
        }
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

        GetPokemonUsageUseCase::new(self.catalog_repo.as_ref(), self.usage_repo.as_ref())
            .execute(GetPokemonUsageQuery {
                name: name.to_string(),
            })
            .ok()
            .and_then(|result| result.usage.as_ref().map(map_usage_summary_view))
    }

    pub fn refresh_usage_data(&self) -> Result<usize, String> {
        let started_at = Instant::now();
        tracing::info!(source = ?UsageSource::GameWith, "refreshing usage data");

        match RefreshUsageDataUseCase::new(self.usage_fetcher.as_ref(), self.usage_repo.as_ref())
            .execute(RefreshUsageDataCommand {
                source: UsageSource::GameWith,
            }) {
            Ok(result) => {
                tracing::info!(
                    count = result.count,
                    elapsed_ms = started_at.elapsed().as_millis() as u64,
                    "usage data refreshed",
                );
                Ok(result.count)
            }
            Err(error) => {
                tracing::error!(
                    %error,
                    elapsed_ms = started_at.elapsed().as_millis() as u64,
                    "failed to refresh usage data",
                );
                Err(error.to_string())
            }
        }
    }

    pub fn build_selection_support(
        &self,
        my_party: Vec<PokemonBuild>,
        opponents: Vec<OpponentSelectionInput>,
    ) -> Result<BuildSelectionSupportResult, String> {
        tracing::debug!(
            my_party_len = my_party.len(),
            opponent_len = opponents.len(),
            "building selection support",
        );

        match BuildSelectionSupportUseCase::new(
            self.catalog_repo.as_ref(),
            self.usage_repo.as_ref(),
        )
        .execute(BuildSelectionSupportQuery {
            my_party,
            opponents,
        }) {
            Ok(result) => {
                tracing::debug!(
                    opponent_len = result.opponents.len(),
                    "selection support built",
                );
                Ok(result)
            }
            Err(error) => {
                tracing::error!(%error, "failed to build selection support");
                Err(error.to_string())
            }
        }
    }

    pub fn can_infer_battle_selection(&self) -> bool {
        self.battle_selection_inferer.is_some()
    }

    pub fn infer_battle_selection(
        &self,
        frame_width: u32,
        frame_height: u32,
        pixel_format: PixelFormat,
        frame_bytes: &[u8],
        my_candidates: Vec<String>,
        opponent_candidates: Vec<String>,
    ) -> Result<BattleSelectionObservation, String> {
        let Some(inferer) = self.battle_selection_inferer.as_ref() else {
            return Ok(BattleSelectionObservation::default());
        };

        inferer.infer_from_frame(
            frame_width,
            frame_height,
            pixel_format,
            frame_bytes,
            &my_candidates,
            &opponent_candidates,
        )
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
        abilities: usage
            .abilities
            .iter()
            .map(|ability_usage| AbilityUsageView {
                name: ability_usage.name.clone(),
                rate: ability_usage.rate.clone(),
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
