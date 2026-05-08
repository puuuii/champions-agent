use super::components::VideoPreview;
use super::pokemon::{FieldType, PokemonState, PokemonView, SuggestionRequest};
use super::subscriptions::{self, RuntimeMessage};
use champions_application::ports::{CatalogRepository, PartyRepository};
use champions_application::use_cases::{
    LoadPartyUseCase, SavePartyCommand, SavePartyUseCase, SuggestKind, SuggestNamesQuery,
    SuggestNamesUseCase,
};
use champions_domain::party::SavedParty;
use champions_domain::usage::PokemonUsageSummary;
use champions_interface::{PreviewFrame, RuntimeEvent};
use champions_runtime::CommandSender;
use iced::futures::SinkExt;
use iced::{
    Element, Length, Subscription, Task, Theme,
    widget::{button, column, container, row, scrollable, text},
};
use std::sync::{Arc, Mutex, mpsc};

use super::JAPANESE_FONT;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Editor,
    SelectionSupport,
}

pub struct PokeEditorApp {
    pokemons: [PokemonState; 6],
    active_tab: Tab,
    opponent_party: Option<Vec<PokemonUsageSummary>>,
    info_receiver: Arc<Mutex<mpsc::Receiver<Vec<PokemonUsageSummary>>>>,
    catalog_repo: Arc<dyn CatalogRepository>,
    party_repo: Arc<dyn PartyRepository>,
    #[allow(dead_code)]
    command_sender: Arc<CommandSender>,
    latest_preview: Option<PreviewFrame>,
}

#[derive(Debug, Clone)]
pub enum Message {
    PokemonMsg(usize, super::pokemon::Message),
    TabSelected(Tab),
    PartyInfoReceived(Vec<PokemonUsageSummary>),
    Save,
    RuntimeMsg(RuntimeMessage),
}

