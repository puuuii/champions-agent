use champions_application::errors::*;
use champions_application::ports::*;
use champions_application::use_cases::*;
use champions_domain::battle::DamageInput;
use champions_domain::catalog::{BattleMasterData, MoveData, NatureData};
use champions_domain::party::{PokemonBuild, SavedParty};
use champions_domain::usage::{MoveUsage, PokemonUsageSummary};
use std::collections::HashMap;
use std::sync::RwLock;

// --- Fake Repositories ---

struct FakePartyRepository {
    party: RwLock<SavedParty>,
}

impl FakePartyRepository {
    fn new(party: SavedParty) -> Self {
        Self {
            party: RwLock::new(party),
        }
    }

    fn empty() -> Self {
        Self::new(SavedParty::default())
    }
}

impl PartyRepository for FakePartyRepository {
    fn load_my_party(&self) -> Result<SavedParty, PartyRepositoryError> {
        Ok(self.party.read().unwrap().clone())
    }

    fn save_my_party(&self, party: &SavedParty) -> Result<(), PartyRepositoryError> {
        *self.party.write().unwrap() = party.clone();
        Ok(())
    }
}

struct FakeCatalogRepository {
    species: Vec<String>,
    moves: Vec<String>,
    items: Vec<String>,
    natures: Vec<String>,
    abilities: Vec<String>,
    master_data: BattleMasterData,
}

impl FakeCatalogRepository {
    fn with_species(names: Vec<&str>) -> Self {
        Self {
            species: names.into_iter().map(|s| s.to_string()).collect(),
            moves: vec!["10まんボルト".to_string(), "かみなり".to_string()],
            items: vec!["こだわりメガネ".to_string()],
            natures: vec!["ひかえめ".to_string()],
            abilities: vec!["せいでんき".to_string()],
            master_data: fixture_master_data(),
        }
    }
}

impl CatalogRepository for FakeCatalogRepository {
    fn suggest_species(&self, query: &str, limit: usize) -> Result<Vec<String>, CatalogError> {
        Ok(self
            .species
            .iter()
            .filter(|s| s.starts_with(query))
            .take(limit)
            .cloned()
            .collect())
    }

    fn suggest_moves(&self, query: &str, limit: usize) -> Result<Vec<String>, CatalogError> {
        Ok(self
            .moves
            .iter()
            .filter(|s| s.starts_with(query))
            .take(limit)
            .cloned()
            .collect())
    }

    fn suggest_items(&self, query: &str, limit: usize) -> Result<Vec<String>, CatalogError> {
        Ok(self
            .items
            .iter()
            .filter(|s| s.starts_with(query))
            .take(limit)
            .cloned()
            .collect())
    }

    fn suggest_natures(&self, query: &str, limit: usize) -> Result<Vec<String>, CatalogError> {
        Ok(self
            .natures
            .iter()
            .filter(|s| s.starts_with(query))
            .take(limit)
            .cloned()
            .collect())
    }

    fn suggest_abilities(&self, query: &str, limit: usize) -> Result<Vec<String>, CatalogError> {
        Ok(self
            .abilities
            .iter()
            .filter(|s| s.starts_with(query))
            .take(limit)
            .cloned()
            .collect())
    }

    fn load_battle_master_data(&self) -> Result<BattleMasterData, CatalogError> {
        Ok(self.master_data.clone())
    }
}

struct FakeUsageRepository {
    data: RwLock<Vec<PokemonUsageSummary>>,
}

impl FakeUsageRepository {
    fn new(data: Vec<PokemonUsageSummary>) -> Self {
        Self {
            data: RwLock::new(data),
        }
    }

    fn empty() -> Self {
        Self::new(Vec::new())
    }
}

impl UsageRepository for FakeUsageRepository {
    fn find_by_pokemon_name(&self, name: &str) -> Result<Option<PokemonUsageSummary>, UsageError> {
        let read = self.data.read().unwrap();
        Ok(read.iter().find(|u| u.name == name).cloned())
    }

    fn find_many_by_names(&self, names: &[String]) -> Result<Vec<PokemonUsageSummary>, UsageError> {
        let read = self.data.read().unwrap();
        Ok(read
            .iter()
            .filter(|u| names.contains(&u.name))
            .cloned()
            .collect())
    }

