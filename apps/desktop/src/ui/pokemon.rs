use iced::{
    Element,
    widget::{column, container, row, text, text_input, button},
    Border, Color,
};
use std::sync::Arc;
use crate::domain::master_data::MasterData;
use super::JAPANESE_FONT;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    Species,
    Move(usize),
    Nature,
    Item,
    Ability,
}

#[derive(Debug, Clone)]
pub struct PokemonState {
    pub label: String,
    pub species: String,
    pub item: String,
    pub h: String,
    pub a: String,
    pub b: String,
    pub c: String,
    pub d: String,
    pub s: String,
    pub nature: String,
    pub ability: String,
    pub moves: [String; 4],
    
    // サジェスト用
    pub suggestions: Vec<String>,
    pub active_field: Option<FieldType>,
    master_data: Arc<MasterData>,
}

#[derive(Debug, Clone)]
pub enum Message {
    SpeciesChanged(String),
    ItemChanged(String),
    HChanged(String),
    AChanged(String),
    BChanged(String),
    CChanged(String),
    DChanged(String),
    SChanged(String),
    NatureChanged(String),
    AbilityChanged(String),
    MoveChanged(usize, String),
    SuggestionSelected(String),
}

use crate::domain::party::SavedPokemon;

impl PokemonState {
    pub fn new(label: String, master_data: Arc<MasterData>) -> Self {
        Self {
            label,
            species: String::new(),
            item: String::new(),
            h: String::new(),
            a: String::new(),
            b: String::new(),
            c: String::new(),
            d: String::new(),
            s: String::new(),
            nature: String::new(),
            ability: String::new(),
            moves: Default::default(),
            suggestions: Vec::new(),
            active_field: None,
            master_data,
        }
    }

    pub fn from_saved(label: String, saved: SavedPokemon, master_data: Arc<MasterData>) -> Self {
        Self {
            label,
            species: saved.species,
            item: saved.item,
            h: saved.h,
            a: saved.a,
            b: saved.b,
            c: saved.c,
            d: saved.d,
            s: saved.s,
            nature: saved.nature,
            ability: saved.ability,
            moves: saved.moves,
            suggestions: Vec::new(),
            active_field: None,
            master_data,
        }
    }

    pub fn to_saved(&self) -> SavedPokemon {
        SavedPokemon {
            species: self.species.clone(),
            item: self.item.clone(),
            h: self.h.clone(),
            a: self.a.clone(),
            b: self.b.clone(),
            c: self.c.clone(),
            d: self.d.clone(),
            s: self.s.clone(),
            nature: self.nature.clone(),
            ability: self.ability.clone(),
            moves: self.moves.clone(),
        }
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::SpeciesChanged(v) => {
                self.species = v;
                self.update_suggestions(FieldType::Species, &self.species.clone());
            }
            Message::ItemChanged(v) => {
                self.item = v;
                self.update_suggestions(FieldType::Item, &self.item.clone());
            }
            Message::HChanged(v) => self.h = self.validate_stat(v),
            Message::AChanged(v) => self.a = self.validate_stat(v),
            Message::BChanged(v) => self.b = self.validate_stat(v),
            Message::CChanged(v) => self.c = self.validate_stat(v),
            Message::DChanged(v) => self.d = self.validate_stat(v),
            Message::SChanged(v) => self.s = self.validate_stat(v),
            Message::NatureChanged(v) => {
                self.nature = v;
                self.update_suggestions(FieldType::Nature, &self.nature.clone());
            }
            Message::AbilityChanged(v) => {
                self.ability = v;
                self.update_suggestions(FieldType::Ability, &self.ability.clone());
            }
            Message::MoveChanged(i, v) => {
                self.moves[i] = v;
                self.update_suggestions(FieldType::Move(i), &self.moves[i].clone());
            }
            Message::SuggestionSelected(v) => {
                if let Some(field) = self.active_field {
                    match field {
                        FieldType::Species => self.species = v,
                        FieldType::Item => self.item = v,
                        FieldType::Nature => self.nature = v,
                        FieldType::Move(i) => self.moves[i] = v,
                        FieldType::Ability => self.ability = v,
                    }
                }
                self.suggestions.clear();
                self.active_field = None;
            }
        }
    }

    fn validate_stat(&self, input: String) -> String {
        let filtered: String = input.chars().filter(|c| c.is_digit(10)).collect();
        if filtered.len() > 3 {
            filtered[..3].to_string()
        } else {
            filtered
        }
    }

    fn update_suggestions(&mut self, field: FieldType, query: &str) {
        self.active_field = Some(field);
        if query.is_empty() {
            self.suggestions.clear();
            return;
        }

        let source = match field {
            FieldType::Species => &self.master_data.pokemon,
            FieldType::Move(_) => &self.master_data.moves,
            FieldType::Nature => &self.master_data.natures,
            FieldType::Item => &self.master_data.items,
            FieldType::Ability => &self.master_data.abilities,
        };

        self.suggestions = source
            .iter()
            .filter(|name: &&String| name.starts_with(query))
            .take(5)
            .cloned()
            .collect();
    }
}

