use super::components::VideoPreview;
use super::pokemon::{
    FieldType, PokemonState, PokemonView, SuggestionRequest, editor_input_ids, restore_input_ids,
};
use super::subscriptions::{self, RuntimeMessage};
use crate::services::{DesktopAppServices, SuggestionKind};
use champions_domain::party::SavedParty;
use champions_interface::{
    ConflictView, OpponentPartyView, PokemonUsageSummaryView, RecognizedPokemonView,
    RuntimeCommand, RuntimeEvent,
};
use champions_runtime::PreviewFrame;
use iced::advanced::widget as advanced_widget;
use iced::event;
use iced::keyboard::{self, key};
use iced::widget::Id as WidgetId;
use iced::widget::operation as widget_ops;
use iced::window;
use iced::{
    Border, Color, Element, Length, Rectangle, Size, Subscription, Task,
    widget::{button, column, container, row, scrollable, text, text_input},
};

use super::JAPANESE_FONT;

const EDITOR_SLOT_ORDER: [usize; 6] = [0, 1, 2, 3, 4, 5];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Editor,
    SelectionSupport,
}

#[derive(Debug, Clone)]
struct OpponentPartyState {
    pokemons: Vec<OpponentPokemonState>,
    conflicts: Vec<ConflictView>,
}

impl OpponentPartyState {
    fn from_view(party: OpponentPartyView) -> Self {
        Self {
            pokemons: party
                .pokemons
                .into_iter()
                .map(OpponentPokemonState::from_view)
                .collect(),
            conflicts: party.conflicts,
        }
    }
}

#[derive(Debug, Clone)]
struct OpponentPokemonState {
    slot_index: u8,
    input_name: String,
    recognized_name: Option<String>,
    usage: Option<PokemonUsageSummaryView>,
    suggestions: Vec<String>,
}

