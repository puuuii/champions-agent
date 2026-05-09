use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, AtomicU32, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use tokio::sync::mpsc;

use champions_interface::{
    AbilityUsageView, CandidateView, ConfidenceView, ConflictView, EffortValueUsageView,
    EventSequence, FrameSequence, ItemUsageView, MoveUsageView, NatureUsageView, OpponentPartyView,
    PokemonUsageSummaryView, RecognitionAttemptId, RecognizedPokemonView, RuntimeCommand,
    RuntimeEvent,
};

use crate::frame::CapturedFrame;
use crate::handle::RuntimeHandle;
use crate::latest::{LatestFrame, LatestPreview};
use crate::recognition::RecognitionPort;
use crate::scheduler::RecognitionScheduler;
use crate::shutdown::{ShutdownSignal, ShutdownToken, shutdown_pair};
use crate::traits::{CaptureError, FrameSource, PreviewFrameConverter};

const COMMAND_CHANNEL_SIZE: usize = 32;
const EVENT_CHANNEL_SIZE: usize = 64;
const PREVIEW_CHANNEL_SIZE: usize = 2;

pub struct RuntimeBuilder {
    frame_source: Option<Box<dyn FrameSource>>,
    preview_converter: Option<Box<dyn PreviewFrameConverter>>,
    recognition_port: Option<Box<dyn RecognitionPort>>,
    preview_max_width: u32,
    preview_target_fps: u8,
}

impl RuntimeBuilder {
    pub fn new() -> Self {
        Self {
            frame_source: None,
            preview_converter: None,
            recognition_port: None,
            preview_max_width: 960,
            preview_target_fps: 15,
        }
    }

    pub fn frame_source(mut self, source: Box<dyn FrameSource>) -> Self {
        self.frame_source = Some(source);
        self
    }

    pub fn preview_converter(mut self, converter: Box<dyn PreviewFrameConverter>) -> Self {
        self.preview_converter = Some(converter);
        self
    }

    pub fn recognition_port(mut self, port: Box<dyn RecognitionPort>) -> Self {
        self.recognition_port = Some(port);
        self
    }

    pub fn preview_max_width(mut self, width: u32) -> Self {
        self.preview_max_width = width;
        self
    }

    pub fn preview_target_fps(mut self, fps: u8) -> Self {
        self.preview_target_fps = fps;
        self
    }

    pub fn build(self) -> (RuntimeHandle, RuntimeWorkers) {
        let frame_source = self.frame_source.expect("frame_source is required");
        let preview_converter = self
            .preview_converter
            .expect("preview_converter is required");

        let (command_tx, command_rx) = mpsc::channel(COMMAND_CHANNEL_SIZE);
        let (event_tx, event_rx) = mpsc::channel(EVENT_CHANNEL_SIZE);
        let (preview_notify_tx, preview_notify_rx) = mpsc::channel(PREVIEW_CHANNEL_SIZE);
        let (shutdown_signal, shutdown_token) = shutdown_pair();

        let latest_frame = LatestFrame::new();
        let latest_preview = LatestPreview::new();

        let handle = RuntimeHandle::new(
            command_tx,
            event_rx,
            preview_notify_rx,
            latest_preview.clone(),
        );

        let workers = RuntimeWorkers {
            frame_source,
            preview_converter,
            recognition_port: self.recognition_port,
            command_rx,
            event_tx,
            preview_notify_tx,
            shutdown_signal,
            shutdown_token,
            latest_frame,
            latest_preview,
            preview_max_width: self.preview_max_width,
            preview_target_fps: self.preview_target_fps,
        };

        (handle, workers)
    }
}

impl Default for RuntimeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub struct RuntimeWorkers {
    frame_source: Box<dyn FrameSource>,
    preview_converter: Box<dyn PreviewFrameConverter>,
    recognition_port: Option<Box<dyn RecognitionPort>>,
    command_rx: mpsc::Receiver<RuntimeCommand>,
    event_tx: mpsc::Sender<RuntimeEvent>,
    preview_notify_tx: mpsc::Sender<()>,
    shutdown_signal: ShutdownSignal,
    shutdown_token: ShutdownToken,
    latest_frame: LatestFrame,
    latest_preview: LatestPreview,
    preview_max_width: u32,
    preview_target_fps: u8,
}