    fn replace_all(&self, data: Vec<PokemonUsageSummary>) -> Result<(), UsageError> {
        *self.data.write().unwrap() = data;
        Ok(())
    }
}

struct FakeUsageFetcher {
    result: Vec<PokemonUsageSummary>,
}

impl FakeUsageFetcher {
    fn new(result: Vec<PokemonUsageSummary>) -> Self {
        Self { result }
    }
}

impl UsageFetcher for FakeUsageFetcher {
    fn fetch_usage(
        &self,
        _source: UsageSource,
    ) -> Result<Vec<PokemonUsageSummary>, UsageFetchError> {
        Ok(self.result.clone())
    }
}

// --- Helpers ---

fn fixture_master_data() -> BattleMasterData {
    let mut pokemon_stats = HashMap::new();
    pokemon_stats.insert(1, [100_u32, 130, 80, 60, 70, 110]);
    pokemon_stats.insert(2, [100, 80, 120, 60, 80, 60]);

    let mut moves = HashMap::new();
    moves.insert(
        10,
        MoveData {
            id: 10,
            type_id: 1,
            power: Some(80),
            damage_class_id: 2,
        },
    );

    let mut natures = HashMap::new();
    natures.insert(
        1,
        NatureData {
            id: 1,
            increased_stat_id: 0,
            decreased_stat_id: 0,
        },
    );

    let mut type_efficacy = HashMap::new();
    type_efficacy.insert((1, 2), 200);

    let mut pokemon_types = HashMap::new();
    pokemon_types.insert(1, vec![1]);
    pokemon_types.insert(2, vec![2]);

    BattleMasterData {
        pokemon_stats,
        moves,
        natures,
        type_efficacy,
        pokemon_types,
    }
}

