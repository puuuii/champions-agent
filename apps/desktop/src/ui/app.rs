use super::components::VideoPreview;
use super::pokemon::{FieldType, PokemonState, PokemonView, SuggestionRequest};
use super::subscriptions::{self, RuntimeMessage};
use champions_application::ports::{
    CatalogRepository, PartyRepository, UsageFetcher, UsageRepository, UsageSource,
};
use champions_application::use_cases::{
    LoadPartyUseCase, RefreshUsageDataCommand, RefreshUsageDataUseCase, SavePartyCommand,
    SavePartyUseCase, SuggestKind, SuggestNamesQuery, SuggestNamesUseCase,
};
use champions_domain::party::SavedParty;
use champions_interface::{
    ConflictView, OpponentPartyView, PreviewFrame, RecognizedPokemonView, RuntimeEvent,
};
use champions_runtime::CommandSender;
use iced::window;
use iced::{
    Element, Length, Size, Subscription, Task,
    widget::{button, column, container, row, scrollable, text},
};
use std::sync::Arc;

use super::JAPANESE_FONT;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Editor,
    SelectionSupport,
}

pub struct PokeEditorApp {
    pokemons: [PokemonState; 6],
    active_tab: Tab,
    opponent_party: Option<OpponentPartyView>,
    catalog_repo: Arc<dyn CatalogRepository>,
    party_repo: Arc<dyn PartyRepository>,
    #[allow(dead_code)]
    command_sender: Arc<CommandSender>,
    latest_preview: Option<PreviewFrame>,
    preview_window_id: Option<window::Id>,
    main_window_id: Option<window::Id>,

    // --- 新規追加 ---
    usage_fetcher: Arc<dyn UsageFetcher>,
    usage_repo: Arc<dyn UsageRepository>,
    is_refreshing: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
    PokemonMsg(usize, super::pokemon::Message),
    TabSelected(Tab),
    Save,
    RuntimeMsg(RuntimeMessage),
    WindowClosed(window::Id),

    // --- 新規追加 ---
    RefreshUsageData,
    UsageDataRefreshed(Result<usize, String>),
}

impl PokeEditorApp {
    pub fn new(
        catalog_repo: Arc<dyn CatalogRepository>,
        party_repo: Arc<dyn PartyRepository>,
        command_sender: Arc<CommandSender>,
        usage_fetcher: Arc<dyn UsageFetcher>,
        usage_repo: Arc<dyn UsageRepository>,
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

        let (main_id, main_task) = window::open(window::Settings {
            size: Size {
                width: 1200.0,
                height: 800.0,
            },
            ..Default::default()
        });

        let (preview_id, preview_task) = window::open(window::Settings {
            size: Size {
                width: 960.0,
                height: 540.0,
            },
            ..Default::default()
        });

        (
            Self {
                pokemons,
                active_tab: Tab::Editor,
                opponent_party: None,
                catalog_repo,
                party_repo,
                command_sender,
                latest_preview: None,
                preview_window_id: Some(preview_id),
                main_window_id: Some(main_id),
                usage_fetcher,
                usage_repo,
                is_refreshing: false,
            },
            Task::batch([main_task.discard(), preview_task.discard()]),
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
            Message::WindowClosed(id) => {
                if self.preview_window_id == Some(id) {
                    self.preview_window_id = None;
                } else if self.main_window_id == Some(id) {
                    return iced::exit();
                }
            }
            // --- 新規追加: 使用率データの更新 ---
            Message::RefreshUsageData => {
                if self.is_refreshing {
                    return Task::none();
                }
                self.is_refreshing = true;

                let fetcher = self.usage_fetcher.clone();
                let repo = self.usage_repo.clone();

                return Task::perform(
                    async move {
                        let cmd = RefreshUsageDataCommand {
                            source: UsageSource::GameWith,
                        };
                        // ↓ use_case の構築をクロージャ内に移動
                        tokio::task::spawn_blocking(move || {
                            let use_case =
                                RefreshUsageDataUseCase::new(fetcher.as_ref(), repo.as_ref());
                            use_case.execute(cmd)
                        })
                        .await
                        .unwrap()
                    },
                    |result| match result {
                        Ok(res) => Message::UsageDataRefreshed(Ok(res.count)),
                        Err(e) => Message::UsageDataRefreshed(Err(e.to_string())),
                    },
                );
            }
            Message::UsageDataRefreshed(result) => {
                self.is_refreshing = false;
                match result {
                    Ok(count) => println!("使用率データを {} 件更新しました", count),
                    Err(e) => eprintln!("使用率データの更新に失敗しました: {}", e),
                }
            }
        }
        Task::none()
    }

