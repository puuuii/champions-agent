use super::JAPANESE_FONT;
use champions_domain::party::{EffortValueSpread, MoveSet, PokemonBuild};
use iced::{
    Border, Color, Element,
    widget::{button, column, container, row, text, text_input},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    Species,
    Move(usize),
    Nature,
    Item,
    Ability,
}

#[derive(Debug, Clone)]
pub struct SuggestionRequest {
    pub field: FieldType,
    pub query: String,
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

    pub suggestions: Vec<String>,
    pub active_field: Option<FieldType>,
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

impl PokemonState {
    pub fn new(label: String) -> Self {
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
        }
    }

    pub fn from_saved_build(label: String, build: PokemonBuild) -> Self {
        Self {
            label,
            species: build.species_name,
            item: build.item_name.unwrap_or_default(),
            h: build.effort_values.h.to_string(),
            a: build.effort_values.a.to_string(),
            b: build.effort_values.b.to_string(),
            c: build.effort_values.c.to_string(),
            d: build.effort_values.d.to_string(),
            s: build.effort_values.s.to_string(),
            nature: build.nature_name.unwrap_or_default(),
            ability: build.ability_name.unwrap_or_default(),
            moves: build.moves.moves,
            suggestions: Vec::new(),
            active_field: None,
        }
    }

    pub fn to_build(&self) -> PokemonBuild {
        PokemonBuild {
            species_name: self.species.clone(),
            item_name: if self.item.is_empty() {
                None
            } else {
                Some(self.item.clone())
            },
            ability_name: if self.ability.is_empty() {
                None
            } else {
                Some(self.ability.clone())
            },
            nature_name: if self.nature.is_empty() {
                None
            } else {
                Some(self.nature.clone())
            },
            effort_values: EffortValueSpread {
                h: self.h.parse().unwrap_or(0),
                a: self.a.parse().unwrap_or(0),
                b: self.b.parse().unwrap_or(0),
                c: self.c.parse().unwrap_or(0),
                d: self.d.parse().unwrap_or(0),
                s: self.s.parse().unwrap_or(0),
            },
            moves: MoveSet {
                moves: self.moves.clone(),
            },
        }
    }

    pub fn set_suggestions(&mut self, suggestions: Vec<String>) {
        self.suggestions = suggestions;
    }

    pub fn update(&mut self, message: Message) -> Option<SuggestionRequest> {
        match message {
            Message::SpeciesChanged(v) => {
                self.species = v;
                self.active_field = Some(FieldType::Species);
                return Some(SuggestionRequest {
                    field: FieldType::Species,
                    query: self.species.clone(),
                });
            }
            Message::ItemChanged(v) => {
                self.item = v;
                self.active_field = Some(FieldType::Item);
                return Some(SuggestionRequest {
                    field: FieldType::Item,
                    query: self.item.clone(),
                });
            }
            Message::HChanged(v) => self.h = self.validate_stat(v),
            Message::AChanged(v) => self.a = self.validate_stat(v),
            Message::BChanged(v) => self.b = self.validate_stat(v),
            Message::CChanged(v) => self.c = self.validate_stat(v),
            Message::DChanged(v) => self.d = self.validate_stat(v),
            Message::SChanged(v) => self.s = self.validate_stat(v),
            Message::NatureChanged(v) => {
                self.nature = v;
                self.active_field = Some(FieldType::Nature);
                return Some(SuggestionRequest {
                    field: FieldType::Nature,
                    query: self.nature.clone(),
                });
            }
            Message::AbilityChanged(v) => {
                self.ability = v;
                self.active_field = Some(FieldType::Ability);
                return Some(SuggestionRequest {
                    field: FieldType::Ability,
                    query: self.ability.clone(),
                });
            }
            Message::MoveChanged(i, v) => {
                self.moves[i] = v;
                self.active_field = Some(FieldType::Move(i));
                return Some(SuggestionRequest {
                    field: FieldType::Move(i),
                    query: self.moves[i].clone(),
                });
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
        None
    }

    fn validate_stat(&self, input: String) -> String {
        let filtered: String = input.chars().filter(|c| c.is_ascii_digit()).collect();
        if filtered.len() > 3 {
            filtered[..3].to_string()
        } else {
            filtered
        }
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
            "名前:",
            &state.species,
            FieldType::Species,
            &state.suggestions,
            state.active_field,
            Message::SpeciesChanged,
        );

        let item_field = Self::field_with_suggestions(
            "持物:",
            &state.item,
            FieldType::Item,
            &state.suggestions,
            state.active_field,
            Message::ItemChanged,
        );

        let nature_field = Self::field_with_suggestions(
            "性格:",
            &state.nature,
            FieldType::Nature,
            &state.suggestions,
            state.active_field,
            Message::NatureChanged,
        );

        let stats_col1 = column![
            stat_input("H:", &state.h, Message::HChanged),
            stat_input("A:", &state.a, Message::AChanged),
            stat_input("C:", &state.c, Message::CChanged),
            nature_field,
        ]
        .spacing(10);

        let ability_field = Self::field_with_suggestions(
            "特性:",
            &state.ability,
            FieldType::Ability,
            &state.suggestions,
            state.active_field,
            Message::AbilityChanged,
        );

        let stats_col2 = column![
            stat_input("S:", &state.s, Message::SChanged),
            stat_input("B:", &state.b, Message::BChanged),
            stat_input("D:", &state.d, Message::DChanged),
            ability_field,
        ]
        .spacing(10);

        let moves_col = column![
            Self::field_with_suggestions(
                "技1:",
                &state.moves[0],
                FieldType::Move(0),
                &state.suggestions,
                state.active_field,
                |v| Message::MoveChanged(0, v)
            ),
            Self::field_with_suggestions(
                "技2:",
                &state.moves[1],
                FieldType::Move(1),
                &state.suggestions,
                state.active_field,
                |v| Message::MoveChanged(1, v)
            ),
            Self::field_with_suggestions(
                "技3:",
                &state.moves[2],
                FieldType::Move(2),
                &state.suggestions,
                state.active_field,
                |v| Message::MoveChanged(2, v)
            ),
            Self::field_with_suggestions(
                "技4:",
                &state.moves[3],
                FieldType::Move(3),
                &state.suggestions,
                state.active_field,
                |v| Message::MoveChanged(3, v)
            ),
        ]
        .spacing(10);

        container(
            column![
                text(&state.label)
                    .size(18)
                    .color(Color::from_rgb(0.3, 0.3, 0.3)),
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
            ]
            .spacing(5)
        ];

        if active_field == Some(field_type) && !suggestions.is_empty() {
            let suggestion_list = column(suggestions.iter().map(|s| {
                button(text(s).size(14))
                    .on_press(Message::SuggestionSelected(s.clone()))
                    .style(button::secondary)
                    .width(120)
                    .into()
            }))
            .spacing(1);

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
                    }),
            );
        }
        col.into()
    }
}