impl OpponentPokemonState {
    fn from_view(pokemon: RecognizedPokemonView) -> Self {
        let input_name = pokemon
            .usage
            .as_ref()
            .map(|usage| usage.name.clone())
            .or_else(|| pokemon.display_name.clone())
            .unwrap_or_default();

        Self {
            slot_index: pokemon.slot_index,
            input_name,
            recognized_name: pokemon.display_name,
            usage: pokemon.usage,
            suggestions: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct RestoreWindowState {
    id: window::Id,
    target_index: usize,
    drafts: Vec<PokemonState>,
    feedback: Option<String>,
}

pub struct PokeEditorApp {
    pokemons: [PokemonState; 6],
    pokemon_feedbacks: [Option<String>; 6],
    saved_party: SavedParty,
    active_tab: Tab,
    opponent_party: Option<OpponentPartyState>,
    services: DesktopAppServices,
    latest_preview: Option<PreviewFrame>,
    preview_window_id: Option<window::Id>,
    main_window_id: Option<window::Id>,
    restore_window: Option<RestoreWindowState>,
    is_refreshing: bool,
    editor_status: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Message {
    PokemonMsg(usize, super::pokemon::Message),
    RestoreDraftMsg(usize, super::pokemon::Message),
    OpponentPokemonNameChanged(usize, String),
    OpponentPokemonSuggestionSelected(usize, String),
    TabSelected(Tab),
    RestorePokemonSelected(usize),
    SaveRestoreDraft(usize),
    DeleteRestoreDraft(usize),
    CloseRestoreWindow,
    AdvanceInputFocus {
        window_id: window::Id,
        backwards: bool,
    },
    FocusedInputResolved {
        window_id: window::Id,
        backwards: bool,
        focused_id: Option<WidgetId>,
    },
    RuntimeMsg(RuntimeMessage),
    RuntimeCommandSent(Result<(), String>),
    WindowClosed(window::Id),
    RefreshUsageData,
    UsageDataRefreshed(Result<usize, String>),
}

impl PokeEditorApp {
    pub fn new(services: DesktopAppServices) -> (Self, Task<Message>) {
        let (saved_party, editor_status) = match services.load_party() {
            Ok(party) => (party, None),
            Err(error) => {
                eprintln!("Failed to load party: {error}");
                (
                    SavedParty::default(),
                    Some("保存済みパーティの読込に失敗しました".to_string()),
                )
            }
        };

        let pokemons = if saved_party.pokemons.is_empty() {
            std::array::from_fn(|_| PokemonState::new())
        } else {
            let mut pokes = std::array::from_fn(|_| PokemonState::new());
            for (i, build) in saved_party.pokemons.iter().cloned().enumerate().take(6) {
                pokes[i] = PokemonState::from_saved_build(build);
            }
            pokes
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
                width: 1920.0,
                height: 1080.0,
            },
            ..Default::default()
        });

        (
            Self {
                pokemons,
                pokemon_feedbacks: std::array::from_fn(|_| None),
                saved_party,
                active_tab: Tab::Editor,
                opponent_party: None,
                services,
                latest_preview: None,
                preview_window_id: Some(preview_id),
                main_window_id: Some(main_id),
                restore_window: None,
                is_refreshing: false,
                editor_status,
            },
            Task::batch([main_task.discard(), preview_task.discard()]),
        )
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::PokemonMsg(index, msg) => match msg {
                super::pokemon::Message::SaveRequested => {
                    self.save_pokemon(index);
                }
                super::pokemon::Message::RestoreRequested => {
                    return self.open_restore_window(index);
                }
                other => {
                    self.pokemon_feedbacks[index] = None;
                    let request = self.pokemons[index].update(other);
                    if let Some(req) = request {
                        self.handle_suggestion_request(index, req);
                    }
                }
            },
            Message::RestoreDraftMsg(history_index, msg) => {
                let request = if let Some(window_state) = self.restore_window.as_mut() {
                    window_state.feedback = None;
                    window_state
                        .drafts
                        .get_mut(history_index)
                        .and_then(|draft| draft.update(msg))
                } else {
                    None
                };

                if let Some(req) = request {
                    self.handle_restore_suggestion_request(history_index, req);
                }
            }
            Message::OpponentPokemonNameChanged(index, value) => {
                self.handle_opponent_pokemon_name_changed(index, value);
            }
            Message::OpponentPokemonSuggestionSelected(index, name) => {
                self.handle_opponent_pokemon_selection(index, name);
            }
            Message::TabSelected(tab) => {
                let previous_tab = self.active_tab;
                self.active_tab = tab;

                if previous_tab != tab {
                    let command = match tab {
                        Tab::Editor => RuntimeCommand::StopRecognition,
                        Tab::SelectionSupport => RuntimeCommand::StartRecognition,
                    };

                    return Task::perform(
                        subscriptions::send_command(command),
                        Message::RuntimeCommandSent,
                    );
                }
            }
            Message::RuntimeCommandSent(result) => {
                if let Err(error) = result {
                    eprintln!("[Runtime] command failed: {error}");
                }
            }
            Message::RestorePokemonSelected(history_index) => {
                return self.restore_pokemon_from_history(history_index);
            }
            Message::SaveRestoreDraft(history_index) => {
                self.save_restore_draft(history_index);
            }
            Message::DeleteRestoreDraft(history_index) => {
                self.delete_restore_draft(history_index);
            }
            Message::CloseRestoreWindow => {
                return self.close_restore_window();
            }
            Message::AdvanceInputFocus {
                window_id,
                backwards,
            } => {
                return self.request_focused_input(window_id, backwards);
            }
            Message::FocusedInputResolved {
                window_id,
                backwards,
                focused_id,
            } => {
                return self.focus_relative_input(window_id, backwards, focused_id);
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
                } else if self.restore_window.as_ref().map(|state| state.id) == Some(id) {
                    self.restore_window = None;
                } else if self.main_window_id == Some(id) {
                    return iced::exit();
                }
            }
            Message::RefreshUsageData => {
                if self.is_refreshing {
                    return Task::none();
                }
                self.is_refreshing = true;

                let services = self.services.clone();

                return Task::perform(
                    async move {
                        match tokio::task::spawn_blocking(move || services.refresh_usage_data())
                            .await
                        {
                            Ok(result) => result,
                            Err(error) => Err(error.to_string()),
                        }
                    },
                    Message::UsageDataRefreshed,
                );
            }
            Message::UsageDataRefreshed(result) => {
                self.is_refreshing = false;
                match result {
                    Ok(count) => {
                        self.refresh_opponent_usage();
                        println!("使用率データを {} 件更新しました", count);
                    }
                    Err(e) => eprintln!("使用率データの更新に失敗しました: {}", e),
                }
            }
        }
        Task::none()
    }