impl PokeEditorApp {
    pub fn new(
        info_receiver: Arc<Mutex<mpsc::Receiver<Vec<PokemonUsageSummary>>>>,
        catalog_repo: Arc<dyn CatalogRepository>,
        party_repo: Arc<dyn PartyRepository>,
        command_sender: Arc<CommandSender>,
    ) -> (Self, Task<Message>) {
        let load_uc = LoadPartyUseCase::new(party_repo.as_ref());
        let pokemons = match load_uc.execute() {
            Ok(result) if !result.party.pokemons.is_empty() => {
                let mut pokes =
                    std::array::from_fn(|i| PokemonState::new(format!("poke{}", i + 1)));
                for (i, build) in result.party.pokemons.into_iter().enumerate().take(6) {
                    pokes[i] = PokemonState::from_saved_build(format!("poke{}", i + 1), build);
                }
                pokes
            }
            _ => std::array::from_fn(|i| PokemonState::new(format!("poke{}", i + 1))),
        };

        (
            Self {
                pokemons,
                active_tab: Tab::Editor,
                opponent_party: None,
                info_receiver,
                catalog_repo,
                party_repo,
                command_sender,
                latest_preview: None,
            },
            Task::none(),
        )
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::PokemonMsg(index, msg) => {
                let request = self.pokemons[index].update(msg);
                if let Some(req) = request {
                    self.handle_suggestion_request(index, req);
                }
            }
            Message::TabSelected(tab) => {
                self.active_tab = tab;
            }
            Message::PartyInfoReceived(party) => {
                self.opponent_party = Some(party);
            }
            Message::Save => {
                let saved_party = SavedParty {
                    pokemons: self.pokemons.iter().map(|p| p.to_build()).collect(),
                };
                let save_uc = SavePartyUseCase::new(self.party_repo.as_ref());
                if let Err(e) = save_uc.execute(SavePartyCommand { party: saved_party }) {
                    eprintln!("Failed to save party: {e}");
                }
            }
            Message::RuntimeMsg(runtime_msg) => match runtime_msg {
                RuntimeMessage::PreviewFrameReceived(frame) => {
                    self.latest_preview = Some(frame);
                }
                RuntimeMessage::RuntimeEventReceived(event) => {
                    self.handle_runtime_event(event);
                }
            },
        }
        Task::none()
    }

    fn handle_runtime_event(&mut self, event: RuntimeEvent) {
        match event {
            RuntimeEvent::RuntimeStopped { .. } => {
                println!("[Runtime] stopped");
            }
            RuntimeEvent::Error { error, .. } => {
                eprintln!("[Runtime] error: {:?}", error);
            }
            _ => {}
        }
    }

    fn handle_suggestion_request(&mut self, index: usize, req: SuggestionRequest) {
        if req.query.is_empty() {
            self.pokemons[index].set_suggestions(Vec::new());
            return;
        }

        let kind = match req.field {
            FieldType::Species => SuggestKind::Species,
            FieldType::Move(_) => SuggestKind::Move,
            FieldType::Item => SuggestKind::Item,
            FieldType::Nature => SuggestKind::Nature,
            FieldType::Ability => SuggestKind::Ability,
        };

        let suggest_uc = SuggestNamesUseCase::new(self.catalog_repo.as_ref());
        let query = SuggestNamesQuery {
            kind,
            query: req.query,
            limit: 5,
        };

        match suggest_uc.execute(query) {
            Ok(result) => {
                self.pokemons[index].set_suggestions(result.suggestions);
            }
            Err(_) => {
                self.pokemons[index].set_suggestions(Vec::new());
            }
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let preview_sub = subscriptions::preview_subscription().map(Message::RuntimeMsg);
        let event_sub = subscriptions::event_subscription().map(Message::RuntimeMsg);

        let party_info_sub = {
            static RECEIVER: std::sync::OnceLock<
                Arc<Mutex<mpsc::Receiver<Vec<PokemonUsageSummary>>>>,
            > = std::sync::OnceLock::new();
            let _ = RECEIVER.set(self.info_receiver.clone());

            iced::Subscription::run(|| {
                iced::stream::channel::<Message>(
                    100,
                    |mut output: iced::futures::channel::mpsc::Sender<Message>| {
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
                    },
                )
            })
        };

        Subscription::batch([preview_sub, event_sub, party_info_sub])
    }

    pub fn view(&self) -> Element<'_, Message> {
        let tab_bar = row![
            button(text("パーティ編集").font(JAPANESE_FONT))
                .on_press(Message::TabSelected(Tab::Editor))
                .padding(10),
            button(text("選出サポート").font(JAPANESE_FONT))
                .on_press(Message::TabSelected(Tab::SelectionSupport))
                .padding(10),
        ]
        .spacing(10);

        let right_content = match self.active_tab {
            Tab::Editor => self.editor_view(),
            Tab::SelectionSupport => self.selection_support_view(),
        };

        let right_panel = column![tab_bar, right_content].spacing(20);

        let left_panel = self.preview_view();

        let layout = row![
            container(left_panel)
                .width(Length::FillPortion(1))
                .height(Length::Fill),
            container(right_panel)
                .width(Length::FillPortion(2))
                .height(Length::Fill),
        ]
        .spacing(20);

        container(layout)
            .padding(20)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn preview_view(&self) -> Element<'_, Message> {
        container(
            column![
                text("カメラプレビュー").font(JAPANESE_FONT).size(32),
                VideoPreview::view(self.latest_preview.as_ref()),
            ]
            .spacing(20),
        )
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

        let header = row![
            text("Party Editor").size(32),
            button(text("保存").font(JAPANESE_FONT))
                .on_press(Message::Save)
                .padding(10),
        ]
        .spacing(20)
        .align_y(iced::Alignment::Center);

        scrollable(column![header, grid].spacing(20)).into()
    }

    fn selection_support_view(&self) -> Element<'_, Message> {
        let content: Element<'_, Message> = match &self.opponent_party {
            None => text::<Theme, iced::Renderer>("ポケモン選出画面を待機中...")
                .size(20)
                .font(JAPANESE_FONT)
                .into(),
            Some(party) => {
                let mut name_row = row![
                    container(text("名前").font(JAPANESE_FONT).size(16)).width(Length::Fixed(80.0))
                ]
                .spacing(10);
                let mut type_row = row![
                    container(text("タイプ").font(JAPANESE_FONT).size(16))
                        .width(Length::Fixed(80.0))
                ]
                .spacing(10);
                let mut move_row = row![
                    container(text("技").font(JAPANESE_FONT).size(16)).width(Length::Fixed(80.0))
                ]
                .spacing(10);
                let mut item_row = row![
                    container(text("持ち物").font(JAPANESE_FONT).size(16))
                        .width(Length::Fixed(80.0))
                ]
                .spacing(10);
                let mut ev_row = row![
                    container(text("努力値").font(JAPANESE_FONT).size(16))
                        .width(Length::Fixed(80.0))
                ]
                .spacing(10);
                let mut nature_row = row![
                    container(text("性格").font(JAPANESE_FONT).size(16)).width(Length::Fixed(80.0))
                ]
                .spacing(10);

                for p in party {
                    name_row = name_row.push(
                        container(text(&p.name).font(JAPANESE_FONT).size(20))
                            .width(Length::FillPortion(1)),
                    );
                    type_row = type_row.push(
                        container(text(p.types.join(", ")).font(JAPANESE_FONT).size(14))
                            .width(Length::FillPortion(1)),
                    );
                    let mut moves_col = column![].spacing(2);
                    for m in p.moves.iter().take(8) {
                        moves_col = moves_col.push(
                            text(format!("{} ({})", m.name, m.rate))
                                .font(JAPANESE_FONT)
                                .size(12),
                        );
                    }
                    move_row = move_row.push(container(moves_col).width(Length::FillPortion(1)));
                    let mut items_col = column![].spacing(2);
                    for i in p.items.iter().take(3) {
                        items_col = items_col.push(
                            text(format!("{} ({})", i.name, i.rate))
                                .font(JAPANESE_FONT)
                                .size(12),
                        );
                    }
                    item_row = item_row.push(container(items_col).width(Length::FillPortion(1)));
                    let mut evs_col = column![].spacing(2);
                    for e in p.effort_values.iter().take(3) {
                        evs_col = evs_col.push(
                            text(format!(
                                "H{} A{} B{} C{} D{} S{}\n({})",
                                e.h, e.a, e.b, e.c, e.d, e.s, e.rate
                            ))
                            .font(JAPANESE_FONT)
                            .size(12),
                        );
                    }
                    ev_row = ev_row.push(container(evs_col).width(Length::FillPortion(1)));
                    let mut natures_col = column![].spacing(2);
                    for n in p.natures.iter().take(2) {
                        natures_col = natures_col.push(
                            text(format!("{} ({})", n.name, n.rate))
                                .font(JAPANESE_FONT)
                                .size(12),
                        );
                    }
                    nature_row =
                        nature_row.push(container(natures_col).width(Length::FillPortion(1)));
                }

                let table =
                    column![name_row, type_row, move_row, item_row, ev_row, nature_row].spacing(20);
                scrollable(table).into()
            }
        };

        container(column![text("選出サポート").font(JAPANESE_FONT).size(32), content].spacing(20))
            .padding(20)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}
