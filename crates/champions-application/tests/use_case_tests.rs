use champions_application::errors::*;
use champions_application::ports::*;
use champions_application::use_cases::*;
use champions_domain::battle::DamageInput;
use champions_domain::catalog::{BattleMasterData, MoveData, NatureData};
use champions_domain::party::{EffortValueSpread, MoveSet, PokemonBuild, SavedParty};
use champions_domain::recognition::ScreenState;
use champions_domain::usage::{EffortValueUsage, MoveUsage, NatureUsage, PokemonUsageSummary};
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
    species_ids: HashMap<String, u32>,
    moves: Vec<String>,
    move_ids: HashMap<String, u32>,
    items: Vec<String>,
    natures: Vec<String>,
    nature_ids: HashMap<String, u32>,
    abilities: Vec<String>,
    master_data: BattleMasterData,
}

impl FakeCatalogRepository {
    fn with_species(names: Vec<&str>) -> Self {
        let species: Vec<String> = names.into_iter().map(|s| s.to_string()).collect();
        let species_ids = species
            .iter()
            .enumerate()
            .map(|(index, name)| (name.clone(), index as u32 + 1))
            .collect();
        Self {
            species,
            species_ids,
            moves: vec!["10まんボルト".to_string(), "かみなり".to_string()],
            move_ids: HashMap::from([
                ("10まんボルト".to_string(), 10),
                ("かみなり".to_string(), 20),
            ]),
            items: vec!["こだわりメガネ".to_string()],
            natures: vec!["ひかえめ".to_string()],
            nature_ids: HashMap::from([("ひかえめ".to_string(), 1)]),
            abilities: vec!["せいでんき".to_string()],
            master_data: fixture_master_data(),
        }
    }

    fn suggest_names(names: &[String], query: &str, limit: usize) -> Vec<String> {
        if limit == 0 {
            return Vec::new();
        }
        if query.is_empty() {
            return names.iter().take(limit).cloned().collect();
        }

        let normalized_query = Self::normalize_for_match(query);
        let mut prefix_matches = Vec::new();
        let mut contains_matches = Vec::new();

        for name in names {
            let normalized_name = Self::normalize_for_match(name);
            if normalized_name.starts_with(&normalized_query) {
                prefix_matches.push(name.clone());
            } else if normalized_name.contains(&normalized_query) {
                contains_matches.push(name.clone());
            }
        }

        prefix_matches.extend(
            contains_matches
                .into_iter()
                .take(limit.saturating_sub(prefix_matches.len())),
        );
        prefix_matches.truncate(limit);
        prefix_matches
    }

    fn normalize_for_match(value: &str) -> String {
        value.chars().map(Self::normalize_kana_char).collect()
    }

    fn normalize_kana_char(ch: char) -> char {
        match ch {
            '\u{30A1}'..='\u{30F6}' => char::from_u32(ch as u32 - 0x60).unwrap_or(ch),
            _ => ch,
        }
    }
}

impl CatalogRepository for FakeCatalogRepository {
    fn suggest_species(&self, query: &str, limit: usize) -> Result<Vec<String>, CatalogError> {
        Ok(Self::suggest_names(&self.species, query, limit))
    }

    fn suggest_moves(&self, query: &str, limit: usize) -> Result<Vec<String>, CatalogError> {
        Ok(Self::suggest_names(&self.moves, query, limit))
    }

    fn suggest_items(&self, query: &str, limit: usize) -> Result<Vec<String>, CatalogError> {
        Ok(Self::suggest_names(&self.items, query, limit))
    }

    fn suggest_natures(&self, query: &str, limit: usize) -> Result<Vec<String>, CatalogError> {
        Ok(Self::suggest_names(&self.natures, query, limit))
    }

    fn suggest_abilities(&self, query: &str, limit: usize) -> Result<Vec<String>, CatalogError> {
        Ok(Self::suggest_names(&self.abilities, query, limit))
    }

    fn find_species_id_by_name(&self, name: &str) -> Result<Option<u32>, CatalogError> {
        Ok(self.species_ids.get(name.trim()).copied())
    }

    fn find_move_id_by_name(&self, name: &str) -> Result<Option<u32>, CatalogError> {
        Ok(self.move_ids.get(name.trim()).copied())
    }

