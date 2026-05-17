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
    EventSequence, FrameSequence, ItemUsageView, MatchPhase, MoveUsageView, NatureUsageView,
    OpponentPartyView, PokemonUsageSummaryView, RecognitionAttemptId, RecognizedPokemonView,
    RuntimeCommand, RuntimeEvent,
};

use crate::frame::CapturedFrame;
use crate::handle::RuntimeHandle;
use crate::latest::{LatestFrame, LatestPreview};
use crate::recognition::RecognitionPort;
use crate::scheduler::{RecognitionScheduler, SchedulerState};
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
        tracing::info!(
            preview_max_width = self.preview_max_width,
            preview_target_fps = self.preview_target_fps,
            recognition_enabled = self.recognition_port.is_some(),
            "building runtime handle and workers",
        );
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
    #[tracing::instrument(
        name = "runtime_workers",
        skip(self),
        fields(
            preview_max_width = self.preview_max_width,
            preview_target_fps = self.preview_target_fps,
            has_recognition_worker = self.recognition_port.is_some()
        )
    )]
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
        let has_recognition_worker = recognition_port.is_some();
        tracing::info!("runtime workers starting");

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
            has_recognition_worker,
        )
        .await;

        let _ = capture_worker.await;
        let _ = preview_worker.await;
        if let Some(recognition_worker) = recognition_worker {
            let _ = recognition_worker.await;
        }
        tracing::info!("runtime workers stopped");
    }
}

