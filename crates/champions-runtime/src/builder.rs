use std::{collections::HashMap, time::Instant};

use tokio::sync::mpsc;

use champions_interface::{
    CandidateView, ConfidenceView, ConflictView, EffortValueUsageView, EventSequence,
    FrameSequence, ItemUsageView, MoveUsageView, NatureUsageView, OpponentPartyView,
    PokemonUsageSummaryView, PreviewFrame, RecognitionAttemptId, RecognizedPokemonView,
    RuntimeCommand, RuntimeEvent,
};

use crate::handle::RuntimeHandle;
use crate::latest::LatestFrame;
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
        let (preview_tx, preview_rx) = mpsc::channel(PREVIEW_CHANNEL_SIZE);
        let (shutdown_signal, shutdown_token) = shutdown_pair();

        let latest_frame = LatestFrame::new();

        let handle = RuntimeHandle::new(command_tx, event_rx, preview_rx);

        let workers = RuntimeWorkers {
            frame_source,
            preview_converter,
            recognition_port: self.recognition_port,
            command_rx,
            event_tx,
            preview_tx,
            shutdown_signal,
            shutdown_token,
            latest_frame,
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
    preview_tx: mpsc::Sender<PreviewFrame>,
    shutdown_signal: ShutdownSignal,
    #[allow(dead_code)]
    shutdown_token: ShutdownToken,
    latest_frame: LatestFrame,
    preview_max_width: u32,
    preview_target_fps: u8,
}