    fn find_nature_id_by_name(&self, name: &str) -> Result<Option<u32>, CatalogError> {
        Ok(self.nature_ids.get(name.trim()).copied())
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
    fn find_by_pokemon_id(
        &self,
        pokemon_id: u32,
    ) -> Result<Option<PokemonUsageSummary>, UsageError> {
        let read = self.data.read().unwrap();
        Ok(read.iter().find(|u| u.pokemon_id == pokemon_id).cloned())
    }

    fn find_many_by_pokemon_ids(
        &self,
        pokemon_ids: &[u32],
    ) -> Result<Vec<PokemonUsageSummary>, UsageError> {
        let read = self.data.read().unwrap();
        Ok(read
            .iter()
            .filter(|u| pokemon_ids.contains(&u.pokemon_id))
            .cloned()
            .collect())
    }

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

struct FakeOcrEngine {
    text: String,
}

impl FakeOcrEngine {
    fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

impl OcrEngine for FakeOcrEngine {
    fn recognize_selection_text(&self, _image: &OcrImage) -> Result<String, OcrError> {
        Ok(self.text.clone())
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

fn sample_pokemon(name: &str) -> PokemonBuild {
    PokemonBuild {
        species_name: name.to_string(),
        ..Default::default()
    }
}

fn sample_ocr_image() -> OcrImage {
    OcrImage {
        width: 1,
        height: 1,
        rgb_bytes: vec![255, 255, 255],
    }
}

// --- LoadPartyUseCase Tests ---

#[test]
fn load_party_returns_saved_party() {
    let party = SavedParty {
        pokemons: vec![sample_pokemon("ピカチュウ")],
        ..Default::default()
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
        ..Default::default()
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
        ..Default::default()
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
fn suggest_species_supports_partial_matching() {
    let catalog =
        FakeCatalogRepository::with_species(vec!["ピカチュウ", "ライチュウ", "ピジョット"]);
    let uc = SuggestNamesUseCase::new(&catalog);

    let result = uc
        .execute(SuggestNamesQuery {
            kind: SuggestKind::Species,
            query: "チュ".to_string(),
            limit: 10,
        })
        .unwrap();

    assert_eq!(
        result.suggestions,
        vec!["ピカチュウ".to_string(), "ライチュウ".to_string()]
    );
}

#[test]
fn suggest_moves_supports_partial_matching() {
    let catalog = FakeCatalogRepository::with_species(vec![]);
    let uc = SuggestNamesUseCase::new(&catalog);

    let result = uc
        .execute(SuggestNamesQuery {
            kind: SuggestKind::Move,
            query: "ボル".to_string(),
            limit: 10,
        })
        .unwrap();

    assert_eq!(result.suggestions, vec!["10まんボルト"]);
}

#[test]
fn suggest_natures_supports_kana_insensitive_matching() {
    let catalog = FakeCatalogRepository::with_species(vec![]);
    let uc = SuggestNamesUseCase::new(&catalog);

    let result = uc
        .execute(SuggestNamesQuery {
            kind: SuggestKind::Nature,
            query: "カエ".to_string(),
            limit: 10,
        })
        .unwrap();

    assert_eq!(result.suggestions, vec!["ひかえめ"]);
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
    let fetched = vec![sample_usage(25, "ピカチュウ"), sample_usage(6, "リザードン")];
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
    let catalog = FakeCatalogRepository::with_species(vec!["ピカチュウ"]);
    let repo = FakeUsageRepository::new(vec![sample_usage(1, "ピカチュウ")]);
    let uc = GetPokemonUsageUseCase::new(&catalog, &repo);

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
    let catalog = FakeCatalogRepository::with_species(vec!["ピカチュウ"]);
    let repo = FakeUsageRepository::empty();
    let uc = GetPokemonUsageUseCase::new(&catalog, &repo);

    let result = uc
        .execute(GetPokemonUsageQuery {
            name: "フシギダネ".to_string(),
        })
        .unwrap();

    assert!(result.usage.is_none());
}

// --- DetectSelectionScreenUseCase Tests ---

#[test]
fn detect_selection_screen_accepts_text_with_three_hint_chars() {
    let ocr = FakeOcrEngine::new("対戦準備\nシグバ");
    let uc = DetectSelectionScreenUseCase::new(&ocr);

    let result = uc
        .execute(DetectSelectionScreenCommand {
            target_text_image: sample_ocr_image(),
        })
        .unwrap();

    assert_eq!(result.screen_state, ScreenState::SelectionScreen);
}

#[test]
fn detect_selection_screen_rejects_text_with_only_two_hint_chars() {
    let ocr = FakeOcrEngine::new("対戦準備\nシバ");
    let uc = DetectSelectionScreenUseCase::new(&ocr);

    let result = uc
        .execute(DetectSelectionScreenCommand {
            target_text_image: sample_ocr_image(),
        })
        .unwrap();

    assert_eq!(result.screen_state, ScreenState::Other);
}

#[test]
fn detect_battle_result_phase_accepts_win_text() {
    let ocr = FakeOcrEngine::new("YOU WIN");
    let uc = DetectBattleResultPhaseUseCase::new(&ocr);

    let result = uc
        .execute(DetectBattleResultPhaseCommand {
            target_text_image: sample_ocr_image(),
        })
        .unwrap();

    assert!(result);
}

#[test]
fn detect_battle_result_phase_accepts_lose_text() {
    let ocr = FakeOcrEngine::new("LOSE");
    let uc = DetectBattleResultPhaseUseCase::new(&ocr);

    let result = uc
        .execute(DetectBattleResultPhaseCommand {
            target_text_image: sample_ocr_image(),
        })
        .unwrap();

    assert!(result);
}

#[test]
fn detect_battle_result_phase_rejects_other_text() {
    let ocr = FakeOcrEngine::new("ターン 3");
    let uc = DetectBattleResultPhaseUseCase::new(&ocr);

    let result = uc
        .execute(DetectBattleResultPhaseCommand {
            target_text_image: sample_ocr_image(),
        })
        .unwrap();

    assert!(!result);
}

#[test]
fn build_selection_support_calculates_speed_and_two_hit_ko() {
    let catalog = FakeCatalogRepository::with_species(vec!["アタッカー", "タンク"]);
    let repo = FakeUsageRepository::new(vec![PokemonUsageSummary {
        pokemon_id: 2,
        name: "タンク".to_string(),
        types: vec!["じめん".to_string()],
        moves: vec![MoveUsage {
            name: "10まんボルト".to_string(),
            rate: "100%".to_string(),
        }],
        items: vec![],
        abilities: vec![],
        effort_values: vec![EffortValueUsage {
            h: 0,
            a: 0,
            b: 0,
            c: 0,
            d: 0,
            s: 0,
            rate: "100%".to_string(),
        }],
        natures: vec![NatureUsage {
            name: "ひかえめ".to_string(),
            rate: "100%".to_string(),
        }],
    }]);
    let uc = BuildSelectionSupportUseCase::new(&catalog, &repo);

    let result = uc
        .execute(BuildSelectionSupportQuery {
            my_party: vec![PokemonBuild {
                species_name: "アタッカー".to_string(),
                effort_values: EffortValueSpread {
                    h: 175,
                    a: 150,
                    b: 100,
                    c: 80,
                    d: 90,
                    s: 130,
                },
                moves: MoveSet {
                    moves: [
                        "10まんボルト".to_string(),
                        String::new(),
                        String::new(),
                        String::new(),
                    ],
                },
                ..Default::default()
            }],
            opponents: vec![OpponentSelectionInput {
                slot_index: 0,
                name: "タンク".to_string(),
            }],
        })
        .unwrap();

    assert_eq!(result.opponents.len(), 1);
    let opponent = &result.opponents[0];
    let assumption = opponent.assumption.as_ref().unwrap();
    assert_eq!(assumption.stats, [175, 100, 140, 80, 100, 80]);

    let matchup = &opponent.matchups[0];
    let speed = matchup.speed.as_ref().unwrap();
    assert_eq!(speed.my_first_chance_percent, 100.0);
    assert_eq!(speed.opponent_first_chance_percent, 0.0);

    let my_attack = matchup.my_attack.as_ref().unwrap();
    assert_eq!(my_attack.move_name, "10まんボルト");
    assert_eq!(my_attack.guaranteed_hits, Some(2));
    match &my_attack.ko_summary {
        KoSummary::TwoHit { chance_percent } => assert_eq!(*chance_percent, 100.0),
        _ => panic!("expected two-hit ko summary"),
    }

    assert!(matchup.opponent_attack.is_some());
}

#[test]
fn build_selection_support_uses_usage_pokemon_id_when_catalog_name_lookup_fails() {
    let catalog = FakeCatalogRepository::with_species(vec!["アタッカー"]);
    let repo = FakeUsageRepository::new(vec![PokemonUsageSummary {
        pokemon_id: 2,
        name: "タンク(別フォーム)".to_string(),
        types: vec!["じめん".to_string()],
        moves: vec![MoveUsage {
            name: "10まんボルト".to_string(),
            rate: "100%".to_string(),
        }],
        items: vec![],
        abilities: vec![],
        effort_values: vec![EffortValueUsage {
            h: 0,
            a: 0,
            b: 0,
            c: 0,
            d: 0,
            s: 0,
            rate: "100%".to_string(),
        }],
        natures: vec![NatureUsage {
            name: "ひかえめ".to_string(),
            rate: "100%".to_string(),
        }],
    }]);
    let uc = BuildSelectionSupportUseCase::new(&catalog, &repo);

    let result = uc
        .execute(BuildSelectionSupportQuery {
            my_party: vec![PokemonBuild {
                species_name: "アタッカー".to_string(),
                effort_values: EffortValueSpread {
                    h: 175,
                    a: 150,
                    b: 100,
                    c: 80,
                    d: 90,
                    s: 130,
                },
                moves: MoveSet {
                    moves: [
                        "10まんボルト".to_string(),
                        String::new(),
                        String::new(),
                        String::new(),
                    ],
                },
                ..Default::default()
            }],
            opponents: vec![OpponentSelectionInput {
                slot_index: 0,
                name: "タンク(別フォーム)".to_string(),
            }],
        })
        .unwrap();

    let opponent = &result.opponents[0];
    assert!(opponent.note.is_none());
    assert!(opponent.assumption.is_some());
    assert_eq!(opponent.assumption.as_ref().unwrap().stats, [175, 100, 140, 80, 100, 80]);
}

#[test]
fn build_selection_support_uses_usage_distribution_points_for_opponent_stats() {
    let catalog = FakeCatalogRepository::with_species(vec!["アタッカー", "タンク"]);
    let repo = FakeUsageRepository::new(vec![PokemonUsageSummary {
        pokemon_id: 2,
        name: "タンク".to_string(),
        types: vec!["じめん".to_string()],
        moves: vec![MoveUsage {
            name: "10まんボルト".to_string(),
            rate: "100%".to_string(),
        }],
        items: vec![],
        abilities: vec![],
        effort_values: vec![EffortValueUsage {
            h: 0,
            a: 0,
            b: 0,
            c: 32,
            d: 0,
            s: 32,
            rate: "100%".to_string(),
        }],
        natures: vec![NatureUsage {
            name: "おくびょう".to_string(),
            rate: "100%".to_string(),
        }],
    }]);
    let uc = BuildSelectionSupportUseCase::new(&catalog, &repo);

    let result = uc
        .execute(BuildSelectionSupportQuery {
            my_party: vec![PokemonBuild {
                species_name: "アタッカー".to_string(),
                effort_values: EffortValueSpread {
                    h: 175,
                    a: 150,
                    b: 100,
                    c: 80,
                    d: 90,
                    s: 110,
                },
                ..Default::default()
            }],
            opponents: vec![OpponentSelectionInput {
                slot_index: 0,
                name: "タンク".to_string(),
            }],
        })
        .unwrap();

    let opponent = &result.opponents[0];
    let assumption = opponent.assumption.as_ref().unwrap();
    assert_eq!(assumption.stats, [175, 90, 140, 112, 100, 123]);

    let speed = opponent.matchups[0].speed.as_ref().unwrap();
    assert_eq!(speed.my_speed, 110);
    assert_eq!(speed.opponent_speed, 123);
    assert_eq!(speed.my_first_chance_percent, 0.0);
    assert_eq!(speed.opponent_first_chance_percent, 100.0);
}

#[test]
fn build_selection_support_reports_missing_usage() {
    let catalog = FakeCatalogRepository::with_species(vec!["アタッカー", "タンク"]);
    let repo = FakeUsageRepository::empty();
    let uc = BuildSelectionSupportUseCase::new(&catalog, &repo);

    let result = uc
        .execute(BuildSelectionSupportQuery {
            my_party: vec![sample_pokemon("アタッカー")],
            opponents: vec![OpponentSelectionInput {
                slot_index: 1,
                name: "タンク".to_string(),
            }],
        })
        .unwrap();

    assert_eq!(result.opponents.len(), 1);
    assert!(result.opponents[0].assumption.is_none());
    assert_eq!(
        result.opponents[0].note.as_deref(),
        Some("使用率データがないため相性を計算できません")
    );
}

#[test]
fn build_selection_support_applies_nature_without_catalog_nature_id() {
    let mut catalog = FakeCatalogRepository::with_species(vec!["アタッカー", "タンク"]);
    catalog.nature_ids.clear();
    let repo = FakeUsageRepository::new(vec![PokemonUsageSummary {
        pokemon_id: 2,
        name: "タンク".to_string(),
        types: vec!["じめん".to_string()],
        moves: vec![MoveUsage {
            name: "10まんボルト".to_string(),
            rate: "100%".to_string(),
        }],
        items: vec![],
        abilities: vec![],
        effort_values: vec![EffortValueUsage {
            h: 0,
            a: 0,
            b: 0,
            c: 0,
            d: 0,
            s: 0,
            rate: "100%".to_string(),
        }],
        natures: vec![NatureUsage {
            name: "おくびょう".to_string(),
            rate: "100%".to_string(),
        }],
    }]);
    let uc = BuildSelectionSupportUseCase::new(&catalog, &repo);

    let result = uc
        .execute(BuildSelectionSupportQuery {
            my_party: vec![sample_pokemon("アタッカー")],
            opponents: vec![OpponentSelectionInput {
                slot_index: 0,
                name: "タンク".to_string(),
            }],
        })
        .unwrap();

    let assumption = result.opponents[0].assumption.as_ref().unwrap();
    assert_eq!(assumption.nature_name.as_deref(), Some("おくびょう"));
    assert_eq!(assumption.stats[5], 88);
    assert_eq!(assumption.stats[1], 90);
}