impl RuntimeWorkers {
    pub async fn run(self) {
        let RuntimeWorkers {
            frame_source,
            preview_converter,
            recognition_port,
            mut command_rx,
            event_tx,
            preview_notify_tx,
            shutdown_signal,
            shutdown_token,
            latest_frame,
            latest_preview,
            preview_max_width,
            preview_target_fps,
        } = self;

        let control = Arc::new(RuntimeControl::new(preview_max_width, preview_target_fps));
        let event_seq = EventSequencer::default();
        let frame_seq = FrameSequencer::default();

        let capture_worker = CaptureWorker {
            frame_source,
            latest_frame: latest_frame.clone(),
            event_tx: event_tx.clone(),
            event_seq: event_seq.clone(),
            frame_seq,
            control: control.clone(),
            shutdown_token: shutdown_token.clone(),
        }
        .spawn();

        let preview_worker = PreviewWorker {
            preview_converter,
            latest_frame: latest_frame.clone(),
            latest_preview,
            preview_notify_tx,
            control: control.clone(),
            shutdown_token: shutdown_token.clone(),
        }
        .spawn();

        let recognition_worker = recognition_port.map(|recognition_port| {
            RecognitionWorker {
                recognition_port,
                latest_frame,
                event_tx: event_tx.clone(),
                event_seq: event_seq.clone(),
                control: control.clone(),
                shutdown_token: shutdown_token.clone(),
            }
            .spawn()
        });

        self::run_command_loop(
            &mut command_rx,
            &event_tx,
            &shutdown_signal,
            &control,
            &event_seq,
            recognition_worker.is_some(),
        )
        .await;

        let _ = capture_worker.await;
        let _ = preview_worker.await;
        if let Some(recognition_worker) = recognition_worker {
            let _ = recognition_worker.await;
        }
    }
}

const RECOGNITION_TICK_INTERVAL: Duration = Duration::from_millis(100);
const IDLE_POLL_INTERVAL: Duration = Duration::from_millis(20);

#[derive(Default, Clone)]
struct EventSequencer {
    next: Arc<AtomicU64>,
}

impl EventSequencer {
    fn next(&self) -> EventSequence {
        EventSequence(self.next.fetch_add(1, Ordering::Relaxed) + 1)
    }
}

#[derive(Default, Clone)]
struct FrameSequencer {
    next: Arc<AtomicU64>,
}

impl FrameSequencer {
    fn next(&self) -> FrameSequence {
        FrameSequence(self.next.fetch_add(1, Ordering::Relaxed) + 1)
    }
}

struct RuntimeControl {
    capturing: AtomicBool,
    preview_enabled: AtomicBool,
    recognition_enabled: AtomicBool,
    preview_max_width: AtomicU32,
    preview_target_fps: AtomicU8,
    recognition_generation: AtomicU64,
}

impl RuntimeControl {
    fn new(preview_max_width: u32, preview_target_fps: u8) -> Self {
        Self {
            capturing: AtomicBool::new(false),
            preview_enabled: AtomicBool::new(true),
            recognition_enabled: AtomicBool::new(false),
            preview_max_width: AtomicU32::new(preview_max_width),
            preview_target_fps: AtomicU8::new(preview_target_fps.max(1)),
            recognition_generation: AtomicU64::new(0),
        }
    }

    fn set_capturing(&self, capturing: bool) {
        self.capturing.store(capturing, Ordering::Relaxed);
    }

    fn is_capturing(&self) -> bool {
        self.capturing.load(Ordering::Relaxed)
    }

    fn set_preview_enabled(&self, preview_enabled: bool) {
        self.preview_enabled
            .store(preview_enabled, Ordering::Relaxed);
    }

    fn is_preview_enabled(&self) -> bool {
        self.preview_enabled.load(Ordering::Relaxed)
    }

    fn set_preview_max_width(&self, preview_max_width: u32) {
        self.preview_max_width
            .store(preview_max_width, Ordering::Relaxed);
    }

    fn preview_max_width(&self) -> u32 {
        self.preview_max_width.load(Ordering::Relaxed)
    }