impl RuntimeWorkers {
    pub async fn run(mut self) {
        let mut frame_seq: u64 = 0;
        let mut event_seq: u64 = 0;
        let mut capturing = false;
        let mut preview_enabled = true;
        let mut recognition_enabled = false;
        let mut recognition_attempt_id: u64 = 0;
        let mut scheduler = RecognitionScheduler::new();

        let preview_interval =
            tokio::time::Duration::from_millis(1000 / self.preview_target_fps.max(1) as u64);
        let mut preview_timer = tokio::time::interval(preview_interval);
        preview_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        let recognition_interval = tokio::time::Duration::from_millis(100);
        let mut recognition_timer = tokio::time::interval(recognition_interval);
        recognition_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                Some(cmd) = self.command_rx.recv() => {
                    match cmd {
                        RuntimeCommand::Shutdown => {
                            self.shutdown_signal.trigger();
                            event_seq += 1;
                            let _ = self.event_tx.send(RuntimeEvent::RuntimeStopped {
                                event_sequence: EventSequence(event_seq),
                            }).await;
                            return;
                        }
                        RuntimeCommand::StartCapture => {
                            capturing = true;
                            event_seq += 1;
                            let _ = self.event_tx.send(RuntimeEvent::CaptureStatusChanged {
                                event_sequence: EventSequence(event_seq),
                                status: champions_interface::CaptureStatus::Running,
                            }).await;
                        }
                        RuntimeCommand::StopCapture => {
                            capturing = false;
                            event_seq += 1;
                            let _ = self.event_tx.send(RuntimeEvent::CaptureStatusChanged {
                                event_sequence: EventSequence(event_seq),
                                status: champions_interface::CaptureStatus::Stopped,
                            }).await;
                        }
                        RuntimeCommand::StartRecognition => {
                            if self.recognition_port.is_some() {
                                recognition_enabled = true;
                                scheduler.reset();
                                event_seq += 1;
                                let _ = self.event_tx.send(RuntimeEvent::RecognitionStatusChanged {
                                    event_sequence: EventSequence(event_seq),
                                    status: champions_interface::RecognitionStatus::Running,
                                }).await;
                            }
                        }
                        RuntimeCommand::StopRecognition => {
                            recognition_enabled = false;
                            scheduler.reset();
                            event_seq += 1;
                            let _ = self.event_tx.send(RuntimeEvent::RecognitionStatusChanged {
                                event_sequence: EventSequence(event_seq),
                                status: champions_interface::RecognitionStatus::Stopped,
                            }).await;
                        }
                        RuntimeCommand::SetPreviewEnabled(enabled) => {
                            preview_enabled = enabled;
                        }
                        RuntimeCommand::SetPreviewMaxWidth(w) => {
                            self.preview_max_width = w;
                        }
                        RuntimeCommand::SetPreviewTargetFps(fps) => {
                            self.preview_target_fps = fps;
                            let interval = tokio::time::Duration::from_millis(
                                1000 / fps.max(1) as u64
                            );
                            preview_timer = tokio::time::interval(interval);
                            preview_timer.set_missed_tick_behavior(
                                tokio::time::MissedTickBehavior::Skip,
                            );
                        }
                        _ => {}
                    }
                }
                _ = preview_timer.tick(), if capturing && preview_enabled => {
                    if let Err(e) = self.capture_and_preview(&mut frame_seq, &mut event_seq).await {
                        tracing::error!("capture error: {e}");
                        event_seq += 1;
                        let _ = self.event_tx.send(RuntimeEvent::Error {
                            event_sequence: EventSequence(event_seq),
                            error: match &e {
                                CaptureError::DeviceNotFound => {
                                    champions_interface::RuntimeError::CaptureDeviceNotFound
                                }
                                CaptureError::ReadFailed(msg) => {
                                    champions_interface::RuntimeError::CaptureReadFailed(msg.clone())
                                }
                            },
                        }).await;
                    }
                }
                _ = recognition_timer.tick(), if capturing && recognition_enabled && self.recognition_port.is_some() => {
                    self.run_recognition_tick(
                        &mut scheduler,
                        &mut event_seq,
                        &mut recognition_attempt_id,
                        frame_seq,
                    ).await;
                }
                else => {
                    tokio::task::yield_now().await;
                }
            }
        }
    }

    async fn capture_and_preview(
        &mut self,
        frame_seq: &mut u64,
        _event_seq: &mut u64,
    ) -> Result<(), CaptureError> {
        if let Some(frame) = self.frame_source.read_frame()? {
            *frame_seq += 1;
            let frame = champions_interface::CapturedFrame {
                frame_sequence: FrameSequence(*frame_seq),
                ..frame
            };

            self.latest_frame.store(frame.clone());

            let preview = self
                .preview_converter
                .convert(&frame, self.preview_max_width);
            let _ = self.preview_tx.try_send(preview);
        }
        Ok(())
    }

    async fn run_recognition_tick(
        &mut self,
        scheduler: &mut RecognitionScheduler,
        event_seq: &mut u64,
        attempt_id: &mut u64,
        current_frame_seq: u64,
    ) {
        let now = Instant::now();
        let port = match &self.recognition_port {
            Some(p) => p,
            None => return,
        };

        if scheduler.should_run_ocr(now) {
            let frame = match self.latest_frame.peek() {
                Some(f) => f,
                None => return,
            };

            let ocr_image = port.extract_target_text_image(
                frame.image.width,
                frame.image.height,
                &frame.image.bytes,
            );

            match port.detect_selection_screen(ocr_image) {
                Ok(result) => {
                    let prev_state = scheduler.state();
                    scheduler.on_ocr_result(result.screen_state, now);

                    if scheduler.state() != prev_state {
                        tracing::debug!(
                            "scheduler state: {:?} -> {:?} (text: {:?})",
                            prev_state,
                            scheduler.state(),
                            result.raw_text
                        );
                    }
                }
                Err(e) => {
                    tracing::warn!("OCR failed: {e}");
                    *event_seq += 1;
                    let _ = self
                        .event_tx
                        .send(RuntimeEvent::Error {
                            event_sequence: EventSequence(*event_seq),
                            error: champions_interface::RuntimeError::RecognitionFailed(e),
                        })
                        .await;
                }
            }
        }

        if scheduler.should_run_identification() {
            let frame = match self.latest_frame.peek() {
                Some(f) => f,
                None => return,
            };

            let party_images =
                port.extract_party_slots(frame.image.width, frame.image.height, &frame.image.bytes);

            match port.identify_opponent_party(party_images) {
                Ok(result) => {
                    *attempt_id += 1;
                    scheduler.on_identification_complete(now);

                    let party_view = map_to_opponent_party_view(&result);

                    *event_seq += 1;
                    let _ = self
                        .event_tx
                        .send(RuntimeEvent::OpponentPartyRecognized {
                            event_sequence: EventSequence(*event_seq),
                            frame_sequence: FrameSequence(current_frame_seq),
                            attempt_id: RecognitionAttemptId(*attempt_id),
                            party: party_view,
                        })
                        .await;

                    tracing::info!(
                        "opponent party identified (attempt {}): {} pokemon, {} conflicts",
                        attempt_id,
                        result.recognized_party.pokemons.len(),
                        result.conflicts.len()
                    );
                }
                Err(e) => {
                    tracing::error!("party identification failed: {e}");
                    *event_seq += 1;
                    let _ = self
                        .event_tx
                        .send(RuntimeEvent::Error {
                            event_sequence: EventSequence(*event_seq),
                            error: champions_interface::RuntimeError::RecognitionFailed(e),
                        })
                        .await;
                }
            }
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
        EffortValueUsage, ItemUsage, MoveUsage, NatureUsage, PokemonUsageSummary,
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
