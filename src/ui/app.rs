use super::pokemon::{PokemonState, PokemonView};
use iced::{
    Element, Length, Subscription, Task,
    futures::SinkExt,
    widget::{button, column, container, row, scrollable, text},
};
use std::sync::{Arc, Mutex, mpsc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Editor,
    SelectionSupport,
}

pub struct PokeEditorApp {
    pokemons: [PokemonState; 6],
    active_tab: Tab,
    ocr_result: Option<String>,
    ocr_receiver: Arc<Mutex<mpsc::Receiver<String>>>,
}

#[derive(Debug, Clone)]
pub enum Message {
    PokemonMsg(usize, super::pokemon::Message),
    TabSelected(Tab),
    OcrResultReceived(String),
}

impl PokeEditorApp {
    pub fn new(ocr_receiver: Arc<Mutex<mpsc::Receiver<String>>>) -> (Self, Task<Message>) {
        (
            Self {
                pokemons: std::array::from_fn(|i| PokemonState::new(format!("poke{}", i + 1))),
                active_tab: Tab::Editor,
                ocr_result: None,
                ocr_receiver,
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
            Message::OcrResultReceived(text) => {
                self.ocr_result = Some(text);
            }
        }
        Task::none()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        // Use a static OnceLock to store the receiver so it can be accessed without capturing in the subscription closure.
        static RECEIVER: std::sync::OnceLock<Arc<Mutex<mpsc::Receiver<String>>>> = std::sync::OnceLock::new();
        let _ = RECEIVER.set(self.ocr_receiver.clone());

        iced::Subscription::run(|| {
            iced::stream::channel(100, |mut output: iced::futures::channel::mpsc::Sender<Message>| {
                let receiver = RECEIVER.get().unwrap().clone();
                async move {
                    loop {
                        let receiver = receiver.clone();
                        let result = tokio::task::spawn_blocking(move || {
                            receiver.lock().unwrap().recv().ok()
                        })
                        .await;

                        if let Ok(Some(text)) = result {
                            let _ = output.send(Message::OcrResultReceived(text)).await;
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
        let result_widget = match &self.ocr_result {
            None => text("OCR結果待機中...").size(20),
            Some(result) => {
                let has_single = result.contains("シングル");
                let has_battle = result.contains("バトル");
                let label = if has_single && has_battle { "OK" } else { "NG" };
                text(format!("{}: {}", label, result)).size(20)
            }
        };

        container(column![text("選出サポート").size(32), result_widget].spacing(20))
            .padding(20)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}