    fn set_preview_target_fps(&self, preview_target_fps: u8) {
        self.preview_target_fps
            .store(preview_target_fps.max(1), Ordering::Relaxed);
    }

    fn preview_interval(&self) -> Duration {
        Duration::from_millis(1000 / self.preview_target_fps.load(Ordering::Relaxed) as u64)
    }

    fn set_recognition_enabled(&self, recognition_enabled: bool) {
        self.recognition_enabled
            .store(recognition_enabled, Ordering::Relaxed);
        self.recognition_generation.fetch_add(1, Ordering::Relaxed);
    }

    fn is_recognition_enabled(&self) -> bool {
        self.recognition_enabled.load(Ordering::Relaxed)
    }

    fn recognition_generation(&self) -> u64 {
        self.recognition_generation.load(Ordering::Relaxed)
    }
}

struct CaptureWorker {
    frame_source: Box<dyn FrameSource>,
    latest_frame: LatestFrame,
    event_tx: mpsc::Sender<RuntimeEvent>,
    event_seq: EventSequencer,
    frame_seq: FrameSequencer,
    control: Arc<RuntimeControl>,
    shutdown_token: ShutdownToken,
}

impl CaptureWorker {
    fn spawn(mut self) -> tokio::task::JoinHandle<()> {
        tokio::task::spawn_blocking(move || self.run())
    }

    fn run(&mut self) {
        while !self.shutdown_token.is_shutdown() {
            if !self.control.is_capturing() {
                std::thread::sleep(IDLE_POLL_INTERVAL);
                continue;
            }

            match self.frame_source.read_frame() {
                Ok(Some(frame)) => {
                    let frame = CapturedFrame {
                        frame_sequence: self.frame_seq.next(),
                        ..frame
                    };
                    self.latest_frame.store(frame);
                }
                Ok(None) => {}
                Err(error) => {
                    tracing::error!("capture error: {error}");
                    blocking_send_event(&self.event_tx, &self.event_seq, |event_sequence| {
                        RuntimeEvent::Error {
                            event_sequence,
                            error: map_capture_error(&error),
                        }
                    });
                }
            }

            std::thread::sleep(self.control.preview_interval());
        }
    }
}

struct PreviewWorker {
    preview_converter: Box<dyn PreviewFrameConverter>,
    latest_frame: LatestFrame,
    latest_preview: LatestPreview,
    preview_notify_tx: mpsc::Sender<()>,
    control: Arc<RuntimeControl>,
    shutdown_token: ShutdownToken,
}

impl PreviewWorker {
    fn spawn(mut self) -> tokio::task::JoinHandle<()> {
        tokio::task::spawn_blocking(move || self.run())
    }

    fn run(&mut self) {
        let mut last_previewed_frame_seq: Option<FrameSequence> = None;

        while !self.shutdown_token.is_shutdown() {
            if !self.control.is_capturing() || !self.control.is_preview_enabled() {
                std::thread::sleep(IDLE_POLL_INTERVAL);
                continue;
            }

            if let Some(frame) = self.latest_frame.peek() {
                if last_previewed_frame_seq != Some(frame.frame_sequence) {
                    let preview = self
                        .preview_converter
                        .convert(&frame, self.control.preview_max_width());
                    self.latest_preview.store(preview);
                    let _ = self.preview_notify_tx.try_send(());
                    last_previewed_frame_seq = Some(frame.frame_sequence);
                }
            }

            std::thread::sleep(self.control.preview_interval());
        }
    }
}

struct RecognitionWorker {
    recognition_port: Box<dyn RecognitionPort>,
    latest_frame: LatestFrame,
    event_tx: mpsc::Sender<RuntimeEvent>,
    event_seq: EventSequencer,
    control: Arc<RuntimeControl>,
    shutdown_token: ShutdownToken,
}

impl RecognitionWorker {
    fn spawn(self) -> tokio::task::JoinHandle<()> {
        tokio::task::spawn_blocking(move || self.run())
    }