fn sample_usage(name: &str) -> PokemonUsageSummary {
    PokemonUsageSummary {
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

fn sample_pokemon(name: &str) -> PokemonBuild {
    PokemonBuild {
        species_name: name.to_string(),
        ..Default::default()
    }
}

// --- LoadPartyUseCase Tests ---

#[test]
fn load_party_returns_saved_party() {
    let party = SavedParty {
        pokemons: vec![sample_pokemon("ピカチュウ")],
    };
    let repo = FakePartyRepository::new(party.clone());
    let uc = LoadPartyUseCase::new(&repo);

    let result = uc.execute().unwrap();
    assert_eq!(result.party.pokemons.len(), 1);
    assert_eq!(result.party.pokemons[0].species_name, "ピカチュウ");
}

#[test]
fn load_party_returns_empty_when_no_data() {
    let repo = FakePartyRepository::empty();
    let uc = LoadPartyUseCase::new(&repo);

    let result = uc.execute().unwrap();
    assert!(result.party.pokemons.is_empty());
}

// --- SavePartyUseCase Tests ---

#[test]
fn save_party_persists_data() {
    let repo = FakePartyRepository::empty();
    let uc = SavePartyUseCase::new(&repo);

    let party = SavedParty {
        pokemons: vec![sample_pokemon("リザードン"), sample_pokemon("フシギダネ")],
    };
    let result = uc.execute(SavePartyCommand { party }).unwrap();

    assert_eq!(result.saved_count, 2);
    assert!(result.warnings.is_empty());

    let loaded = repo.load_my_party().unwrap();
    assert_eq!(loaded.pokemons.len(), 2);
}

#[test]
fn save_party_warns_on_empty_species_name() {
    let repo = FakePartyRepository::empty();
    let uc = SavePartyUseCase::new(&repo);

    let party = SavedParty {
        pokemons: vec![sample_pokemon(""), sample_pokemon("ピカチュウ")],
    };
    let result = uc.execute(SavePartyCommand { party }).unwrap();

    assert_eq!(result.saved_count, 2);
    assert_eq!(result.warnings.len(), 1);
    assert_eq!(result.warnings[0].pokemon_index, 0);
}

// --- SuggestNamesUseCase Tests ---

#[test]
fn suggest_species_returns_matching() {
    let catalog =
        FakeCatalogRepository::with_species(vec!["ピカチュウ", "ピジョット", "リザードン"]);
    let uc = SuggestNamesUseCase::new(&catalog);

    let result = uc
        .execute(SuggestNamesQuery {
            kind: SuggestKind::Species,
            query: "ピ".to_string(),
            limit: 10,
        })
        .unwrap();

    assert_eq!(result.suggestions.len(), 2);
    assert!(result.suggestions.contains(&"ピカチュウ".to_string()));
    assert!(result.suggestions.contains(&"ピジョット".to_string()));
}

#[test]
fn suggest_moves_returns_matching() {
    let catalog = FakeCatalogRepository::with_species(vec![]);
    let uc = SuggestNamesUseCase::new(&catalog);

    let result = uc
        .execute(SuggestNamesQuery {
            kind: SuggestKind::Move,
            query: "10".to_string(),
            limit: 10,
        })
        .unwrap();

    assert_eq!(result.suggestions, vec!["10まんボルト"]);
}

#[test]
fn suggest_with_empty_query_returns_all_up_to_limit() {
    let catalog =
        FakeCatalogRepository::with_species(vec!["ピカチュウ", "リザードン", "フシギダネ"]);
    let uc = SuggestNamesUseCase::new(&catalog);

    let result = uc
        .execute(SuggestNamesQuery {
            kind: SuggestKind::Species,
            query: "".to_string(),
            limit: 2,
        })
        .unwrap();

    assert_eq!(result.suggestions.len(), 2);
}

// --- CalculateDamageUseCase Tests ---

#[test]
fn calculate_damage_returns_result() {
    let catalog = FakeCatalogRepository::with_species(vec![]);
    let uc = CalculateDamageUseCase::new(&catalog);

    let input = DamageInput {
        attacker_id: 1,
        defender_id: 2,
        move_id: 10,
        attacker_ap: [0; 6],
        defender_ap: [0; 6],
        attacker_nature_id: 1,
        defender_nature_id: 1,
        attacker_stages: [0; 8],
        defender_stages: [0; 8],
        is_critical: false,
        rng_roll: 1.0,
    };

    let result = uc.execute(CalculateDamageCommand { input }).unwrap();

    assert!(result.damage > 0);
}

#[test]
fn calculate_damage_returns_error_for_missing_pokemon() {
    let catalog = FakeCatalogRepository::with_species(vec![]);
    let uc = CalculateDamageUseCase::new(&catalog);

    let input = DamageInput {
        attacker_id: 999,
        defender_id: 2,
        move_id: 10,
        attacker_ap: [0; 6],
        defender_ap: [0; 6],
        attacker_nature_id: 1,
        defender_nature_id: 1,
        attacker_stages: [0; 8],
        defender_stages: [0; 8],
        is_critical: false,
        rng_roll: 1.0,
    };

    let result = uc.execute(CalculateDamageCommand { input });
    assert!(result.is_err());
}

// --- RefreshUsageDataUseCase Tests ---

#[test]
fn refresh_usage_fetches_and_stores() {
    let fetched = vec![sample_usage("ピカチュウ"), sample_usage("リザードン")];
    let fetcher = FakeUsageFetcher::new(fetched);
    let repo = FakeUsageRepository::empty();
    let uc = RefreshUsageDataUseCase::new(&fetcher, &repo);

    let result = uc
        .execute(RefreshUsageDataCommand {
            source: UsageSource::GameWith,
        })
        .unwrap();

    assert_eq!(result.count, 2);

    let stored = repo.find_by_pokemon_name("ピカチュウ").unwrap();
    assert!(stored.is_some());
}

// --- GetPokemonUsageUseCase Tests ---

#[test]
fn get_pokemon_usage_returns_data() {
    let repo = FakeUsageRepository::new(vec![sample_usage("ピカチュウ")]);
    let uc = GetPokemonUsageUseCase::new(&repo);

    let result = uc
        .execute(GetPokemonUsageQuery {
            name: "ピカチュウ".to_string(),
        })
        .unwrap();

    assert!(result.usage.is_some());
    assert_eq!(result.usage.unwrap().name, "ピカチュウ");
}

#[test]
fn get_pokemon_usage_returns_none_when_not_found() {
    let repo = FakeUsageRepository::empty();
    let uc = GetPokemonUsageUseCase::new(&repo);

    let result = uc
        .execute(GetPokemonUsageQuery {
            name: "フシギダネ".to_string(),
        })
        .unwrap();

    assert!(result.usage.is_none());
}
