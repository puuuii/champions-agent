use crate::errors::{CatalogError, UsageError};
use crate::ports::{CatalogRepository, UsageRepository};
use champions_domain::battle::{DamageCalcError, calculate_damage_with_stats, resolve_stat_value};
use champions_domain::party::{EffortValueSpread, PokemonBuild};
use champions_domain::usage::{EffortValueUsage, PokemonUsageSummary};
use std::collections::HashMap;

const DAMAGE_RNG_ROLLS: [f64; 16] = [
    0.85, 0.86, 0.87, 0.88, 0.89, 0.90, 0.91, 0.92, 0.93, 0.94, 0.95, 0.96, 0.97, 0.98, 0.99, 1.0,
];

#[derive(Debug, Clone)]
pub struct BuildSelectionSupportQuery {
    pub my_party: Vec<PokemonBuild>,
    pub opponents: Vec<OpponentSelectionInput>,
}

#[derive(Debug, Clone)]
pub struct OpponentSelectionInput {
    pub slot_index: u8,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct BuildSelectionSupportResult {
    pub opponents: Vec<OpponentSelectionSupport>,
}

#[derive(Debug, Clone)]
pub struct OpponentSelectionSupport {
    pub slot_index: u8,
    pub opponent_name: String,
    pub assumption: Option<OpponentAssumption>,
    pub matchups: Vec<PokemonMatchupSupport>,
    pub note: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OpponentAssumption {
    pub effort_values: EffortValueSpread,
    pub nature_name: Option<String>,
    pub stats: [u32; 6],
}

#[derive(Debug, Clone)]
pub struct PokemonMatchupSupport {
    pub my_slot_index: usize,
    pub my_name: String,
    pub speed: Option<SpeedComparison>,
    pub my_attack: Option<AttackSupport>,
    pub opponent_attack: Option<AttackSupport>,
}

#[derive(Debug, Clone)]
pub struct SpeedComparison {
    pub my_speed: u32,
    pub opponent_speed: u32,
    pub my_first_chance_percent: f32,
    pub opponent_first_chance_percent: f32,
}

#[derive(Debug, Clone)]
pub struct AttackSupport {
    pub move_name: String,
    pub ko_summary: KoSummary,
    pub guaranteed_hits: Option<u8>,
    pub min_damage: u32,
    pub max_damage: u32,
}

#[derive(Debug, Clone)]
pub enum KoSummary {
    OneHit { chance_percent: f32 },
    TwoHit { chance_percent: f32 },
    MoreThanTwo,
}

#[derive(Debug, thiserror::Error)]
pub enum BuildSelectionSupportError {
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error(transparent)]
    Usage(#[from] UsageError),
    #[error(transparent)]
    Calculation(#[from] DamageCalcError),
}

pub struct BuildSelectionSupportUseCase<'a> {
    catalog_repo: &'a dyn CatalogRepository,
    usage_repo: &'a dyn UsageRepository,
}

impl<'a> BuildSelectionSupportUseCase<'a> {
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
        query: BuildSelectionSupportQuery,
    ) -> Result<BuildSelectionSupportResult, BuildSelectionSupportError> {
        let master = self.catalog_repo.load_battle_master_data()?;
        let opponent_names: Vec<String> = query
            .opponents
            .iter()
            .map(|opponent| opponent.name.trim().to_string())
            .filter(|name| !name.is_empty())
            .collect();
        let usage_by_name: HashMap<String, PokemonUsageSummary> = self
            .usage_repo
            .find_many_by_names(&opponent_names)?
            .into_iter()
            .map(|usage| (usage.name.clone(), usage))
            .collect();

        let my_party = query
            .my_party
            .into_iter()
            .enumerate()
            .map(|(slot_index, build)| {
                ResolvedMyPokemon::resolve(slot_index, build, self.catalog_repo)
            })
            .collect::<Result<Vec<_>, _>>()?;

        let mut opponents = Vec::new();
        for opponent in query.opponents {
            let opponent_name = opponent.name.trim().to_string();
            if opponent_name.is_empty() {
                continue;
            }

            let Some(usage) = usage_by_name.get(&opponent_name) else {
                opponents.push(OpponentSelectionSupport {
                    slot_index: opponent.slot_index,
                    opponent_name,
                    assumption: None,
                    matchups: Vec::new(),
                    note: Some("使用率データがないため相性を計算できません".to_string()),
                });
                continue;
            };

            let Some(opponent_species_id) =
                self.catalog_repo.find_species_id_by_name(&opponent_name)?
            else {
                opponents.push(OpponentSelectionSupport {
                    slot_index: opponent.slot_index,
                    opponent_name,
                    assumption: None,
                    matchups: Vec::new(),
                    note: Some("種族値の参照に必要なポケモンIDを解決できません".to_string()),
                });
                continue;
            };

            let assumed_evs = top_effort_values(usage);
            let assumed_nature_name = top_nature_name(usage);
            let assumed_nature_id = match assumed_nature_name.as_deref() {
                Some(name) => self.catalog_repo.find_nature_id_by_name(name)?.unwrap_or(0),
                None => 0,
            };
            let opponent_move_names = usage
                .moves
                .iter()
                .map(|move_usage| move_usage.name.clone())
                .collect::<Vec<_>>();
            let Some(opponent_stats) = build_assumed_stats(
                &master,
                opponent_species_id,
                &assumed_evs,
                assumed_nature_id,
            ) else {
                opponents.push(OpponentSelectionSupport {
                    slot_index: opponent.slot_index,
                    opponent_name,
                    assumption: None,
                    matchups: Vec::new(),
                    note: Some("想定能力値の計算に必要な種族値が見つかりません".to_string()),
                });
                continue;
            };

            let mut matchups = Vec::new();
            for my_pokemon in &my_party {
                let speed = build_speed_comparison(my_pokemon.actual_stats[5], opponent_stats[5]);
                let my_attack = match my_pokemon.species_id {
                    Some(my_species_id) => best_attack_support(
                        self.catalog_repo,
                        &master,
                        my_species_id,
                        my_pokemon.actual_stats,
                        &my_pokemon.moves,
                        opponent_species_id,
                        opponent_stats,
                    )?,
                    None => None,
                };
                let opponent_attack = match my_pokemon.species_id {
                    Some(my_species_id) => best_attack_support(
                        self.catalog_repo,
                        &master,
                        opponent_species_id,
                        opponent_stats,
                        &opponent_move_names,
                        my_species_id,
                        my_pokemon.actual_stats,
                    )?,
                    None => None,
                };

                matchups.push(PokemonMatchupSupport {
                    my_slot_index: my_pokemon.slot_index,
                    my_name: my_pokemon.display_name.clone(),
                    speed,
                    my_attack,
                    opponent_attack,
                });
            }

            opponents.push(OpponentSelectionSupport {
                slot_index: opponent.slot_index,
                opponent_name,
                assumption: Some(OpponentAssumption {
                    effort_values: assumed_evs,
                    nature_name: assumed_nature_name,
                    stats: opponent_stats,
                }),
                matchups,
                note: None,
            });
        }

        Ok(BuildSelectionSupportResult { opponents })
    }
}

#[derive(Debug, Clone)]
struct ResolvedMyPokemon {
    slot_index: usize,
    display_name: String,
    species_id: Option<u32>,
    actual_stats: [u32; 6],
    moves: Vec<String>,
}

impl ResolvedMyPokemon {
    fn resolve(
        slot_index: usize,
        build: PokemonBuild,
        catalog_repo: &dyn CatalogRepository,
    ) -> Result<Self, BuildSelectionSupportError> {
        let species_name = build.species_name.trim().to_string();
        let species_id = if species_name.is_empty() {
            None
        } else {
            catalog_repo.find_species_id_by_name(&species_name)?
        };
        let actual_stats = [
            build.effort_values.h,
            build.effort_values.a,
            build.effort_values.b,
            build.effort_values.c,
            build.effort_values.d,
            build.effort_values.s,
        ];
        let moves = build
            .moves
            .moves
            .iter()
            .map(|move_name| move_name.trim().to_string())
            .filter(|move_name| !move_name.is_empty())
            .collect();

        Ok(Self {
            slot_index,
            display_name: if species_name.is_empty() {
                format!("自分#{}", slot_index + 1)
            } else {
                species_name
            },
            species_id,
            actual_stats,
            moves,
        })
    }
}

fn top_effort_values(usage: &PokemonUsageSummary) -> EffortValueSpread {
    usage
        .effort_values
        .iter()
        .max_by(|left, right| rate_score(&left.rate).total_cmp(&rate_score(&right.rate)))
        .map(to_effort_value_spread)
        .unwrap_or_default()
}

fn top_nature_name(usage: &PokemonUsageSummary) -> Option<String> {
    usage
        .natures
        .iter()
        .max_by(|left, right| rate_score(&left.rate).total_cmp(&rate_score(&right.rate)))
        .map(|nature| nature.name.clone())
        .filter(|name| !name.trim().is_empty())
}

fn to_effort_value_spread(effort_values: &EffortValueUsage) -> EffortValueSpread {
    EffortValueSpread {
        h: effort_values.h,
        a: effort_values.a,
        b: effort_values.b,
        c: effort_values.c,
        d: effort_values.d,
        s: effort_values.s,
    }
}

fn build_assumed_stats(
    master: &champions_domain::catalog::BattleMasterData,
    species_id: u32,
    effort_values: &EffortValueSpread,
    nature_id: u32,
) -> Option<[u32; 6]> {
    let base_stats = master.pokemon_stats.get(&species_id)?;
    let added_points = [
        ev_to_added_points(effort_values.h),
        ev_to_added_points(effort_values.a),
        ev_to_added_points(effort_values.b),
        ev_to_added_points(effort_values.c),
        ev_to_added_points(effort_values.d),
        ev_to_added_points(effort_values.s),
    ];

    let mut stats = [0; 6];
    for stat_idx in 0..6 {
        let nature = nature_multiplier(master, nature_id, stat_idx);
        stats[stat_idx] = resolve_stat_value(
            base_stats[stat_idx],
            added_points[stat_idx],
            nature,
            stat_idx == 0,
        );
    }
    Some(stats)
}

fn ev_to_added_points(effort_value: u32) -> u32 {
    (effort_value + 4) / 8
}

fn nature_multiplier(
    master: &champions_domain::catalog::BattleMasterData,
    nature_id: u32,
    stat_idx: usize,
) -> f64 {
    if stat_idx == 0 {
        return 1.0;
    }
    if let Some(nature) = master.natures.get(&nature_id) {
        if nature.increased_stat_id == (stat_idx + 1) as u32 {
            return 1.1;
        }
        if nature.decreased_stat_id == (stat_idx + 1) as u32 {
            return 0.9;
        }
    }
    1.0
}

fn build_speed_comparison(my_speed: u32, opponent_speed: u32) -> Option<SpeedComparison> {
    if my_speed == 0 || opponent_speed == 0 {
        return None;
    }

    let (my_first, opponent_first) = if my_speed > opponent_speed {
        (100.0, 0.0)
    } else if my_speed < opponent_speed {
        (0.0, 100.0)
    } else {
        (50.0, 50.0)
    };

    Some(SpeedComparison {
        my_speed,
        opponent_speed,
        my_first_chance_percent: my_first,
        opponent_first_chance_percent: opponent_first,
    })
}

fn best_attack_support(
    catalog_repo: &dyn CatalogRepository,
    master: &champions_domain::catalog::BattleMasterData,
    attacker_id: u32,
    attacker_stats: [u32; 6],
    move_names: &[String],
    defender_id: u32,
    defender_stats: [u32; 6],
) -> Result<Option<AttackSupport>, BuildSelectionSupportError> {
    if defender_stats[0] == 0 {
        return Ok(None);
    }

    let mut best: Option<(AttackSupport, u64)> = None;
    for move_name in move_names {
        let move_name = move_name.trim();
        if move_name.is_empty() {
            continue;
        }

        let Some(move_id) = catalog_repo.find_move_id_by_name(move_name)? else {
            continue;
        };
        let Some(move_data) = master.moves.get(&move_id) else {
            continue;
        };
        let Some((attacker_stat_idx, defender_stat_idx)) =
            damage_stat_indices(move_data.damage_class_id)
        else {
            continue;
        };
        if attacker_stats[attacker_stat_idx] == 0 || defender_stats[defender_stat_idx] == 0 {
            continue;
        }

        let damages = DAMAGE_RNG_ROLLS
            .into_iter()
            .map(|rng_roll| {
                calculate_damage_with_stats(
                    master,
                    attacker_id,
                    defender_id,
                    move_id,
                    attacker_stats,
                    defender_stats,
                    [0; 8],
                    [0; 8],
                    false,
                    rng_roll,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;

        let attack = build_attack_support(move_name.to_string(), &damages, defender_stats[0]);
        let score = damages.iter().map(|damage| u64::from(*damage)).sum::<u64>();

        let should_replace = best
            .as_ref()
            .map(|(best_attack, best_score)| {
                score > *best_score
                    || (score == *best_score && attack.min_damage > best_attack.min_damage)
                    || (score == *best_score
                        && attack.min_damage == best_attack.min_damage
                        && attack.max_damage > best_attack.max_damage)
            })
            .unwrap_or(true);

        if should_replace {
            best = Some((attack, score));
        }
    }

    Ok(best.map(|(attack, _)| attack))
}

fn damage_stat_indices(damage_class_id: u32) -> Option<(usize, usize)> {
    match damage_class_id {
        2 => Some((1, 2)),
        3 => Some((3, 4)),
        _ => None,
    }
}

fn build_attack_support(move_name: String, damages: &[u32], defender_hp: u32) -> AttackSupport {
    let min_damage = damages.iter().copied().min().unwrap_or(0);
    let max_damage = damages.iter().copied().max().unwrap_or(0);
    let ohko_count = damages
        .iter()
        .filter(|&&damage| damage >= defender_hp)
        .count();
    let ohko_chance = chance_percent(ohko_count, damages.len());

    let ko_summary = if ohko_chance > 0.0 {
        KoSummary::OneHit {
            chance_percent: ohko_chance,
        }
    } else {
        let two_hit_count = damages
            .iter()
            .flat_map(|first| damages.iter().map(move |second| first + second))
            .filter(|total_damage| *total_damage >= defender_hp)
            .count();
        let two_hit_chance = chance_percent(two_hit_count, damages.len() * damages.len());

        if two_hit_chance > 0.0 {
            KoSummary::TwoHit {
                chance_percent: two_hit_chance,
            }
        } else {
            KoSummary::MoreThanTwo
        }
    };

    AttackSupport {
        move_name,
        ko_summary,
        guaranteed_hits: guaranteed_hits(defender_hp, min_damage),
        min_damage,
        max_damage,
    }
}

fn guaranteed_hits(defender_hp: u32, min_damage: u32) -> Option<u8> {
    if min_damage == 0 {
        return None;
    }

    let hits = defender_hp.div_ceil(min_damage);
    Some(hits.min(u32::from(u8::MAX)) as u8)
}

fn chance_percent(successes: usize, total: usize) -> f32 {
    if successes == 0 || total == 0 {
        return 0.0;
    }

    (successes as f32 * 100.0) / total as f32
}

fn rate_score(rate: &str) -> f32 {
    rate.trim()
        .trim_end_matches('%')
        .trim_end_matches('％')
        .trim()
        .parse::<f32>()
        .unwrap_or(0.0)
}