    fn run(self) {
        let mut scheduler = RecognitionScheduler::new();
        let mut attempt_id = 0_u64;
        let mut recognition_generation = self.control.recognition_generation();

        while !self.shutdown_token.is_shutdown() {
            let next_generation = self.control.recognition_generation();
            if next_generation != recognition_generation {
                scheduler.reset();
                recognition_generation = next_generation;
            }

            if !self.control.is_capturing() || !self.control.is_recognition_enabled() {
                std::thread::sleep(IDLE_POLL_INTERVAL);
                continue;
            }

            self.run_tick(&mut scheduler, &mut attempt_id);
            std::thread::sleep(RECOGNITION_TICK_INTERVAL);
        }
    }

    fn run_tick(&self, scheduler: &mut RecognitionScheduler, attempt_id: &mut u64) {
        let now = Instant::now();

        if scheduler.should_run_ocr(now) {
            let frame = match self.latest_frame.peek() {
                Some(frame) => frame,
                None => return,
            };

            let ocr_image = self.recognition_port.extract_target_text_image(
                frame.image.width,
                frame.image.height,
                &frame.image.bytes,
            );

            match self.recognition_port.detect_selection_screen(ocr_image) {
                Ok(result) => {
                    let previous_state = scheduler.state();
                    scheduler.on_ocr_result(result.screen_state, now);

                    if scheduler.state() != previous_state {
                        tracing::debug!(
                            "scheduler state: {:?} -> {:?} (text: {:?})",
                            previous_state,
                            scheduler.state(),
                            result.raw_text
                        );
                    }
                }
                Err(error) => {
                    tracing::warn!("OCR failed: {error}");
                    blocking_send_event(&self.event_tx, &self.event_seq, |event_sequence| {
                        RuntimeEvent::Error {
                            event_sequence,
                            error: champions_interface::RuntimeError::RecognitionFailed(error),
                        }
                    });
                }
            }
        }

        if scheduler.should_run_identification() {
            let frame = match self.latest_frame.peek() {
                Some(frame) => frame,
                None => return,
            };

            let party_images = self.recognition_port.extract_party_slots(
                frame.image.width,
                frame.image.height,
                &frame.image.bytes,
            );

            match self.recognition_port.identify_opponent_party(party_images) {
                Ok(result) => {
                    *attempt_id += 1;
                    scheduler.on_identification_complete(now);

                    let party_view = map_to_opponent_party_view(&result);
                    let frame_sequence = frame.frame_sequence;
                    let recognition_attempt_id = RecognitionAttemptId(*attempt_id);

                    blocking_send_event(&self.event_tx, &self.event_seq, |event_sequence| {
                        RuntimeEvent::OpponentPartyRecognized {
                            event_sequence,
                            frame_sequence,
                            attempt_id: recognition_attempt_id,
                            party: party_view,
                        }
                    });

                    tracing::info!(
                        "opponent party identified (attempt {}): {} pokemon, {} conflicts",
                        attempt_id,
                        result.recognized_party.pokemons.len(),
                        result.conflicts.len()
                    );
                }
                Err(error) => {
                    tracing::error!("party identification failed: {error}");
                    blocking_send_event(&self.event_tx, &self.event_seq, |event_sequence| {
                        RuntimeEvent::Error {
                            event_sequence,
                            error: champions_interface::RuntimeError::RecognitionFailed(error),
                        }
                    });
                }
            }
        }
    }
}