    fn save_pokemon(&mut self, index: usize) {
        let build = self.pokemons[index].to_build();
        if build.is_blank() {
            self.pokemon_feedbacks[index] = None;
            self.editor_status = Some(format!("ポケモン{}は未入力のため保存できません", index + 1));
            return;
        }

        let mut saved_party = self.saved_party.clone();
        if saved_party.pokemons.len() <= index {
            saved_party.pokemons.resize(index + 1, Default::default());
        }
        saved_party.pokemons[index] = build.clone();
        let is_duplicate_in_library = saved_party.has_saved_pokemon_equivalent(&build);
        if !is_duplicate_in_library {
            saved_party.remember_pokemon(build);
        }

        if self.persist_saved_party(saved_party).is_ok() {
            self.pokemon_feedbacks[index] = if is_duplicate_in_library {
                Some("一覧に同じ内容があるため保存しませんでした".to_string())
            } else {
                None
            };
        }
    }

    fn open_restore_window(&mut self, index: usize) -> Task<Message> {
        if self.saved_party.saved_pokemons.is_empty() {
            self.editor_status = Some("保存済みポケモンがまだありません".to_string());
            return Task::none();
        }

        if let Some(window_state) = self.restore_window.as_mut() {
            window_state.target_index = index;
            window_state.feedback = None;
            return Task::none();
        }

        let (id, task) = window::open(window::Settings {
            size: Size {
                width: 760.0,
                height: 720.0,
            },
            ..Default::default()
        });
        self.restore_window = Some(RestoreWindowState {
            id,
            target_index: index,
            drafts: self
                .saved_party
                .saved_pokemons
                .iter()
                .cloned()
                .map(PokemonState::from_saved_build)
                .collect(),
            feedback: None,
        });
        task.discard()
    }

    fn restore_pokemon_from_history(&mut self, history_index: usize) -> Task<Message> {
        let Some(window_state) = self.restore_window.as_ref() else {
            self.editor_status = Some("復元先のスロットが見つかりません".to_string());
            return Task::none();
        };

        let Some(build) = window_state
            .drafts
            .get(history_index)
            .cloned()
            .map(|draft| draft.to_build())
        else {
            if let Some(window_state) = self.restore_window.as_mut() {
                window_state.feedback = Some("選択した保存済み内容が見つかりません".to_string());
            }
            return Task::none();
        };
        let target_index = window_state.target_index;

        if build.is_blank() {
            if let Some(window_state) = self.restore_window.as_mut() {
                window_state.feedback = Some("内容が空のため復元できません".to_string());
            }
            return Task::none();
        }

        let mut saved_party = self.saved_party.clone();
        if saved_party.pokemons.len() <= target_index {
            saved_party
                .pokemons
                .resize(target_index + 1, Default::default());
        }
        saved_party.pokemons[target_index] = build.clone();

        match self.persist_saved_party(saved_party) {
            Ok(()) => {
                self.pokemons[target_index] = PokemonState::from_saved_build(build);
                self.pokemon_feedbacks[target_index] = None;
                return self.close_restore_window();
            }
            Err(error) => {
                if let Some(window_state) = self.restore_window.as_mut() {
                    window_state.feedback = Some(format!("復元に失敗しました: {error}"));
                }
            }
        }

        Task::none()
    }

    fn close_restore_window(&mut self) -> Task<Message> {
        let Some(window_state) = self.restore_window.as_ref() else {
            return Task::none();
        };

        window::close(window_state.id)
    }

    fn persist_saved_party(&mut self, saved_party: SavedParty) -> Result<(), String> {
        match self.services.save_party(saved_party.clone()) {
            Ok(()) => {
                self.saved_party = saved_party;
                self.editor_status = None;
                Ok(())
            }
            Err(error) => {
                eprintln!("Failed to save party: {error}");
                self.editor_status = Some(format!("保存に失敗しました: {error}"));
                Err(error)
            }
        }
    }

