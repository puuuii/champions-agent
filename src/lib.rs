//! # usage_fetcher
//!
//! GameWith のポケモン使用率ページから構造化データを取得するライブラリ。
//!
//! ## 使い方
//! ```no_run
//! use usage_fetcher::fetch_usage;
//!
//! let data = fetch_usage("[https://gamewith.jp/pokemon-champions/555373](https://gamewith.jp/pokemon-champions/555373)").unwrap();
//! println!("{}体取得", data.len());
//! ```

use indexmap::IndexMap;
use regex::Regex;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::LazyLock;

// ─── 正規表現の事前コンパイル (Rust 1.80+ LazyLock) ──────────────────────────

static JS_OBJ_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)const pkchPokemonData\s*=\s*(\{.*?\});").unwrap());
static KEY_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b([A-Za-z_]\w*)\s*:").unwrap());
static TRAILING_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r",\s*([\]}])").unwrap());

// ─── 公開データ型 ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Move {
    pub name: String,
    pub rate: String,
    pub type_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub name: String,
    pub rate: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffortValue {
    pub h: i64,
    pub a: i64,
    pub b: i64,
    pub c: i64,
    pub d: i64,
    pub s: i64,
    pub rate: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Nature {
    pub name: String,
    pub rate: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PokemonData {
    pub name: String,
    pub rank: String,
    pub img_url: String,
    pub types: Vec<String>,
    pub moves: Vec<Move>,
    pub items: Vec<Item>,
    pub effort_values: Vec<EffortValue>,
    pub natures: Vec<Nature>,
}

// ─── 公開 API ─────────────────────────────────────────────────────────────────

pub fn fetch_usage(url: &str) -> anyhow::Result<Vec<PokemonData>> {
    let html = fetch_html(url)?;
    let js_text = extract_js_object(&html)?;
    let json_text = js_to_json(&js_text);
    let raw_data = parse_raw(&json_text)?;
    Ok(build_pokemon_list(raw_data))
}

// ─── 内部実装 ─────────────────────────────────────────────────────────────────

fn fetch_html(url: &str) -> anyhow::Result<String> {
    let client = Client::builder().user_agent("Mozilla/5.0").build()?;
    Ok(client.get(url).send()?.text()?)
}

fn extract_js_object(html: &str) -> anyhow::Result<String> {
    JS_OBJ_RE
        .captures(html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_owned())
        .ok_or_else(|| anyhow::anyhow!("pkchPokemonData が見つかりません"))
}

fn js_to_json(js: &str) -> String {
    let step1 = KEY_RE.replace_all(js, "\"$1\":");
    let step2 = step1.replace('\'', "\"");
    TRAILING_RE.replace_all(&step2, "$1").into_owned()
}

fn parse_raw(json_text: &str) -> anyhow::Result<IndexMap<String, Value>> {
    serde_json::from_str(json_text).map_err(|e| {
        let snippet = &json_text[..json_text.len().min(500)];
        anyhow::anyhow!("JSON パースエラー: {e}\n--- 先頭500文字 ---\n{snippet}")
    })
}

fn build_pokemon_list(raw: IndexMap<String, Value>) -> Vec<PokemonData> {
    raw.into_iter()
        .enumerate()
        .map(|(i, (p_id, p))| {
            let rank = (i + 1).to_string();
            PokemonData {
                name: str_field(&p, "name"),
                rank,
                img_url: format!(
                    "https://img.gamewith.jp/article_tools/pokemon-champions/gacha/{p_id}.png"
                ),
                types: str_array(&p["types"]),
                moves: parse_moves(&p["moves"]),
                items: parse_items(&p["items"]),
                effort_values: parse_evs(&p["evDistributions"]),
                natures: parse_natures(&p["natures"]),
            }
        })
        .collect()
}

// ─── フィールド抽出ヘルパー ───────────────────────────────────────────────────

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

fn parse_moves(v: &Value) -> Vec<Move> {
    v.as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|x| {
                    let a = x.as_array()?;
                    Some(Move {
                        type_name: a.first()?.as_str()?.to_owned(),
                        name: a.get(1)?.as_str()?.to_owned(),
                        rate: a.get(2)?.as_str().unwrap_or("").to_owned(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_items(v: &Value) -> Vec<Item> {
    v.as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|x| {
                    let a = x.as_array()?;
                    Some(Item {
                        name: a.first()?.as_str()?.to_owned(),
                        rate: a.get(1)?.as_str().unwrap_or("").to_owned(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_natures(v: &Value) -> Vec<Nature> {
    v.as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|x| {
                    let a = x.as_array()?;
                    Some(Nature {
                        name: a.first()?.as_str()?.to_owned(),
                        rate: a.get(1)?.as_str().unwrap_or("").to_owned(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_evs(v: &Value) -> Vec<EffortValue> {
    v.as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|x| {
                    let outer = x.as_array()?;
                    let stats = outer.first()?.as_array()?;
                    let rate = outer.get(1)?.as_str().unwrap_or("").to_owned();
                    let n = |i: usize| stats.get(i).and_then(Value::as_i64).unwrap_or(0);
                    Some(EffortValue {
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