async fn run_command_loop(
    command_rx: &mut mpsc::Receiver<RuntimeCommand>,
    event_tx: &mpsc::Sender<RuntimeEvent>,
    shutdown_signal: &ShutdownSignal,
    control: &RuntimeControl,
    event_seq: &EventSequencer,
    has_recognition_worker: bool,
) {
    while let Some(command) = command_rx.recv().await {
        match command {
            RuntimeCommand::Shutdown => {
                shutdown_signal.trigger();
                send_event(event_tx, event_seq, |event_sequence| {
                    RuntimeEvent::RuntimeStopped { event_sequence }
                })
                .await;
                return;
            }
            RuntimeCommand::StartCapture => {
                control.set_capturing(true);
                send_event(event_tx, event_seq, |event_sequence| {
                    RuntimeEvent::CaptureStatusChanged {
                        event_sequence,
                        status: champions_interface::CaptureStatus::Running,
                    }
                })
                .await;
            }
            RuntimeCommand::StopCapture => {
                control.set_capturing(false);
                send_event(event_tx, event_seq, |event_sequence| {
                    RuntimeEvent::CaptureStatusChanged {
                        event_sequence,
                        status: champions_interface::CaptureStatus::Stopped,
                    }
                })
                .await;
            }
            RuntimeCommand::StartRecognition if has_recognition_worker => {
                control.set_recognition_enabled(true);
                send_event(event_tx, event_seq, |event_sequence| {
                    RuntimeEvent::RecognitionStatusChanged {
                        event_sequence,
                        status: champions_interface::RecognitionStatus::Running,
                    }
                })
                .await;
            }
            RuntimeCommand::StopRecognition => {
                control.set_recognition_enabled(false);
                send_event(event_tx, event_seq, |event_sequence| {
                    RuntimeEvent::RecognitionStatusChanged {
                        event_sequence,
                        status: champions_interface::RecognitionStatus::Stopped,
                    }
                })
                .await;
            }
            RuntimeCommand::StartRecognition => {}
            RuntimeCommand::SetPreviewEnabled(enabled) => {
                control.set_preview_enabled(enabled);
            }
            RuntimeCommand::SetPreviewMaxWidth(preview_max_width) => {
                control.set_preview_max_width(preview_max_width);
            }
            RuntimeCommand::SetPreviewTargetFps(preview_target_fps) => {
                control.set_preview_target_fps(preview_target_fps);
            }
        }
    }

    shutdown_signal.trigger();
    send_event(event_tx, event_seq, |event_sequence| {
        RuntimeEvent::RuntimeStopped { event_sequence }
    })
    .await;
}

async fn send_event<F>(event_tx: &mpsc::Sender<RuntimeEvent>, event_seq: &EventSequencer, build: F)
where
    F: FnOnce(EventSequence) -> RuntimeEvent,
{
    let _ = event_tx.send(build(event_seq.next())).await;
}

fn blocking_send_event<F>(
    event_tx: &mpsc::Sender<RuntimeEvent>,
    event_seq: &EventSequencer,
    build: F,
) where
    F: FnOnce(EventSequence) -> RuntimeEvent,
{
    let _ = event_tx.blocking_send(build(event_seq.next()));
}

fn map_capture_error(error: &CaptureError) -> champions_interface::RuntimeError {
    match error {
        CaptureError::DeviceNotFound => champions_interface::RuntimeError::CaptureDeviceNotFound,
        CaptureError::ReadFailed(message) => {
            champions_interface::RuntimeError::CaptureReadFailed(message.clone())
        }
    }
}

fn map_to_opponent_party_view(
    result: &champions_application::use_cases::OpponentPartyIdentificationResult,
) -> OpponentPartyView {
    use champions_domain::recognition::ConfidenceScore;

    let usage_by_name: HashMap<&str, &champions_domain::usage::PokemonUsageSummary> = result
        .usage_summaries
        .iter()
        .map(|usage| (usage.name.as_str(), usage))
        .collect();

    let mut pokemons: Vec<_> = result
        .recognized_party
        .pokemons
        .iter()
        .map(|p| RecognizedPokemonView {
            slot_index: p.slot.0,
            display_name: p.display_name.clone(),
            confidence: match p.confidence {
                ConfidenceScore::High(s) => ConfidenceView::High(s),
                ConfidenceScore::Medium(s) => ConfidenceView::Medium(s),
                ConfidenceScore::Low(s) => ConfidenceView::Low(s),
                ConfidenceScore::Unknown => ConfidenceView::Unknown,
            },
            candidates: p
                .candidates
                .iter()
                .map(|c| CandidateView {
                    display_name: c.display_name.clone(),
                    score: c.score,
                })
                .collect(),
            usage: p
                .display_name
                .as_deref()
                .and_then(|name| usage_by_name.get(name).copied())
                .map(map_usage_summary_view),
        })
        .collect();
    pokemons.sort_by_key(|pokemon| pokemon.slot_index);

    let conflicts = result
        .conflicts
        .iter()
        .map(|c| ConflictView {
            species_name: c.species_name.clone(),
            slot_indices: c.slots.iter().map(|s| s.0).collect(),
        })
        .collect();

    OpponentPartyView {
        pokemons,
        conflicts,
    }
}