    fn handle_runtime_event(&mut self, event: RuntimeEvent) {
        match event {
            RuntimeEvent::OpponentPartyRecognized { party, .. } => {
                self.opponent_party = Some(OpponentPartyState::from_view(party));
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
            FieldType::Species => SuggestionKind::Species,
            FieldType::Move(_) => SuggestionKind::Move,
            FieldType::Item => SuggestionKind::Item,
            FieldType::Ability => SuggestionKind::Ability,
        };

        let suggestions = self.services.suggest_names(kind, &req.query, 5);
        self.pokemons[index].set_suggestions(suggestions);
    }

    fn handle_restore_suggestion_request(&mut self, history_index: usize, req: SuggestionRequest) {
        let suggestions = if req.query.is_empty() {
            Vec::new()
        } else {
            let kind = match req.field {
                FieldType::Species => SuggestionKind::Species,
                FieldType::Move(_) => SuggestionKind::Move,
                FieldType::Item => SuggestionKind::Item,
                FieldType::Ability => SuggestionKind::Ability,
            };

            self.services.suggest_names(kind, &req.query, 5)
        };

        if let Some(window_state) = self.restore_window.as_mut() {
            if let Some(draft) = window_state.drafts.get_mut(history_index) {
                draft.set_suggestions(suggestions);
            }
        }
    }

    fn save_restore_draft(&mut self, history_index: usize) {
        let Some(build) = self
            .restore_window
            .as_ref()
            .and_then(|window_state| window_state.drafts.get(history_index))
            .cloned()
            .map(|draft| draft.to_build())
        else {
            if let Some(window_state) = self.restore_window.as_mut() {
                window_state.feedback = Some("保存対象が見つかりません".to_string());
            }
            return;
        };

        if build.is_blank() {
            if let Some(window_state) = self.restore_window.as_mut() {
                window_state.feedback = Some(
                    "内容が空のため保存できません。削除する場合はゴミ箱を使用してください"
                        .to_string(),
                );
            }
            return;
        }

        let mut saved_party = self.saved_party.clone();
        if saved_party.has_saved_pokemon_equivalent_except(&build, history_index) {
            if let Some(window_state) = self.restore_window.as_mut() {
                window_state.feedback = Some("一覧に同じ内容があるため保存できません".to_string());
            }
            return;
        }
        let Some(saved_pokemon) = saved_party.saved_pokemons.get_mut(history_index) else {
            if let Some(window_state) = self.restore_window.as_mut() {
                window_state.feedback = Some("保存対象が見つかりません".to_string());
            }
            return;
        };
        *saved_pokemon = build.clone();

        match self.persist_saved_party(saved_party) {
            Ok(()) => {
                if let Some(window_state) = self.restore_window.as_mut() {
                    if let Some(draft) = window_state.drafts.get_mut(history_index) {
                        *draft = PokemonState::from_saved_build(build);
                    }
                    window_state.feedback =
                        Some(format!("保存履歴 {} を更新しました", history_index + 1));
                }
            }
            Err(error) => {
                if let Some(window_state) = self.restore_window.as_mut() {
                    window_state.feedback = Some(format!("保存に失敗しました: {error}"));
                }
            }
        }
    }

    fn delete_restore_draft(&mut self, history_index: usize) {
        let mut saved_party = self.saved_party.clone();
        if history_index >= saved_party.saved_pokemons.len() {
            if let Some(window_state) = self.restore_window.as_mut() {
                window_state.feedback = Some("削除対象が見つかりません".to_string());
            }
            return;
        }

        saved_party.saved_pokemons.remove(history_index);

        match self.persist_saved_party(saved_party) {
            Ok(()) => {
                if let Some(window_state) = self.restore_window.as_mut() {
                    if history_index < window_state.drafts.len() {
                        window_state.drafts.remove(history_index);
                    }
                    window_state.feedback =
                        Some(format!("保存履歴 {} を削除しました", history_index + 1));
                }
            }
            Err(error) => {
                if let Some(window_state) = self.restore_window.as_mut() {
                    window_state.feedback = Some(format!("削除に失敗しました: {error}"));
                }
            }
        }
    }

    fn request_focused_input(&self, window_id: window::Id, backwards: bool) -> Task<Message> {
        if self.input_ids_for_window(window_id).is_none() {
            return Task::none();
        }

        advanced_widget::operate(find_focused_input()).map(move |focused_id| {
            Message::FocusedInputResolved {
                window_id,
                backwards,
                focused_id,
            }
        })
    }

    fn focus_relative_input(
        &self,
        window_id: window::Id,
        backwards: bool,
        focused_id: Option<WidgetId>,
    ) -> Task<Message> {
        let Some(input_ids) = self.input_ids_for_window(window_id) else {
            return Task::none();
        };

        if input_ids.is_empty() {
            return Task::none();
        }

        let next_index = match focused_id
            .as_ref()
            .and_then(|focused_id| input_ids.iter().position(|id| id == focused_id))
        {
            Some(current) if backwards => {
                if current == 0 {
                    input_ids.len() - 1
                } else {
                    current - 1
                }
            }
            Some(current) => (current + 1) % input_ids.len(),
            None if backwards => input_ids.len() - 1,
            None => 0,
        };

        widget_ops::focus(input_ids[next_index].clone())
    }

    fn input_ids_for_window(&self, window_id: window::Id) -> Option<Vec<WidgetId>> {
        if self.main_window_id == Some(window_id) && self.active_tab == Tab::Editor {
            let mut ids = Vec::new();
            for slot_index in EDITOR_SLOT_ORDER {
                ids.extend(editor_input_ids(slot_index));
            }
            Some(ids)
        } else if self.restore_window.as_ref().map(|state| state.id) == Some(window_id) {
            let mut ids = Vec::new();
            if let Some(window_state) = self.restore_window.as_ref() {
                for history_index in (0..window_state.drafts.len()).rev() {
                    ids.extend(restore_input_ids(history_index));
                }
            }
            Some(ids)
        } else {
            None
        }
    }

    fn handle_opponent_pokemon_name_changed(&mut self, index: usize, input_name: String) {
        let suggestions = self.suggest_species_names(&input_name);
        let usage = self.lookup_usage_summary_view(&input_name);

        if let Some(party) = self.opponent_party.as_mut() {
            if let Some(pokemon) = party.pokemons.get_mut(index) {
                pokemon.input_name = input_name;
                pokemon.suggestions = suggestions;
                pokemon.usage = usage;
            }
        }
    }

    fn handle_opponent_pokemon_selection(&mut self, index: usize, name: String) {
        let usage = self.lookup_usage_summary_view(&name);

        if let Some(party) = self.opponent_party.as_mut() {
            if let Some(pokemon) = party.pokemons.get_mut(index) {
                pokemon.input_name = name;
                pokemon.suggestions.clear();
                pokemon.usage = usage;
            }
        }
    }

    fn suggest_species_names(&self, query: &str) -> Vec<String> {
        self.services
            .suggest_names(SuggestionKind::Species, query, 5)
    }

    fn lookup_usage_summary_view(&self, name: &str) -> Option<PokemonUsageSummaryView> {
        self.services.lookup_usage_summary_view(name)
    }

    fn refresh_opponent_usage(&mut self) {
        let usage_updates = match self.opponent_party.as_ref() {
            Some(party) => party
                .pokemons
                .iter()
                .map(|pokemon| self.lookup_usage_summary_view(&pokemon.input_name))
                .collect::<Vec<_>>(),
            None => return,
        };

        if let Some(party) = self.opponent_party.as_mut() {
            for (pokemon, usage) in party.pokemons.iter_mut().zip(usage_updates) {
                pokemon.usage = usage;
            }
        }
    }

    fn saved_pokemon_count(&self) -> usize {
        self.saved_party.saved_pokemons.len()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let preview_sub = subscriptions::preview_subscription().map(Message::RuntimeMsg);
        let event_sub = subscriptions::event_subscription().map(Message::RuntimeMsg);

        let close_sub = window::close_events().map(Message::WindowClosed);
        let tab_sub = event::listen_with(|event, status, window_id| match event {
            iced::Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(key::Named::Tab),
                modifiers,
                ..
            }) if status == event::Status::Ignored => Some(Message::AdvanceInputFocus {
                window_id,
                backwards: modifiers.shift(),
            }),
            _ => None,
        });

