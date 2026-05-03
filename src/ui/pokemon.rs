use iced::{
    Element,
    widget::{column, container, row, text, text_input},
};

#[derive(Debug, Clone)]
pub struct PokemonState {
    pub name: String,
    pub h: String,
    pub a: String,
    pub b: String,
    pub c: String,
    pub d: String,
    pub s: String,
    pub nature: String,
    pub ability: String,
    pub moves: [String; 4],
}

#[derive(Debug, Clone)]
pub enum Message {
    HChanged(String),
    AChanged(String),
    BChanged(String),
    CChanged(String),
    DChanged(String),
    SChanged(String),
    NatureChanged(String),
    AbilityChanged(String),
    MoveChanged(usize, String),
}

impl PokemonState {
    pub fn new(name: String) -> Self {
        Self {
            name,
            h: String::new(),
            a: String::new(),
            b: String::new(),
            c: String::new(),
            d: String::new(),
            s: String::new(),
            nature: String::new(),
            ability: String::new(),
            moves: Default::default(),
        }
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::HChanged(v) => self.h = v,
            Message::AChanged(v) => self.a = v,
            Message::BChanged(v) => self.b = v,
            Message::CChanged(v) => self.c = v,
            Message::DChanged(v) => self.d = v,
            Message::SChanged(v) => self.s = v,
            Message::NatureChanged(v) => self.nature = v,
            Message::AbilityChanged(v) => self.ability = v,
            Message::MoveChanged(i, v) => self.moves[i] = v,
        }
    }
}

pub struct PokemonView;

impl PokemonView {
    pub fn view(state: &PokemonState) -> Element<'_, Message> {
        // label を &'static str にすることで、戻り値の Element に安全に含められるようになります
        let stat_input = |label: &'static str, value: &str, on_change: fn(String) -> Message| {
            row![
                text(label).width(20),
                text_input("数値", value).on_input(on_change).width(100)
            ]
            .spacing(5)
        };

        let stats_col1 = column![
            stat_input("H:", &state.h, Message::HChanged),
            stat_input("A:", &state.a, Message::AChanged),
            stat_input("C:", &state.c, Message::CChanged),
            row![
                text("性格:").width(40),
                text_input("性格", &state.nature)
                    .on_input(Message::NatureChanged)
                    .width(80)
            ]
            .spacing(5)
        ]
        .spacing(10);

        let stats_col2 = column![
            stat_input("S:", &state.s, Message::SChanged),
            stat_input("B:", &state.b, Message::BChanged),
            stat_input("D:", &state.d, Message::DChanged),
            row![
                text("特性:").width(40),
                text_input("特性", &state.ability)
                    .on_input(Message::AbilityChanged)
                    .width(80)
            ]
            .spacing(5)
        ]
        .spacing(10);

        let moves_col = column![
            text_input("move1", &state.moves[0])
                .on_input(|v| Message::MoveChanged(0, v))
                .width(120),
            text_input("move2", &state.moves[1])
                .on_input(|v| Message::MoveChanged(1, v))
                .width(120),
            text_input("move3", &state.moves[2])
                .on_input(|v| Message::MoveChanged(2, v))
                .width(120),
            text_input("move4", &state.moves[3])
                .on_input(|v| Message::MoveChanged(3, v))
                .width(120),
        ]
        .spacing(10);

        container(
            column![
                text(&state.name).size(20),
                row![stats_col1, stats_col2, moves_col].spacing(20)
            ]
            .spacing(10),
        )
        .padding(15)
        .into()
    }
}