fn map_usage_summary_view(
    usage: &champions_domain::usage::PokemonUsageSummary,
) -> PokemonUsageSummaryView {
    PokemonUsageSummaryView {
        name: usage.name.clone(),
        types: usage.types.clone(),
        moves: usage
            .moves
            .iter()
            .map(|m| MoveUsageView {
                name: m.name.clone(),
                rate: m.rate.clone(),
            })
            .collect(),
        items: usage
            .items
            .iter()
            .map(|i| ItemUsageView {
                name: i.name.clone(),
                rate: i.rate.clone(),
            })
            .collect(),
        abilities: usage
            .abilities
            .iter()
            .map(|a| AbilityUsageView {
                name: a.name.clone(),
                rate: a.rate.clone(),
            })
            .collect(),
        effort_values: usage
            .effort_values
            .iter()
            .map(|ev| EffortValueUsageView {
                h: ev.h,
                a: ev.a,
                b: ev.b,
                c: ev.c,
                d: ev.d,
                s: ev.s,
                rate: ev.rate.clone(),
            })
            .collect(),
        natures: usage
            .natures
            .iter()
            .map(|n| NatureUsageView {
                name: n.name.clone(),
                rate: n.rate.clone(),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::map_to_opponent_party_view;
    use champions_application::use_cases::OpponentPartyIdentificationResult;
    use champions_domain::recognition::{
        ConfidenceScore, RecognizedParty, RecognizedPokemon, SelectionSlot,
    };
    use champions_domain::usage::{
        AbilityUsage, EffortValueUsage, ItemUsage, MoveUsage, NatureUsage, PokemonUsageSummary,
    };

    #[test]
    fn map_to_opponent_party_view_attaches_usage_by_recognized_name() {
        let result = OpponentPartyIdentificationResult {
            recognized_party: RecognizedParty {
                pokemons: vec![
                    RecognizedPokemon {
                        slot: SelectionSlot(3),
                        species_id: None,
                        display_name: Some("ピカチュウ".to_string()),
                        confidence: ConfidenceScore::High(0.97),
                        candidates: vec![],
                    },
                    RecognizedPokemon {
                        slot: SelectionSlot(1),
                        species_id: None,
                        display_name: None,
                        confidence: ConfidenceScore::Unknown,
                        candidates: vec![],
                    },
                ],
            },
            usage_summaries: vec![sample_usage("ピカチュウ")],
            conflicts: vec![],
        };

        let view = map_to_opponent_party_view(&result);

        assert_eq!(view.pokemons.len(), 2);
        assert_eq!(view.pokemons[0].slot_index, 1);
        assert!(view.pokemons[0].usage.is_none());
        assert_eq!(view.pokemons[1].slot_index, 3);
        assert_eq!(
            view.pokemons[1]
                .usage
                .as_ref()
                .map(|usage| usage.name.as_str()),
            Some("ピカチュウ")
        );
        assert_eq!(
            view.pokemons[1]
                .usage
                .as_ref()
                .and_then(|usage| usage.abilities.first())
                .map(|ability| ability.name.as_str()),
            Some("せいでんき")
        );
    }

    fn sample_usage(name: &str) -> PokemonUsageSummary {
        PokemonUsageSummary {
            id: name.to_string(),
            name: name.to_string(),
            types: vec!["でんき".to_string()],
            moves: vec![MoveUsage {
                name: "10まんボルト".to_string(),
                rate: "80%".to_string(),
            }],
            items: vec![ItemUsage {
                name: "きあいのタスキ".to_string(),
                rate: "35%".to_string(),
            }],
            abilities: vec![AbilityUsage {
                name: "せいでんき".to_string(),
                rate: "92%".to_string(),
            }],
            effort_values: vec![EffortValueUsage {
                h: 0,
                a: 0,
                b: 0,
                c: 252,
                d: 4,
                s: 252,
                rate: "52%".to_string(),
            }],
            natures: vec![NatureUsage {
                name: "おくびょう".to_string(),
                rate: "61%".to_string(),
            }],
        }
    }
}
