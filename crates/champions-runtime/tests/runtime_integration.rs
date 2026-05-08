mod fake_frame_source;

use champions_interface::{CaptureStatus, RuntimeCommand, RuntimeEvent};
use champions_runtime::RuntimeBuilder;
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
        .preview_target_fps(30)
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

    let preview = tokio::time::timeout(std::time::Duration::from_secs(2), handle.next_preview())
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
