mod fake_frame_source;

use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use champions_application::{
    OcrImage, PartyImageSet, SelectionDetectionResult, use_cases::OpponentPartyIdentificationResult,
};
use champions_domain::recognition::{RecognizedParty, ScreenState};
use champions_interface::{CaptureStatus, FrameSequence, RuntimeCommand, RuntimeEvent};
use champions_runtime::{RecognitionPort, RuntimeBuilder};
use fake_frame_source::{FakeFrameSource, FakePreviewConverter};

#[tokio::test]
async fn shutdown_sends_runtime_stopped() {
    let source = FakeFrameSource::new(640, 480, 1000);
    let converter = FakePreviewConverter;

    let (mut handle, workers) = RuntimeBuilder::new()
        .frame_source(Box::new(source))
        .preview_converter(Box::new(converter))
        .build();

    let worker_task = tokio::spawn(async move {
        workers.run().await;
    });

    handle.send(RuntimeCommand::Shutdown).await.unwrap();

    let event = handle.next_event().await;
    assert!(matches!(event, Some(RuntimeEvent::RuntimeStopped { .. })));

    worker_task.await.unwrap();
}

#[tokio::test]
async fn capture_produces_preview_frames() {
    let source = FakeFrameSource::new(1920, 1080, 1000);
    let converter = FakePreviewConverter;

    let (mut handle, workers) = RuntimeBuilder::new()
        .frame_source(Box::new(source))
        .preview_converter(Box::new(converter))
        .preview_target_fps(60)
        .build();

    let worker_task = tokio::spawn(async move {
        workers.run().await;
    });

    handle.send(RuntimeCommand::StartCapture).await.unwrap();

    let event = handle.next_event().await.unwrap();
    assert!(matches!(
        event,
        RuntimeEvent::CaptureStatusChanged {
            status: CaptureStatus::Running,
            ..
        }
    ));

    let preview = tokio::time::timeout(Duration::from_secs(2), handle.next_preview())
        .await
        .expect("timed out waiting for preview")
        .expect("preview channel closed");

    assert!(preview.width <= 960);
    assert_eq!(
        preview.rgba.len(),
        (preview.width * preview.height * 4) as usize
    );

    handle.send(RuntimeCommand::Shutdown).await.unwrap();

    let mut got_stopped = false;
    while let Some(event) = handle.next_event().await {
        if matches!(event, RuntimeEvent::RuntimeStopped { .. }) {
            got_stopped = true;
            break;
        }
    }
    assert!(got_stopped);

    worker_task.await.unwrap();
}

#[tokio::test]
async fn stop_capture_sends_status_event() {
    let source = FakeFrameSource::new(640, 480, 1000);
    let converter = FakePreviewConverter;

    let (mut handle, workers) = RuntimeBuilder::new()
        .frame_source(Box::new(source))
        .preview_converter(Box::new(converter))
        .build();

    let worker_task = tokio::spawn(async move {
        workers.run().await;
    });

    handle.send(RuntimeCommand::StartCapture).await.unwrap();
    let _ = handle.next_event().await;

    handle.send(RuntimeCommand::StopCapture).await.unwrap();

    let event = handle.next_event().await.unwrap();
    assert!(matches!(
        event,
        RuntimeEvent::CaptureStatusChanged {
            status: CaptureStatus::Stopped,
            ..
        }
    ));

    handle.send(RuntimeCommand::Shutdown).await.unwrap();
    worker_task.await.unwrap();
}

#[tokio::test]
async fn preview_backpressure_keeps_latest_frame() {
    let source = FakeFrameSource::new(640, 480, 10);
    let converter = FakePreviewConverter;

    let (mut handle, workers) = RuntimeBuilder::new()
        .frame_source(Box::new(source))
        .preview_converter(Box::new(converter))
        .preview_target_fps(100)
        .build();

    let worker_task = tokio::spawn(async move {
        workers.run().await;
    });

    handle.send(RuntimeCommand::StartCapture).await.unwrap();
    let _ = handle.next_event().await;

    tokio::time::sleep(Duration::from_millis(250)).await;

    let preview = tokio::time::timeout(Duration::from_secs(1), handle.next_preview())
        .await
        .expect("timed out waiting for preview")
        .expect("preview channel closed");

    assert_eq!(preview.frame_sequence, FrameSequence(10));

    handle.send(RuntimeCommand::Shutdown).await.unwrap();
    worker_task.await.unwrap();
}