        Subscription::batch([preview_sub, event_sub, close_sub, tab_sub])
    }

    pub fn view(&self, id: window::Id) -> Element<'_, Message> {
        if self.preview_window_id == Some(id) {
            return self.preview_view();
        }
        if self.restore_window.as_ref().map(|state| state.id) == Some(id) {
            return self.restore_picker_view();
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
        } else if self.restore_window.as_ref().map(|state| state.id) == Some(id) {
            "Saved Pokemon Restore".to_string()
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

    fn restore_picker_view(&self) -> Element<'_, Message> {
        let Some(window_state) = self.restore_window.as_ref() else {
            return container(text("復元ウィンドウを初期化できませんでした").font(JAPANESE_FONT))
                .padding(20)
                .into();
        };

        let header_row = row![
            text(format!(
                "一覧 (復元先: ポケモン{})",
                window_state.target_index + 1
            ))
            .font(JAPANESE_FONT)
            .size(24),
            button(text("閉じる").font(JAPANESE_FONT))
                .on_press(Message::CloseRestoreWindow)
                .padding(10),
        ]
        .spacing(20)
        .align_y(iced::Alignment::Center);

        let mut header = column![header_row].spacing(12);
        if let Some(feedback) = &window_state.feedback {
            header = header.push(text(feedback).font(JAPANESE_FONT).size(14));
        }

        let content: Element<'_, Message> = if window_state.drafts.is_empty() {
            container(
                text("保存済みポケモンがまだありません")
                    .font(JAPANESE_FONT)
                    .size(16),
            )
            .padding(20)
            .width(Length::Fill)
            .into()
        } else {
            let cards =
                column(window_state.drafts.iter().enumerate().rev().map(
                    |(history_index, draft)| restore_pokemon_card(history_index, draft).into(),
                ))
                .spacing(12);

            scrollable(cards).into()
        };

        container(column![header, content].spacing(20).padding(20))
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn editor_view(&self) -> Element<'_, Message> {
        let has_saved_pokemon_library = self.saved_pokemon_count() > 0;

        let grid = row![
            column![
                PokemonView::view(
                    0,
                    &self.pokemons[0],
                    has_saved_pokemon_library,
                    self.pokemon_feedbacks[0].as_deref()
                )
                .map(|m| Message::PokemonMsg(0, m)),
                PokemonView::view(
                    2,
                    &self.pokemons[2],
                    has_saved_pokemon_library,
                    self.pokemon_feedbacks[2].as_deref()
                )
                .map(|m| Message::PokemonMsg(2, m)),
                PokemonView::view(
                    4,
                    &self.pokemons[4],
                    has_saved_pokemon_library,
                    self.pokemon_feedbacks[4].as_deref()
                )
                .map(|m| Message::PokemonMsg(4, m)),
            ]
            .spacing(20),
            column![
                PokemonView::view(
                    1,
                    &self.pokemons[1],
                    has_saved_pokemon_library,
                    self.pokemon_feedbacks[1].as_deref()
                )
                .map(|m| Message::PokemonMsg(1, m)),
                PokemonView::view(
                    3,
                    &self.pokemons[3],
                    has_saved_pokemon_library,
                    self.pokemon_feedbacks[3].as_deref()
                )
                .map(|m| Message::PokemonMsg(3, m)),
                PokemonView::view(
                    5,
                    &self.pokemons[5],
                    has_saved_pokemon_library,
                    self.pokemon_feedbacks[5].as_deref()
                )
                .map(|m| Message::PokemonMsg(5, m)),
            ]
            .spacing(20),
        ]
        .spacing(40);

        let header = text("Party Editor").size(32);
        let mut content = column![header].spacing(20);

        if let Some(status) = &self.editor_status {
            content = content.push(text(status).font(JAPANESE_FONT).size(14));
        }

        scrollable(content.push(grid)).into()
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
                let mut ability_row = row![
                    container(text("特性").font(JAPANESE_FONT).size(16)).width(Length::Fixed(80.0))
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

                for (index, pokemon) in party.pokemons.iter().enumerate() {
                    let usage = pokemon.usage.as_ref();

                    let mut name_cell = column![
                        text(format!("#{}", pokemon.slot_index + 1))
                            .font(JAPANESE_FONT)
                            .size(14),
                        text_input("相手ポケモン名", &pokemon.input_name)
                            .on_input(move |value| Message::OpponentPokemonNameChanged(
                                index, value
                            ))
                            .font(JAPANESE_FONT)
                            .width(Length::Fill),
                    ]
                    .spacing(6);

                    if let Some(hint) = format_opponent_hint(pokemon) {
                        name_cell = name_cell.push(text(hint).font(JAPANESE_FONT).size(12));
                    }

                    if !pokemon.suggestions.is_empty() {
                        let suggestion_list =
                            column(pokemon.suggestions.iter().map(|suggestion| {
                                button(text(suggestion).font(JAPANESE_FONT).size(13))
                                    .on_press(Message::OpponentPokemonSuggestionSelected(
                                        index,
                                        suggestion.clone(),
                                    ))
                                    .style(button::secondary)
                                    .width(Length::Fill)
                                    .into()
                            }))
                            .spacing(2);

                        name_cell =
                            name_cell.push(container(suggestion_list).padding(4).style(|_| {
                                container::Style {
                                    border: Border {
                                        color: Color::from_rgb(0.7, 0.7, 0.7),
                                        width: 1.0,
                                        radius: 4.0.into(),
                                    },
                                    ..Default::default()
                                }
                            }));
                    }

                    name_row = name_row.push(container(name_cell).width(Length::FillPortion(1)));
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

                    let mut abilities_col = column![].spacing(2);
                    if let Some(usage) = usage {
                        if usage.abilities.is_empty() {
                            abilities_col =
                                abilities_col.push(text("-").font(JAPANESE_FONT).size(12));
                        } else {
                            for ability in usage.abilities.iter().take(3) {
                                abilities_col = abilities_col.push(
                                    text(format!("{} ({})", ability.name, ability.rate))
                                        .font(JAPANESE_FONT)
                                        .size(12),
                                );
                            }
                        }
                    } else {
                        abilities_col = abilities_col
                            .push(text("使用率データなし").font(JAPANESE_FONT).size(12));
                    }
                    ability_row =
                        ability_row.push(container(abilities_col).width(Length::FillPortion(1)));

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

                let table = column![
                    name_row,
                    type_row,
                    move_row,
                    item_row,
                    ability_row,
                    ev_row,
                    nature_row
                ]
                .spacing(20);
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

fn restore_pokemon_card<'a>(history_index: usize, draft: &'a PokemonState) -> Element<'a, Message> {
    let header = row![
        column![
            text(format!("保存履歴 {}", history_index + 1))
                .font(JAPANESE_FONT)
                .size(12)
                .color(Color::from_rgb(0.45, 0.45, 0.45)),
            text(display_draft_name(draft)).font(JAPANESE_FONT).size(20),
        ]
        .spacing(4)
        .width(Length::Fill),
        button(text("復元").font(JAPANESE_FONT))
            .on_press(Message::RestorePokemonSelected(history_index))
            .padding([8, 12]),
        button(text("保存").font(JAPANESE_FONT))
            .on_press(Message::SaveRestoreDraft(history_index))
            .padding([8, 12]),
        button(text("ゴミ箱").font(JAPANESE_FONT))
            .on_press(Message::DeleteRestoreDraft(history_index))
            .padding([8, 12]),
    ]
    .spacing(10)
    .align_y(iced::Alignment::Center);

    let content = column![
        header,
        PokemonView::restore_form(history_index, draft)
            .map(move |message| Message::RestoreDraftMsg(history_index, message)),
    ]
    .spacing(12);

    container(content)
        .padding(14)
        .width(Length::Fill)
        .style(|_| container::Style {
            border: Border {
                color: Color::from_rgb(0.8, 0.8, 0.8),
                width: 1.0,
                radius: 6.0.into(),
            },
            ..Default::default()
        })
        .into()
}

