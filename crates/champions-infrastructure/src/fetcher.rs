use champions_application::errors::UsageFetchError;
use champions_application::ports::{UsageFetcher, UsageSource};
use champions_domain::usage::{
    EffortValueUsage, ItemUsage, MoveUsage, NatureUsage, PokemonUsageSummary,
};
use indexmap::IndexMap;
use regex::Regex;
use serde_json::Value;
use std::sync::LazyLock;

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
        _source: UsageSource,
    ) -> Result<Vec<PokemonUsageSummary>, UsageFetchError> {
        let html =
            fetch_html(GAMEWITH_URL).map_err(|e| UsageFetchError::FetchFailed(e.to_string()))?;
        let js_text =
            extract_js_object(&html).map_err(|e| UsageFetchError::ParseFailed(e.to_string()))?;
        let json_text = js_to_json(&js_text);
        let raw_data: IndexMap<String, Value> = serde_json::from_str(&json_text)
            .map_err(|e| UsageFetchError::ParseFailed(e.to_string()))?;
        Ok(build_pokemon_list(raw_data))
    }
}

fn fetch_html(url: &str) -> Result<String, reqwest::Error> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("Mozilla/5.0")
        .build()?;
    client.get(url).send()?.text()
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
        .map(|(gamewith_poke_id, p)| PokemonUsageSummary {
            id: gamewith_poke_id,
            name: str_field(&p, "name"),
            types: str_array(&p["types"]),
            moves: parse_moves(&p["moves"]),
            items: parse_items(&p["items"]),
            effort_values: parse_evs(&p["evDistributions"]),
            natures: parse_natures(&p["natures"]),
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
