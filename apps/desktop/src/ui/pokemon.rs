use super::JAPANESE_FONT;
use champions_domain::party::{EffortValueSpread, MoveSet, PokemonBuild};
use iced::{
    Border, Color, Element,
    widget::{Id, button, column, container, row, text, text_input},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    Species,
    Move(usize),
    Item,
    Ability,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputScope {
    Editor(usize),
    Restore(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputField {
    Species,
    Item,
    H,
    A,
    B,
    C,
    D,
    S,
    Ability,
    Move(usize),
}

#[derive(Debug, Clone)]
pub struct SuggestionRequest {
    pub field: FieldType,
    pub query: String,
}

#[derive(Debug, Clone)]
pub struct PokemonState {
    pub species: String,
    pub item: String,
    pub h: String,
    pub a: String,
    pub b: String,
    pub c: String,
    pub d: String,
    pub s: String,
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
    AbilityChanged(String),
    MoveChanged(usize, String),
    SuggestionSelected(String),
    SaveRequested,
    RestoreRequested,
}

impl PokemonState {
    pub fn new() -> Self {
        Self {
            species: String::new(),
            item: String::new(),
            h: String::new(),
            a: String::new(),
            b: String::new(),
            c: String::new(),
            d: String::new(),
            s: String::new(),
            ability: String::new(),
            moves: Default::default(),
            suggestions: Vec::new(),
            active_field: None,
        }
    }

    pub fn from_saved_build(build: PokemonBuild) -> Self {
        Self {
            species: build.species_name,
            item: build.item_name.unwrap_or_default(),
            h: build.effort_values.h.to_string(),
            a: build.effort_values.a.to_string(),
            b: build.effort_values.b.to_string(),
            c: build.effort_values.c.to_string(),
            d: build.effort_values.d.to_string(),
            s: build.effort_values.s.to_string(),
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
            nature_name: None,
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
                        FieldType::Move(i) => self.moves[i] = v,
                        FieldType::Ability => self.ability = v,
                    }
                }
                self.suggestions.clear();
                self.active_field = None;
            }
            Message::SaveRequested | Message::RestoreRequested => {}
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
    pub fn view<'a>(
        slot_index: usize,
        state: &'a PokemonState,
        can_restore_from_library: bool,
        save_status: Option<&'a str>,
    ) -> Element<'a, Message> {
        let save_button = button(text("保存").font(JAPANESE_FONT))
            .on_press(Message::SaveRequested)
            .padding([6, 10]);

        let restore_button = button(text("一覧").font(JAPANESE_FONT)).padding([6, 10]);
        let restore_button = if can_restore_from_library {
            restore_button.on_press(Message::RestoreRequested)
        } else {
            restore_button
        };

        let mut action_row = row![save_button, restore_button]
            .spacing(10)
            .align_y(iced::Alignment::Center);

        if let Some(save_status) = save_status {
            action_row = action_row.push(
                text(save_status)
                    .font(JAPANESE_FONT)
                    .size(14)
                    .color(Color::from_rgb(0.7, 0.15, 0.15)),
            );
        }

        container(
            column![
                action_row,
                Self::form(state, InputScope::Editor(slot_index))
            ]
            .spacing(10),
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

    pub fn restore_form(history_index: usize, state: &PokemonState) -> Element<'_, Message> {
        Self::form(state, InputScope::Restore(history_index))
    }

    fn form(state: &PokemonState, scope: InputScope) -> Element<'_, Message> {
        let stat_input = |label: &'static str,
                          value: &str,
                          on_change: fn(String) -> Message,
                          field: InputField| {
            row![
                text(label).width(20),
                text_input("数値", value)
                    .id(input_id(scope, field))
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
            input_id(scope, InputField::Species),
            &state.suggestions,
            state.active_field,
            Message::SpeciesChanged,
        );

        let item_field = Self::field_with_suggestions(
            "持物:",
            &state.item,
            FieldType::Item,
            input_id(scope, InputField::Item),
            &state.suggestions,
            state.active_field,
            Message::ItemChanged,
        );

        let stats_col1 = column![
            stat_input("H:", &state.h, Message::HChanged, InputField::H),
            stat_input("A:", &state.a, Message::AChanged, InputField::A),
            stat_input("C:", &state.c, Message::CChanged, InputField::C),
        ]
        .spacing(10);

        let ability_field = Self::field_with_suggestions(
            "特性:",
            &state.ability,
            FieldType::Ability,
            input_id(scope, InputField::Ability),
            &state.suggestions,
            state.active_field,
            Message::AbilityChanged,
        );

        let stats_col2 = column![
            stat_input("S:", &state.s, Message::SChanged, InputField::S),
            stat_input("B:", &state.b, Message::BChanged, InputField::B),
            stat_input("D:", &state.d, Message::DChanged, InputField::D),
            ability_field,
        ]
        .spacing(10);

        let moves_col = column![
            Self::field_with_suggestions(
                "技1:",
                &state.moves[0],
                FieldType::Move(0),
                input_id(scope, InputField::Move(0)),
                &state.suggestions,
                state.active_field,
                |v| Message::MoveChanged(0, v)
            ),
            Self::field_with_suggestions(
                "技2:",
                &state.moves[1],
                FieldType::Move(1),
                input_id(scope, InputField::Move(1)),
                &state.suggestions,
                state.active_field,
                |v| Message::MoveChanged(1, v)
            ),
            Self::field_with_suggestions(
                "技3:",
                &state.moves[2],
                FieldType::Move(2),
                input_id(scope, InputField::Move(2)),
                &state.suggestions,
                state.active_field,
                |v| Message::MoveChanged(2, v)
            ),
            Self::field_with_suggestions(
                "技4:",
                &state.moves[3],
                FieldType::Move(3),
                input_id(scope, InputField::Move(3)),
                &state.suggestions,
                state.active_field,
                |v| Message::MoveChanged(3, v)
            ),
        ]
        .spacing(10);

        column![
            species_field,
            item_field,
            row![stats_col1, stats_col2, moves_col].spacing(20)
        ]
        .spacing(5)
        .into()
    }

    fn field_with_suggestions<'a>(
        label: &'static str,
        value: &'a str,
        field_type: FieldType,
        id: Id,
        suggestions: &'a [String],
        active_field: Option<FieldType>,
        on_change: impl Fn(String) -> Message + 'static,
    ) -> Element<'a, Message> {
        let mut col = column![
            row![
                text(label).width(40),
                text_input(label, value)
                    .id(id)
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

pub fn editor_input_ids(slot_index: usize) -> Vec<Id> {
    ordered_input_fields()
        .into_iter()
        .map(|field| input_id(InputScope::Editor(slot_index), field))
        .collect()
}

pub fn restore_input_ids(history_index: usize) -> Vec<Id> {
    ordered_input_fields()
        .into_iter()
        .map(|field| input_id(InputScope::Restore(history_index), field))
        .collect()
}

fn ordered_input_fields() -> [InputField; 13] {
    [
        InputField::Species,
        InputField::Item,
        InputField::H,
        InputField::S,
        InputField::Move(0),
        InputField::A,
        InputField::B,
        InputField::Move(1),
        InputField::C,
        InputField::D,
        InputField::Move(2),
        InputField::Ability,
        InputField::Move(3),
    ]
}

fn input_id(scope: InputScope, field: InputField) -> Id {
    let scope_prefix = match scope {
        InputScope::Editor(slot_index) => format!("party-editor-{slot_index}"),
        InputScope::Restore(history_index) => format!("restore-editor-{history_index}"),
    };

    format!("{scope_prefix}-{}", field.id_suffix()).into()
}

impl InputField {
    fn id_suffix(self) -> &'static str {
        match self {
            Self::Species => "species",
            Self::Item => "item",
            Self::H => "h",
            Self::A => "a",
            Self::B => "b",
            Self::C => "c",
            Self::D => "d",
            Self::S => "s",
            Self::Ability => "ability",
            Self::Move(0) => "move-0",
            Self::Move(1) => "move-1",
            Self::Move(2) => "move-2",
            Self::Move(3) => "move-3",
            Self::Move(_) => unreachable!("move index is limited to four slots"),
        }
    }
}
