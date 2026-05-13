use crate::usage_id_mapping::resolve_master_pokemon_id;
use champions_application::errors::UsageFetchError;
use champions_application::ports::{UsageFetcher, UsageSource};
use champions_domain::usage::{
    AbilityUsage, EffortValueUsage, ItemUsage, MoveUsage, NatureUsage, PokemonUsageSummary,
};
use indexmap::IndexMap;
use regex::Regex;
use serde_json::Value;
use std::{sync::LazyLock, time::Instant};

static JS_OBJ_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)const pkchPokemonData\s*=\s*(\{.*?\});").unwrap());
static KEY_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b([A-Za-z_]\w*)\s*:").unwrap());
static TRAILING_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r",\s*([\]}])").unwrap());

const GAMEWITH_URL: &str = "https://gamewith.jp/pokemon-champions/555373";

#[derive(Default)]
pub struct GameWithUsageFetcher;

impl GameWithUsageFetcher {
    pub fn new() -> Self {
        Self
    }
}

impl UsageFetcher for GameWithUsageFetcher {
    fn fetch_usage(
        &self,
        source: UsageSource,
    ) -> Result<Vec<PokemonUsageSummary>, UsageFetchError> {
        let started_at = Instant::now();
        tracing::info!(?source, url = GAMEWITH_URL, "fetching usage data");

        let html = fetch_html(GAMEWITH_URL).map_err(|error| {
            tracing::error!(%error, url = GAMEWITH_URL, "failed to fetch usage HTML");
            UsageFetchError::FetchFailed(error.to_string())
        })?;

        let js_text = extract_js_object(&html).map_err(|error| {
            tracing::error!(%error, "failed to extract usage data JavaScript object");
            UsageFetchError::ParseFailed(error.to_string())
        })?;

        let json_text = js_to_json(&js_text);

        let raw_data: IndexMap<String, Value> =
            serde_json::from_str(&json_text).map_err(|error| {
                tracing::error!(%error, "failed to parse usage data JSON");
                UsageFetchError::ParseFailed(error.to_string())
            })?;

        let usage = build_pokemon_list(raw_data);
        tracing::info!(
            count = usage.len(),
            elapsed_ms = started_at.elapsed().as_millis() as u64,
            "usage data fetched",
        );

        Ok(usage)
    }
}

fn fetch_html(url: &str) -> Result<String, String> {
    tracing::debug!(url, "requesting usage HTML");
    let res = minreq::get(url)
        .with_header("User-Agent", "Mozilla/5.0")
        .send()
        .map_err(|e| e.to_string())?;

    res.as_str()
        .map(|s| s.to_string())
        .map_err(|e| e.to_string())
}

fn extract_js_object(html: &str) -> Result<String, String> {
    JS_OBJ_RE
        .captures(html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_owned())
        .ok_or_else(|| "pkchPokemonData not found".to_string())
}

fn js_to_json(js: &str) -> String {
    let step1 = KEY_RE.replace_all(js, "\"$1\":");
    let step2 = step1.replace('\'', "\"");
    TRAILING_RE.replace_all(&step2, "$1").into_owned()
}

fn build_pokemon_list(raw: IndexMap<String, Value>) -> Vec<PokemonUsageSummary> {
    raw.into_iter()
        .filter_map(|(gamewith_poke_id, p)| {
            let name = str_field(&p, "name");
            let Some(pokemon_id) = resolve_master_pokemon_id(&gamewith_poke_id, &name) else {
                tracing::warn!(gamewith_id = %gamewith_poke_id, %name, "failed to resolve GameWith Pokémon ID to master pokemon_id");
                return None;
            };

            Some(PokemonUsageSummary {
                pokemon_id,
                name,
                types: str_array(&p["types"]),
                moves: parse_moves(&p["moves"]),
                items: parse_items(&p["items"]),
                abilities: parse_abilities(&p["abilities"]),
                effort_values: parse_evs(&p["evDistributions"]),
                natures: parse_natures(&p["natures"]),
            })
        })
        .collect()
}

fn str_field(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or("").to_owned()
}

fn str_array(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn parse_moves(v: &Value) -> Vec<MoveUsage> {
    v.as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|x| {
                    let a = x.as_array()?;
                    Some(MoveUsage {
                        name: a.get(1)?.as_str()?.to_owned(),
                        rate: a.get(2)?.as_str().unwrap_or("").to_owned(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_items(v: &Value) -> Vec<ItemUsage> {
    v.as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|x| {
                    let a = x.as_array()?;
                    Some(ItemUsage {
                        name: a.first()?.as_str()?.to_owned(),
                        rate: a.get(1)?.as_str().unwrap_or("").to_owned(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_abilities(v: &Value) -> Vec<AbilityUsage> {
    v.as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|x| {
                    let a = x.as_array()?;
                    Some(AbilityUsage {
                        name: a.first()?.as_str()?.to_owned(),
                        rate: a.get(1)?.as_str().unwrap_or("").to_owned(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_natures(v: &Value) -> Vec<NatureUsage> {
    v.as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|x| {
                    let a = x.as_array()?;
                    Some(NatureUsage {
                        name: a.first()?.as_str()?.to_owned(),
                        rate: a.get(1)?.as_str().unwrap_or("").to_owned(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_evs(v: &Value) -> Vec<EffortValueUsage> {
    v.as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|x| {
                    let outer = x.as_array()?;
                    let stats = outer.first()?.as_array()?;
                    let rate = outer.get(1)?.as_str().unwrap_or("").to_owned();
                    let n = |i: usize| stats.get(i).and_then(Value::as_u64).unwrap_or(0) as u32;
                    Some(EffortValueUsage {
                        h: n(0),
                        a: n(1),
                        b: n(2),
                        c: n(3),
                        d: n(4),
                        s: n(5),
                        rate,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetch_usage_real() {
        let fetcher = GameWithUsageFetcher::new();
        let result = fetcher.fetch_usage(UsageSource::GameWith);
        match result {
            Ok(data) => {
                assert!(!data.is_empty());
            }
            Err(e) => {
                panic!("Fetch failed: {}", e);
            }
        }
    }
}