    fn handle_runtime_event(&mut self, event: RuntimeEvent) {
        match event {
            RuntimeEvent::OpponentPartyRecognized { party, .. } => {
                self.opponent_party = Some(party);
                self.active_tab = Tab::SelectionSupport;
            }
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

        let close_sub = window::close_events().map(Message::WindowClosed);

        Subscription::batch([preview_sub, event_sub, close_sub])
    }

    pub fn view(&self, id: window::Id) -> Element<'_, Message> {
        if self.preview_window_id == Some(id) {
            return self.preview_view();
        }

        let tab_bar = row![
            button(text("パーティ編集").font(JAPANESE_FONT))
                .on_press(Message::TabSelected(Tab::Editor))
                .padding(10),
            button(text("選出サポート").font(JAPANESE_FONT))
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

    pub fn title(&self, id: window::Id) -> String {
        if self.preview_window_id == Some(id) {
            "Camera Preview".to_string()
        } else {
            "Pokemon Editor".to_string()
        }
    }

    fn preview_view(&self) -> Element<'_, Message> {
        container(VideoPreview::view(self.latest_preview.as_ref()))
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
        let refresh_btn = button(
            text(if self.is_refreshing {
                "更新中..."
            } else {
                "使用率データを更新"
            })
            .font(JAPANESE_FONT),
        )
        .padding(10);

        let refresh_btn = if self.is_refreshing {
            refresh_btn
        } else {
            refresh_btn.on_press(Message::RefreshUsageData)
        };

        let header_row = row![
            text("選出サポート").font(JAPANESE_FONT).size(32),
            refresh_btn
        ]
        .spacing(20)
        .align_y(iced::Alignment::Center);

        let content: Element<'_, Message> = match &self.opponent_party {
            None => text("ポケモン選出画面を待機中...")
                .size(20)
                .font(JAPANESE_FONT)
                .into(),
            Some(party) if party.pokemons.is_empty() => {
                text("選出画面は検出されましたが、相手パーティをまだ判定できていません。")
                    .size(20)
                    .font(JAPANESE_FONT)
                    .into()
            }
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

                for pokemon in &party.pokemons {
                    let usage = pokemon.usage.as_ref();
                    name_row = name_row.push(
                        container(text(format_slot_name(pokemon)).font(JAPANESE_FONT).size(16))
                            .width(Length::FillPortion(1)),
                    );
                    type_row = type_row.push(
                        container(
                            text(
                                usage
                                    .map(|usage| {
                                        if usage.types.is_empty() {
                                            "-".to_string()
                                        } else {
                                            usage.types.join(", ")
                                        }
                                    })
                                    .unwrap_or_else(|| "使用率データなし".to_string()),
                            )
                            .font(JAPANESE_FONT)
                            .size(14),
                        )
                        .width(Length::FillPortion(1)),
                    );

                    let mut moves_col = column![].spacing(2);
                    if let Some(usage) = usage {
                        for m in usage.moves.iter().take(8) {
                            moves_col = moves_col.push(
                                text(format!("{} ({})", m.name, m.rate))
                                    .font(JAPANESE_FONT)
                                    .size(12),
                            );
                        }
                    } else {
                        moves_col =
                            moves_col.push(text("使用率データなし").font(JAPANESE_FONT).size(12));
                    }
                    move_row = move_row.push(container(moves_col).width(Length::FillPortion(1)));

                    let mut items_col = column![].spacing(2);
                    if let Some(usage) = usage {
                        for i in usage.items.iter().take(3) {
                            items_col = items_col.push(
                                text(format!("{} ({})", i.name, i.rate))
                                    .font(JAPANESE_FONT)
                                    .size(12),
                            );
                        }
                    } else {
                        items_col =
                            items_col.push(text("使用率データなし").font(JAPANESE_FONT).size(12));
                    }
                    item_row = item_row.push(container(items_col).width(Length::FillPortion(1)));

                    let mut evs_col = column![].spacing(2);
                    if let Some(usage) = usage {
                        for e in usage.effort_values.iter().take(3) {
                            evs_col = evs_col.push(
                                text(format!(
                                    "H{} A{} B{} C{} D{} S{}\n({})",
                                    e.h, e.a, e.b, e.c, e.d, e.s, e.rate
                                ))
                                .font(JAPANESE_FONT)
                                .size(12),
                            );
                        }
                    } else {
                        evs_col =
                            evs_col.push(text("使用率データなし").font(JAPANESE_FONT).size(12));
                    }
                    ev_row = ev_row.push(container(evs_col).width(Length::FillPortion(1)));

                    let mut natures_col = column![].spacing(2);
                    if let Some(usage) = usage {
                        for n in usage.natures.iter().take(2) {
                            natures_col = natures_col.push(
                                text(format!("{} ({})", n.name, n.rate))
                                    .font(JAPANESE_FONT)
                                    .size(12),
                            );
                        }
                    } else {
                        natures_col =
                            natures_col.push(text("使用率データなし").font(JAPANESE_FONT).size(12));
                    }
                    nature_row =
                        nature_row.push(container(natures_col).width(Length::FillPortion(1)));
                }

                let mut content = column![].spacing(20);
                if let Some(conflict_summary) = format_conflict_summary(&party.conflicts) {
                    content = content.push(
                        container(text(conflict_summary).font(JAPANESE_FONT).size(14))
                            .padding(10)
                            .width(Length::Fill),
                    );
                }

                let table =
                    column![name_row, type_row, move_row, item_row, ev_row, nature_row].spacing(20);
                content = content.push(table);

                scrollable(content).into()
            }
        };

        container(column![header_row, content].spacing(20))
            .padding(20)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

fn format_slot_name(pokemon: &RecognizedPokemonView) -> String {
    let display_name = pokemon
        .usage
        .as_ref()
        .map(|usage| usage.name.as_str())
        .or(pokemon.display_name.as_deref())
        .unwrap_or("未判定");

    let mut lines = vec![format!("#{} {}", pokemon.slot_index + 1, display_name)];

    if pokemon.usage.is_none() {
        if let Some(candidates) = format_candidate_summary(pokemon) {
            lines.push(candidates);
        }
    }

    lines.join("\n")
}

fn format_candidate_summary(pokemon: &RecognizedPokemonView) -> Option<String> {
    let candidate = pokemon.candidates.first()?;
    Some(format!("候補: {}", candidate.display_name))
}

fn format_conflict_summary(conflicts: &[ConflictView]) -> Option<String> {
    if conflicts.is_empty() {
        return None;
    }

    let body = conflicts
        .iter()
        .map(|conflict| {
            let slots = conflict
                .slot_indices
                .iter()
                .map(|slot| format!("#{}", slot + 1))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{} -> {}", conflict.species_name, slots)
        })
        .collect::<Vec<_>>()
        .join(" / ");

    Some(format!("重複候補があります: {body}"))
}