fn display_draft_name(draft: &PokemonState) -> &str {
    let species = draft.species.trim();
    if species.is_empty() {
        "（名前未入力）"
    } else {
        species
    }
}

fn find_focused_input() -> impl advanced_widget::Operation<Option<WidgetId>> {
    struct FindFocusedInput {
        focused: Option<WidgetId>,
    }

    impl advanced_widget::Operation<Option<WidgetId>> for FindFocusedInput {
        fn focusable(
            &mut self,
            id: Option<&WidgetId>,
            _bounds: Rectangle,
            state: &mut dyn advanced_widget::operation::Focusable,
        ) {
            if self.focused.is_none() && state.is_focused() && id.is_some() {
                self.focused = id.cloned();
            }
        }

        fn traverse(
            &mut self,
            operate: &mut dyn FnMut(&mut dyn advanced_widget::Operation<Option<WidgetId>>),
        ) {
            if self.focused.is_none() {
                operate(self);
            }
        }

        fn finish(&self) -> advanced_widget::operation::Outcome<Option<WidgetId>> {
            advanced_widget::operation::Outcome::Some(self.focused.clone())
        }
    }

    FindFocusedInput { focused: None }
}

fn format_opponent_hint(pokemon: &OpponentPokemonState) -> Option<String> {
    if let Some(recognized) = &pokemon.recognized_name {
        if recognized != &pokemon.input_name {
            return Some(format!("認識: {}", recognized));
        }
    }
    None
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