const RECOGNITION_TICK_INTERVAL: Duration = Duration::from_millis(100);
const IDLE_POLL_INTERVAL: Duration = Duration::from_millis(20);
const BATTLE_RESULT_OCR_INTERVAL: Duration = Duration::from_millis(500);

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
    manual_scan_generation: AtomicU64,
    manual_phase_generation: AtomicU64,
    manual_phase_value: AtomicU8,
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
            manual_scan_generation: AtomicU64::new(0),
            manual_phase_generation: AtomicU64::new(0),
            manual_phase_value: AtomicU8::new(match_phase_to_u8(MatchPhase::Other)),
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
        Duration::from_secs_f64(1.0 / self.preview_target_fps.load(Ordering::Relaxed) as f64)
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

    fn request_manual_scan(&self) {
        self.manual_scan_generation.fetch_add(1, Ordering::Relaxed);
    }

    fn manual_scan_generation(&self) -> u64 {
        self.manual_scan_generation.load(Ordering::Relaxed)
    }

    fn set_manual_phase(&self, phase: MatchPhase) {
        self.manual_phase_value
            .store(match_phase_to_u8(phase), Ordering::Relaxed);
        self.manual_phase_generation.fetch_add(1, Ordering::Relaxed);
    }

    fn manual_phase_generation(&self) -> u64 {
        self.manual_phase_generation.load(Ordering::Relaxed)
    }

    fn manual_phase(&self) -> MatchPhase {
        match_phase_from_u8(self.manual_phase_value.load(Ordering::Relaxed))
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
        let _span = tracing::info_span!("capture_worker").entered();
        tracing::info!("capture worker started");
        while !self.shutdown_token.is_shutdown() {
            if !self.control.is_capturing() {
                std::thread::sleep(IDLE_POLL_INTERVAL);
                continue;
            }

            let tick_started = Instant::now();

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
                    tracing::error!(%error, "capture error");
                    blocking_send_event(&self.event_tx, &self.event_seq, |event_sequence| {
                        RuntimeEvent::Error {
                            event_sequence,
                            error: map_capture_error(&error),
                        }
                    });
                }
            }

            let sleep_for = self
                .control
                .preview_interval()
                .saturating_sub(tick_started.elapsed());

            if !sleep_for.is_zero() {
                std::thread::sleep(sleep_for);
            }
        }
        tracing::info!("capture worker stopped");
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
        let _span = tracing::info_span!("preview_worker").entered();
        tracing::info!("preview worker started");
        let mut last_previewed_frame_seq: Option<FrameSequence> = None;

        while !self.shutdown_token.is_shutdown() {
            if !self.control.is_capturing() || !self.control.is_preview_enabled() {
                std::thread::sleep(IDLE_POLL_INTERVAL);
                continue;
            }

            let tick_started = Instant::now();

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

            let sleep_for = self
                .control
                .preview_interval()
                .saturating_sub(tick_started.elapsed());

            if !sleep_for.is_zero() {
                std::thread::sleep(sleep_for);
            }
        }
        tracing::info!("preview worker stopped");
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
        let _span = tracing::info_span!("recognition_worker").entered();
        tracing::info!("recognition worker started");
        let mut scheduler = RecognitionScheduler::new();
        let mut battle_result_tracker = BattleResultPhaseTracker::default();
        let mut match_phase_tracker = MatchPhaseTracker::default();
        let mut attempt_id = 0_u64;
        let mut recognition_generation = self.control.recognition_generation();
        let mut manual_scan_generation = 0_u64;
        let mut manual_phase_generation = 0_u64;

        while !self.shutdown_token.is_shutdown() {
            let next_generation = self.control.recognition_generation();
            if next_generation != recognition_generation {
                scheduler.reset();
                battle_result_tracker.reset();
                tracing::info!(
                    previous_generation = recognition_generation,
                    next_generation,
                    "recognition generation updated; scheduler reset",
                );
                if let Some(phase) = match_phase_tracker.reset() {
                    send_match_phase_changed(&self.event_tx, &self.event_seq, phase);
                }
                recognition_generation = next_generation;
            }

            self.apply_manual_phase_request(
                &mut manual_phase_generation,
                &mut battle_result_tracker,
                &mut match_phase_tracker,
            );
            self.apply_manual_scan_request(
                &mut manual_scan_generation,
                &mut scheduler,
                &mut battle_result_tracker,
                &mut match_phase_tracker,
                &mut attempt_id,
            );

            if !self.control.is_capturing() || !self.control.is_recognition_enabled() {
                std::thread::sleep(IDLE_POLL_INTERVAL);
                continue;
            }

            self.run_tick(
                &mut scheduler,
                &mut battle_result_tracker,
                &mut match_phase_tracker,
                &mut attempt_id,
            );
            std::thread::sleep(RECOGNITION_TICK_INTERVAL);
        }
        tracing::info!("recognition worker stopped");
    }

    fn apply_manual_phase_request(
        &self,
        manual_phase_generation: &mut u64,
        battle_result_tracker: &mut BattleResultPhaseTracker,
        match_phase_tracker: &mut MatchPhaseTracker,
    ) {
        let next_generation = self.control.manual_phase_generation();
        if next_generation == *manual_phase_generation {
            return;
        }

        *manual_phase_generation = next_generation;
        let phase = self.control.manual_phase();
        tracing::info!(
            ?phase,
            generation = next_generation,
            "manual phase request applied"
        );
        battle_result_tracker.set_phase_hint(phase);

        if let Some(phase) = match_phase_tracker.force_phase(phase) {
            send_match_phase_changed(&self.event_tx, &self.event_seq, phase);
        }
    }

    fn apply_manual_scan_request(
        &self,
        manual_scan_generation: &mut u64,
        scheduler: &mut RecognitionScheduler,
        battle_result_tracker: &mut BattleResultPhaseTracker,
        match_phase_tracker: &mut MatchPhaseTracker,
        attempt_id: &mut u64,
    ) {
        let next_generation = self.control.manual_scan_generation();
        if next_generation == *manual_scan_generation {
            return;
        }

        *manual_scan_generation = next_generation;
        tracing::info!(
            generation = next_generation,
            "manual selection scan requested"
        );
        self.run_manual_selection_scan(
            scheduler,
            battle_result_tracker,
            match_phase_tracker,
            attempt_id,
        );
    }

    fn run_manual_selection_scan(
        &self,
        scheduler: &mut RecognitionScheduler,
        battle_result_tracker: &mut BattleResultPhaseTracker,
        match_phase_tracker: &mut MatchPhaseTracker,
        attempt_id: &mut u64,
    ) {
        battle_result_tracker.set_phase_hint(MatchPhase::PokemonSelection);
        if let Some(phase) = match_phase_tracker.force_phase(MatchPhase::PokemonSelection) {
            send_match_phase_changed(&self.event_tx, &self.event_seq, phase);
        }

        let Some(frame) = self.latest_frame.peek() else {
            tracing::debug!("manual selection scan skipped because no frame is available");
            return;
        };

        let now = Instant::now();
        scheduler.force_selection_screen(now);
        self.identify_party_from_frame(&frame, scheduler, attempt_id, now);
    }

    fn run_tick(
        &self,
        scheduler: &mut RecognitionScheduler,
        battle_result_tracker: &mut BattleResultPhaseTracker,
        match_phase_tracker: &mut MatchPhaseTracker,
        attempt_id: &mut u64,
    ) {
        let now = Instant::now();
        let should_run_selection_ocr = scheduler.should_run_ocr(now);
        let should_run_battle_result_ocr = battle_result_tracker.should_run(now);
        let mut cached_frame = None;

        if should_run_selection_ocr || should_run_battle_result_ocr {
            let frame = match self.latest_frame.peek() {
                Some(frame) => frame,
                None => return,
            };
            cached_frame = Some(frame.clone());

            if should_run_selection_ocr {
                let ocr_image = self.recognition_port.extract_target_text_image(
                    frame.image.width,
                    frame.image.height,
                    &frame.image.bytes,
                );

                match self.recognition_port.detect_selection_screen(ocr_image) {
                    Ok(result) => {
                        let previous_state = scheduler.state();
                        scheduler.on_ocr_result(result.screen_state, now);
                        let current_state = scheduler.state();

                        if current_state != previous_state {
                            tracing::debug!(
                                previous_state = ?previous_state,
                                current_state = ?current_state,
                                raw_text = ?result.raw_text,
                                "selection scheduler transitioned",
                            );
                        }

                        if let Some(phase) = match_phase_tracker
                            .on_scheduler_transition(previous_state, current_state)
                        {
                            send_match_phase_changed(&self.event_tx, &self.event_seq, phase);
                        }
                    }
                    Err(error) => {
                        tracing::warn!(%error, "selection OCR failed");
                        blocking_send_event(&self.event_tx, &self.event_seq, |event_sequence| {
                            RuntimeEvent::Error {
                                event_sequence,
                                error: champions_interface::RuntimeError::RecognitionFailed(error),
                            }
                        });
                    }
                }
            }

            if should_run_battle_result_ocr {
                battle_result_tracker.record_check(now);

                let ocr_image = self.recognition_port.extract_battle_result_text_image(
                    frame.image.width,
                    frame.image.height,
                    &frame.image.bytes,
                );

                match self.recognition_port.detect_battle_result_phase(ocr_image) {
                    Ok(is_battle_result_phase) => {
                        if battle_result_tracker.update(is_battle_result_phase) {
                            if let Some(phase) =
                                match_phase_tracker.on_battle_result(is_battle_result_phase)
                            {
                                send_match_phase_changed(&self.event_tx, &self.event_seq, phase);
                            }
                        }
                    }
                    Err(error) => {
                        tracing::warn!(%error, "battle result OCR failed");
                    }
                }
            }
        }

        if scheduler.should_run_identification()
            && match_phase_tracker.phase() == MatchPhase::PokemonSelection
        {
            let frame = match cached_frame.or_else(|| self.latest_frame.peek()) {
                Some(frame) => frame,
                None => return,
            };
            self.identify_party_from_frame(&frame, scheduler, attempt_id, now);
        }
    }

    fn identify_party_from_frame(
        &self,
        frame: &CapturedFrame,
        scheduler: &mut RecognitionScheduler,
        attempt_id: &mut u64,
        now: Instant,
    ) {
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
                    attempt_id = *attempt_id,
                    pokemon_count = result.recognized_party.pokemons.len(),
                    conflict_count = result.conflicts.len(),
                    "opponent party identified",
                );
            }
            Err(error) => {
                tracing::error!(%error, "party identification failed");
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

#[derive(Debug, Default)]
struct BattleResultPhaseTracker {
    last_ocr_at: Option<Instant>,
    is_battle_result_phase: bool,
}

impl BattleResultPhaseTracker {
    fn should_run(&self, now: Instant) -> bool {
        match self.last_ocr_at {
            Some(last) => now.duration_since(last) >= BATTLE_RESULT_OCR_INTERVAL,
            None => true,
        }
    }

    fn record_check(&mut self, now: Instant) {
        self.last_ocr_at = Some(now);
    }

    fn update(&mut self, is_battle_result_phase: bool) -> bool {
        if self.is_battle_result_phase == is_battle_result_phase {
            return false;
        }

        self.is_battle_result_phase = is_battle_result_phase;
        true
    }

    fn set_phase_hint(&mut self, phase: MatchPhase) {
        self.last_ocr_at = None;
        self.is_battle_result_phase = phase == MatchPhase::BattleResult;
    }

    fn reset(&mut self) {
        self.last_ocr_at = None;
        self.is_battle_result_phase = false;
    }
}

#[derive(Debug)]
struct MatchPhaseTracker {
    phase: MatchPhase,
}

impl MatchPhaseTracker {
    fn phase(&self) -> MatchPhase {
        self.phase
    }

    fn on_scheduler_transition(
        &mut self,
        previous_state: SchedulerState,
        current_state: SchedulerState,
    ) -> Option<MatchPhase> {
        if previous_state == current_state {
            return None;
        }

        let inferred_phase = match current_state {
            SchedulerState::SelectionScreenEntered | SchedulerState::SelectionScreenStable => {
                Some(MatchPhase::PokemonSelection)
            }
            SchedulerState::SelectionScreenExited
                if matches!(
                    previous_state,
                    SchedulerState::SelectionScreenEntered | SchedulerState::SelectionScreenStable
                ) =>
            {
                Some(MatchPhase::Battle)
            }
            _ => None,
        };

        inferred_phase.and_then(|phase| self.apply_inferred_phase(phase))
    }

    fn on_battle_result(&mut self, is_battle_result_phase: bool) -> Option<MatchPhase> {
        if is_battle_result_phase {
            self.apply_inferred_phase(MatchPhase::BattleResult)
        } else if self.phase == MatchPhase::BattleResult {
            self.apply_inferred_phase(MatchPhase::Other)
        } else {
            None
        }
    }

    fn force_phase(&mut self, new_phase: MatchPhase) -> Option<MatchPhase> {
        self.update(new_phase)
    }

    fn reset(&mut self) -> Option<MatchPhase> {
        self.update(MatchPhase::Other)
    }

    fn apply_inferred_phase(&mut self, inferred_phase: MatchPhase) -> Option<MatchPhase> {
        if self.phase == inferred_phase {
            return None;
        }

        if next_match_phase(self.phase) != inferred_phase {
            return None;
        }

        self.update(inferred_phase)
    }

    fn update(&mut self, new_phase: MatchPhase) -> Option<MatchPhase> {
        if self.phase == new_phase {
            return None;
        }

        self.phase = new_phase;
        Some(new_phase)
    }
}

impl Default for MatchPhaseTracker {
    fn default() -> Self {
        Self {
            phase: MatchPhase::Other,
        }
    }
}

fn next_match_phase(current_phase: MatchPhase) -> MatchPhase {
    match current_phase {
        MatchPhase::Other => MatchPhase::PokemonSelection,
        MatchPhase::PokemonSelection => MatchPhase::Battle,
        MatchPhase::Battle => MatchPhase::BattleResult,
        MatchPhase::BattleResult => MatchPhase::Other,
    }
}

#[tracing::instrument(skip_all, fields(has_recognition_worker))]
async fn run_command_loop(
    command_rx: &mut mpsc::Receiver<RuntimeCommand>,
    event_tx: &mpsc::Sender<RuntimeEvent>,
    shutdown_signal: &ShutdownSignal,
    control: &RuntimeControl,
    event_seq: &EventSequencer,
    has_recognition_worker: bool,
) {
    tracing::info!("runtime command loop started");
    while let Some(command) = command_rx.recv().await {
        tracing::debug!(?command, "runtime command received");
        match command {
            RuntimeCommand::Shutdown => {
                tracing::info!("runtime shutdown requested");
                shutdown_signal.trigger();
                send_event(event_tx, event_seq, |event_sequence| {
                    RuntimeEvent::RuntimeStopped { event_sequence }
                })
                .await;
                return;
            }
            RuntimeCommand::StartCapture => {
                tracing::info!("capture started");
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
                tracing::info!("capture stopped");
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
                tracing::info!("recognition started");
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
                tracing::info!("recognition stopped");
                control.set_recognition_enabled(false);
                send_event(event_tx, event_seq, |event_sequence| {
                    RuntimeEvent::RecognitionStatusChanged {
                        event_sequence,
                        status: champions_interface::RecognitionStatus::Stopped,
                    }
                })
                .await;
            }
            RuntimeCommand::StartRecognition => {
                tracing::warn!(
                    "start recognition ignored because recognition worker is unavailable"
                );
            }
            RuntimeCommand::ScanOpponentSelection if has_recognition_worker => {
                tracing::info!("manual opponent selection scan queued");
                control.request_manual_scan();
            }
            RuntimeCommand::ScanOpponentSelection => {
                tracing::warn!(
                    "manual opponent selection scan ignored because recognition worker is unavailable"
                );
            }
            RuntimeCommand::SetMatchPhase(phase) if has_recognition_worker => {
                tracing::info!(?phase, "manual match phase queued");
                control.set_manual_phase(phase);
            }
            RuntimeCommand::SetMatchPhase(phase) => {
                tracing::warn!(
                    ?phase,
                    "manual match phase ignored because recognition worker is unavailable",
                );
            }
            RuntimeCommand::SetPreviewEnabled(enabled) => {
                tracing::debug!(enabled, "preview enabled updated");
                control.set_preview_enabled(enabled);
            }
            RuntimeCommand::SetPreviewMaxWidth(preview_max_width) => {
                tracing::debug!(preview_max_width, "preview max width updated");
                control.set_preview_max_width(preview_max_width);
            }
            RuntimeCommand::SetPreviewTargetFps(preview_target_fps) => {
                tracing::debug!(preview_target_fps, "preview target fps updated");
                control.set_preview_target_fps(preview_target_fps);
            }
        }
    }

    tracing::warn!("runtime command channel closed; shutting down");
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

fn send_match_phase_changed(
    event_tx: &mpsc::Sender<RuntimeEvent>,
    event_seq: &EventSequencer,
    phase: MatchPhase,
) {
    tracing::info!(?phase, "match phase changed");
    blocking_send_event(event_tx, event_seq, |event_sequence| {
        RuntimeEvent::MatchPhaseChanged {
            event_sequence,
            phase,
        }
    });
}

fn match_phase_to_u8(phase: MatchPhase) -> u8 {
    match phase {
        MatchPhase::Other => 0,
        MatchPhase::PokemonSelection => 1,
        MatchPhase::Battle => 2,
        MatchPhase::BattleResult => 3,
    }
}

fn match_phase_from_u8(raw: u8) -> MatchPhase {
    match raw {
        1 => MatchPhase::PokemonSelection,
        2 => MatchPhase::Battle,
        3 => MatchPhase::BattleResult,
        _ => MatchPhase::Other,
    }
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
    use super::{MatchPhaseTracker, map_to_opponent_party_view};
    use crate::scheduler::SchedulerState;
    use champions_application::use_cases::OpponentPartyIdentificationResult;
    use champions_domain::recognition::{
        ConfidenceScore, RecognizedParty, RecognizedPokemon, SelectionSlot,
    };
    use champions_domain::usage::{
        AbilityUsage, EffortValueUsage, ItemUsage, MoveUsage, NatureUsage, PokemonUsageSummary,
    };
    use champions_interface::MatchPhase;

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

    #[test]
    fn match_phase_tracker_transitions_from_selection_to_battle() {
        let mut tracker = MatchPhaseTracker::default();

        assert_eq!(
            tracker.on_scheduler_transition(
                SchedulerState::Idle,
                SchedulerState::SelectionScreenEntered,
            ),
            Some(MatchPhase::PokemonSelection)
        );
        assert_eq!(tracker.phase(), MatchPhase::PokemonSelection);
        assert_eq!(
            tracker.on_scheduler_transition(
                SchedulerState::SelectionScreenEntered,
                SchedulerState::SelectionScreenStable,
            ),
            None
        );
        assert_eq!(
            tracker.on_scheduler_transition(
                SchedulerState::SelectionScreenStable,
                SchedulerState::SelectionScreenExited,
            ),
            Some(MatchPhase::Battle)
        );
        assert_eq!(tracker.phase(), MatchPhase::Battle);
    }

    #[test]
    fn match_phase_tracker_transitions_from_battle_to_result_and_other() {
        let mut tracker = MatchPhaseTracker::default();

        tracker
            .on_scheduler_transition(SchedulerState::Idle, SchedulerState::SelectionScreenEntered);
        tracker.on_scheduler_transition(
            SchedulerState::SelectionScreenStable,
            SchedulerState::SelectionScreenExited,
        );

        assert_eq!(tracker.phase(), MatchPhase::Battle);
        assert_eq!(
            tracker.on_battle_result(true),
            Some(MatchPhase::BattleResult)
        );
        assert_eq!(tracker.phase(), MatchPhase::BattleResult);
        assert_eq!(tracker.on_battle_result(false), Some(MatchPhase::Other));
        assert_eq!(tracker.phase(), MatchPhase::Other);
    }

    #[test]
    fn match_phase_tracker_rejects_direct_transition_to_battle_result() {
        let mut tracker = MatchPhaseTracker::default();

        assert_eq!(tracker.on_battle_result(true), None);
        assert_eq!(tracker.phase(), MatchPhase::Other);
    }

    #[test]
    fn match_phase_tracker_rejects_direct_transition_from_battle_to_selection() {
        let mut tracker = MatchPhaseTracker::default();

        tracker.force_phase(MatchPhase::Battle);

        assert_eq!(
            tracker.on_scheduler_transition(
                SchedulerState::Idle,
                SchedulerState::SelectionScreenEntered,
            ),
            None
        );
        assert_eq!(tracker.phase(), MatchPhase::Battle);
    }

    #[test]
    fn match_phase_tracker_force_phase_allows_manual_recovery() {
        let mut tracker = MatchPhaseTracker::default();

        assert_eq!(
            tracker.force_phase(MatchPhase::Battle),
            Some(MatchPhase::Battle)
        );
        assert_eq!(tracker.phase(), MatchPhase::Battle);
        assert_eq!(
            tracker.on_battle_result(true),
            Some(MatchPhase::BattleResult)
        );
        assert_eq!(tracker.phase(), MatchPhase::BattleResult);
    }

    fn sample_usage(name: &str) -> PokemonUsageSummary {
        PokemonUsageSummary {
            pokemon_id: 25,
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