#[tokio::test]
async fn recognition_continues_when_preview_is_disabled() {
    let source = FakeFrameSource::new(640, 480, 1000);
    let converter = FakePreviewConverter;
    let detect_calls = Arc::new(AtomicUsize::new(0));

    let (mut handle, workers) = RuntimeBuilder::new()
        .frame_source(Box::new(source))
        .preview_converter(Box::new(converter))
        .recognition_port(Box::new(ProbeRecognitionPort::new(
            detect_calls.clone(),
            Duration::ZERO,
        )))
        .build();

    let worker_task = tokio::spawn(async move {
        workers.run().await;
    });

    handle
        .send(RuntimeCommand::SetPreviewEnabled(false))
        .await
        .unwrap();
    handle.send(RuntimeCommand::StartCapture).await.unwrap();
    handle.send(RuntimeCommand::StartRecognition).await.unwrap();

    assert_capture_status(&mut handle, CaptureStatus::Running).await;
    assert_recognition_running(&mut handle).await;

    wait_for_detect_calls(&detect_calls, 1).await;
    assert!(detect_calls.load(Ordering::Relaxed) > 0);

    handle.send(RuntimeCommand::Shutdown).await.unwrap();
    assert_runtime_stopped(&mut handle).await;
    worker_task.await.unwrap();
}

#[tokio::test]
async fn shutdown_stays_responsive_during_blocking_recognition() {
    let source = FakeFrameSource::new(640, 480, 1000);
    let converter = FakePreviewConverter;
    let detect_calls = Arc::new(AtomicUsize::new(0));

    let (mut handle, workers) = RuntimeBuilder::new()
        .frame_source(Box::new(source))
        .preview_converter(Box::new(converter))
        .recognition_port(Box::new(ProbeRecognitionPort::new(
            detect_calls.clone(),
            Duration::from_millis(400),
        )))
        .build();

    let worker_task = tokio::spawn(async move {
        workers.run().await;
    });

    handle.send(RuntimeCommand::StartCapture).await.unwrap();
    handle.send(RuntimeCommand::StartRecognition).await.unwrap();

    assert_capture_status(&mut handle, CaptureStatus::Running).await;
    assert_recognition_running(&mut handle).await;
    wait_for_detect_calls(&detect_calls, 1).await;

    handle.send(RuntimeCommand::Shutdown).await.unwrap();

    let event = tokio::time::timeout(Duration::from_millis(150), handle.next_event())
        .await
        .expect("timed out waiting for runtime stop")
        .expect("event channel closed");
    assert!(matches!(event, RuntimeEvent::RuntimeStopped { .. }));

    worker_task.await.unwrap();
}

async fn assert_capture_status(
    handle: &mut champions_runtime::RuntimeHandle,
    expected_status: CaptureStatus,
) {
    let event = handle.next_event().await.unwrap();
    assert!(matches!(
        event,
        RuntimeEvent::CaptureStatusChanged {
            status,
            ..
        } if status == expected_status
    ));
}

async fn assert_recognition_running(handle: &mut champions_runtime::RuntimeHandle) {
    let event = handle.next_event().await.unwrap();
    assert!(matches!(
        event,
        RuntimeEvent::RecognitionStatusChanged {
            status: champions_interface::RecognitionStatus::Running,
            ..
        }
    ));
}

async fn assert_runtime_stopped(handle: &mut champions_runtime::RuntimeHandle) {
    let event = tokio::time::timeout(Duration::from_secs(2), handle.next_event())
        .await
        .expect("timed out waiting for runtime stop")
        .expect("event channel closed");
    assert!(matches!(event, RuntimeEvent::RuntimeStopped { .. }));
}

async fn wait_for_detect_calls(detect_calls: &Arc<AtomicUsize>, minimum_calls: usize) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    while detect_calls.load(Ordering::Relaxed) < minimum_calls {
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for detection calls"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

struct ProbeRecognitionPort {
    detect_calls: Arc<AtomicUsize>,
    block_for: Duration,
}

impl ProbeRecognitionPort {
    fn new(detect_calls: Arc<AtomicUsize>, block_for: Duration) -> Self {
        Self {
            detect_calls,
            block_for,
        }
    }
}

impl RecognitionPort for ProbeRecognitionPort {
    fn detect_selection_screen(
        &self,
        _image: OcrImage,
    ) -> Result<SelectionDetectionResult, String> {
        self.detect_calls.fetch_add(1, Ordering::Relaxed);
        if !self.block_for.is_zero() {
            std::thread::sleep(self.block_for);
        }

        Ok(SelectionDetectionResult {
            raw_text: String::new(),
            screen_state: ScreenState::Other,
        })
    }

    fn identify_opponent_party(
        &self,
        _images: PartyImageSet,
    ) -> Result<OpponentPartyIdentificationResult, String> {
        Ok(OpponentPartyIdentificationResult {
            recognized_party: RecognizedParty {
                pokemons: Vec::new(),
            },
            usage_summaries: Vec::new(),
            conflicts: Vec::new(),
        })
    }

    fn extract_target_text_image(
        &self,
        _frame_width: u32,
        _frame_height: u32,
        _frame_bytes: &[u8],
    ) -> OcrImage {
        OcrImage {
            width: 1,
            height: 1,
            rgb_bytes: vec![0, 0, 0],
        }
    }

    fn extract_party_slots(
        &self,
        _frame_width: u32,
        _frame_height: u32,
        _frame_bytes: &[u8],
    ) -> PartyImageSet {
        PartyImageSet { slots: Vec::new() }
    }
}
