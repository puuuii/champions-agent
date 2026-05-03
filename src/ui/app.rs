use super::pokemon::{PokemonState, PokemonView};
use iced::{
    Element, Length, Task,
    widget::{button, column, container, row, scrollable, text},
};

// 1. タブの種類を定義
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Editor,
    SelectionSupport,
}

pub struct PokeEditorApp {
    pokemons: [PokemonState; 6], // 6匹分の状態[cite: 2]
    active_tab: Tab,             // 現在表示中のタブ
}

#[derive(Debug, Clone)]
pub enum Message {
    PokemonMsg(usize, super::pokemon::Message), // ポケモン個別の更新メッセージ[cite: 2]
    TabSelected(Tab),                           // タブ切り替えメッセージ
}

impl PokeEditorApp {
    pub fn new() -> (Self, Task<Message>) {
        (
            Self {
                // 既存の初期化[cite: 2]
                pokemons: std::array::from_fn(|i| PokemonState::new(format!("poke{}", i + 1))),
                active_tab: Tab::Editor, // 最初は編集画面
            },
            Task::none(),
        )
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::PokemonMsg(index, msg) => {
                self.pokemons[index].update(msg); // pokemon.rsのupdateを呼ぶ[cite: 1, 2]
            }
            Message::TabSelected(tab) => {
                self.active_tab = tab; // タブを切り替える
            }
        }
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        // タブ切り替え用のボタンバー
        let tab_bar = row![
            button(text("パーティ編集"))
                .on_press(Message::TabSelected(Tab::Editor))
                .padding(10),
            button(text("選出サポート"))
                .on_press(Message::TabSelected(Tab::SelectionSupport))
                .padding(10),
        ]
        .spacing(10);

        // 現在のタブに応じてコンテンツを出し分け
        let content = match self.active_tab {
            Tab::Editor => self.editor_view(),
            Tab::SelectionSupport => self.selection_support_view(),
        };

        container(column![tab_bar, content].spacing(20).padding(20))
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    // --- 内部ヘルパー関数 ---

    // 既存の6匹並べるビュー[cite: 2]
    fn editor_view(&self) -> Element<'_, Message> {
        let grid = row![
            column![
                PokemonView::view(&self.pokemons[0]).map(|m| Message::PokemonMsg(0, m)),
                PokemonView::view(&self.pokemons[2]).map(|m| Message::PokemonMsg(2, m)),
                PokemonView::view(&self.pokemons[4]).map(|m| Message::PokemonMsg(4, m)),
            ]
            .spacing(20),
            column![
                PokemonView::view(&self.pokemons[1]).map(|m| Message::PokemonMsg(1, m)),
                PokemonView::view(&self.pokemons[3]).map(|m| Message::PokemonMsg(3, m)),
                PokemonView::view(&self.pokemons[5]).map(|m| Message::PokemonMsg(5, m)),
            ]
            .spacing(20),
        ]
        .spacing(40);

        scrollable(column![text("Party Editor").size(32), grid].spacing(20)).into()
    }

    // 新規追加する画面
    fn selection_support_view(&self) -> Element<'_, Message> {
        container(text("選出サポート画面（実装待ち）").size(24))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    }
}