pub struct PokemonView;

impl PokemonView {
    pub fn view(state: &PokemonState) -> Element<'_, Message> {
        let stat_input = |label: &'static str, value: &str, on_change: fn(String) -> Message| {
            row![
                text(label).width(20),
                text_input("数値", value)
                    .on_input(on_change)
                    .font(JAPANESE_FONT)
                    .width(60)
            ]
            .spacing(5)
        };

        let species_field = Self::field_with_suggestions(
            "名前:", &state.species, FieldType::Species, &state.suggestions, state.active_field, Message::SpeciesChanged
        );

        let item_field = Self::field_with_suggestions(
            "持物:", &state.item, FieldType::Item, &state.suggestions, state.active_field, Message::ItemChanged
        );

        let nature_field = Self::field_with_suggestions(
            "性格:", &state.nature, FieldType::Nature, &state.suggestions, state.active_field, Message::NatureChanged
        );

        let stats_col1 = column![
            stat_input("H:", &state.h, Message::HChanged),
            stat_input("A:", &state.a, Message::AChanged),
            stat_input("C:", &state.c, Message::CChanged),
            nature_field,
        ]
        .spacing(10);

        let ability_field = Self::field_with_suggestions(
            "特性:", &state.ability, FieldType::Ability, &state.suggestions, state.active_field, Message::AbilityChanged
        );

        let stats_col2 = column![
            stat_input("S:", &state.s, Message::SChanged),
            stat_input("B:", &state.b, Message::BChanged),
            stat_input("D:", &state.d, Message::DChanged),
            ability_field,
        ]
        .spacing(10);

        let moves_col = column![
            Self::field_with_suggestions("技1:", &state.moves[0], FieldType::Move(0), &state.suggestions, state.active_field, |v| Message::MoveChanged(0, v)),
            Self::field_with_suggestions("技2:", &state.moves[1], FieldType::Move(1), &state.suggestions, state.active_field, |v| Message::MoveChanged(1, v)),
            Self::field_with_suggestions("技3:", &state.moves[2], FieldType::Move(2), &state.suggestions, state.active_field, |v| Message::MoveChanged(2, v)),
            Self::field_with_suggestions("技4:", &state.moves[3], FieldType::Move(3), &state.suggestions, state.active_field, |v| Message::MoveChanged(3, v)),
        ]
        .spacing(10);

        container(
            column![
                text(&state.label).size(18).color(Color::from_rgb(0.3, 0.3, 0.3)),
                species_field,
                item_field,
                row![stats_col1, stats_col2, moves_col].spacing(20)
            ]
            .spacing(5),
        )
        .padding(15)
        .style(|_| container::Style {
            border: Border {
                color: Color::from_rgb(0.8, 0.8, 0.8),
                width: 1.0,
                radius: 5.0.into(),
            },
            ..Default::default()
        })
        .into()
    }

    fn field_with_suggestions<'a>(
        label: &'static str,
        value: &'a str,
        field_type: FieldType,
        suggestions: &'a [String],
        active_field: Option<FieldType>,
        on_change: impl Fn(String) -> Message + 'static,
    ) -> Element<'a, Message> {
        let mut col = column![
            row![
                text(label).width(40),
                text_input(label, value)
                    .on_input(on_change)
                    .font(JAPANESE_FONT)
                    .width(120)
            ].spacing(5)
        ];

        if active_field == Some(field_type) && !suggestions.is_empty() {
            let suggestion_list = column(
                suggestions.iter().map(|s| {
                    button(text(s).size(14))
                        .on_press(Message::SuggestionSelected(s.clone()))
                        .style(button::secondary)
                        .width(120)
                        .into()
                })
            ).spacing(1);

            col = col.push(
                container(suggestion_list)
                    .padding(2)
                    .style(|_| container::Style {
                        border: Border {
                            color: Color::from_rgb(0.7, 0.7, 0.7),
                            width: 1.0,
                            radius: 2.0.into(),
                        },
                        ..Default::default()
                    })
            );
        }
        col.into()
    }
}
