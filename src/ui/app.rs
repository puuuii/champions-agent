use super::pokemon::{PokemonState, PokemonView};
use iced::{
    Element, Length, Subscription, Task, Theme,
    widget::{button, column, container, row, scrollable, text},
};
use iced::futures::SinkExt;
use serde::Deserialize;
use std::sync::{Arc, Mutex, mpsc};

// --- JSONから読み込むための構造体定義 ---
#[derive(Debug, Deserialize, Clone)]
pub struct MoveInfo {
    pub name: String,
    pub rate: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ItemInfo {
    pub name: String,
    pub rate: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct EvInfo {
    pub h: u32,
    pub a: u32,
    pub b: u32,
    pub c: u32,
    pub d: u32,
    pub s: u32,
    pub rate: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct NatureInfo {
    pub name: String,
    pub rate: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PokemonUsage {
    pub name: String,
    pub types: Vec<String>,
    pub moves: Vec<MoveInfo>,
    pub items: Vec<ItemInfo>,
    pub effort_values: Vec<EvInfo>,
    pub natures: Vec<NatureInfo>,
}
// ----------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Editor,
    SelectionSupport,
}

pub struct PokeEditorApp {
    pokemons: [PokemonState; 6],
    active_tab: Tab,
    opponent_party: Option<Vec<PokemonUsage>>, // OCRのテキストではなくポケモン情報を保持
    info_receiver: Arc<Mutex<mpsc::Receiver<Vec<PokemonUsage>>>>,
}

#[derive(Debug, Clone)]
pub enum Message {
    PokemonMsg(usize, super::pokemon::Message),
    TabSelected(Tab),
    PartyInfoReceived(Vec<PokemonUsage>), // 受け取るメッセージを変更
}

impl PokeEditorApp {
    pub fn new(
        info_receiver: Arc<Mutex<mpsc::Receiver<Vec<PokemonUsage>>>>,
    ) -> (Self, Task<Message>) {
        (
            Self {
                pokemons: std::array::from_fn(|i| PokemonState::new(format!("poke{}", i + 1))),
                active_tab: Tab::Editor,
                opponent_party: None,
                info_receiver,
            },
            Task::none(),
        )
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::PokemonMsg(index, msg) => {
                self.pokemons[index].update(msg);
            }
            Message::TabSelected(tab) => {
                self.active_tab = tab;
            }
            Message::PartyInfoReceived(party) => {
                self.opponent_party = Some(party);
            }
        }
        Task::none()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        static RECEIVER: std::sync::OnceLock<Arc<Mutex<mpsc::Receiver<Vec<PokemonUsage>>>>> =
            std::sync::OnceLock::new();
        let _ = RECEIVER.set(self.info_receiver.clone());

        iced::Subscription::run(|| {
            iced::stream::channel::<Message>(100, |mut output: iced::futures::channel::mpsc::Sender<Message>| {
                let receiver = RECEIVER.get().unwrap().clone();
                async move {
                    loop {
                        let receiver = receiver.clone();
                        let result = tokio::task::spawn_blocking(move || {
                            receiver.lock().unwrap().recv().ok()
                        })
                        .await;

                        if let Ok(Some(party)) = result {
                            let _ = output.send(Message::PartyInfoReceived(party)).await;
                        } else {
                            break;
                        }
                    }
                }
            })
        })
    }

    pub fn view(&self) -> Element<'_, Message> {
        let tab_bar = row![
            button(text("パーティ編集"))
                .on_press(Message::TabSelected(Tab::Editor))
                .padding(10),
            button(text("選出サポート"))
                .on_press(Message::TabSelected(Tab::SelectionSupport))
                .padding(10),
        ]
        .spacing(10);

        let content = match self.active_tab {
            Tab::Editor => self.editor_view(),
            Tab::SelectionSupport => self.selection_support_view(),
        };

        container(column![tab_bar, content].spacing(20).padding(20))
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn editor_view(&self) -> Element<'_, Message> {
        // ... (既存のまま) ...
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

    fn selection_support_view(&self) -> Element<'_, Message> {
        let content: Element<'_, Message> = match &self.opponent_party {
            None => text::<Theme, iced::Renderer>("ポケモン選出画面を待機中...").size(20).into(),
            Some(party) => {
                let party_view = row(party
                    .iter()
                    .map(|p| {
                        let mut col = column![
                            text(&p.name).size(24),
                            text(format!("タイプ: {}", p.types.join(", "))).size(14),
                        ]
                        .spacing(10);

                        // 技 上位8つ
                        col = col.push(text("よく使われる技:").size(16));
                        for m in p.moves.iter().take(8) {
                            col = col.push(text(format!("  {} ({})", m.name, m.rate)).size(12));
                        }

                        // アイテム 上位3つ
                        col = col.push(text("アイテム:").size(16));
                        for i in p.items.iter().take(3) {
                            col = col.push(text(format!("  {} ({})", i.name, i.rate)).size(12));
                        }

                        // 努力値配分 上位3つ
                        col = col.push(text("努力値配分:").size(16));
                        for e in p.effort_values.iter().take(3) {
                            col = col.push(
                                text(format!(
                                    "  H{} A{} B{} C{} D{} S{} ({})",
                                    e.h, e.a, e.b, e.c, e.d, e.s, e.rate
                                ))
                                .size(12),
                            );
                        }

                        // 性格 上位2つ
                        col = col.push(text("性格:").size(16));
                        for n in p.natures.iter().take(2) {
                            col = col.push(text(format!("  {} ({})", n.name, n.rate)).size(12));
                        }

                        col.spacing(5).width(Length::FillPortion(1)).into()
                    })
                    .collect::<Vec<Element<'_, Message>>>())
                .spacing(10);

                scrollable(party_view).into()
            }
        };

        container(column![text("選出サポート").size(32), content].spacing(20))
            .padding(20)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}
