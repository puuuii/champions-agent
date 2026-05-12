use champions_interface::{RuntimeCommand, RuntimeEvent};
use champions_runtime::{CommandSender, EventReceiver, PreviewFrame, PreviewReceiver};
use iced::Subscription;
use iced::futures::SinkExt;
use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub enum RuntimeMessage {
    PreviewFrameReceived(PreviewFrame),
    RuntimeEventReceived(RuntimeEvent),
}

static PREVIEW_RECEIVER: OnceLock<Arc<Mutex<PreviewReceiver>>> = OnceLock::new();
static EVENT_RECEIVER: OnceLock<Arc<Mutex<EventReceiver>>> = OnceLock::new();
static COMMAND_SENDER: OnceLock<CommandSender> = OnceLock::new();

pub fn init_runtime(
    command_sender: CommandSender,
    preview: Arc<Mutex<PreviewReceiver>>,
    event: Arc<Mutex<EventReceiver>>,
) {
    let _ = COMMAND_SENDER.set(command_sender);
    let _ = PREVIEW_RECEIVER.set(preview);
    let _ = EVENT_RECEIVER.set(event);
    tracing::info!("runtime subscriptions initialized");
}

pub async fn send_command(command: RuntimeCommand) -> Result<(), String> {
    let Some(sender) = COMMAND_SENDER.get().cloned() else {
        tracing::error!("runtime command sender is not initialized");
        return Err("runtime command sender is not initialized".to_string());
    };

    tracing::debug!(?command, "sending runtime command");
    sender
        .send(command)
        .await
        .map_err(|error| error.to_string())
}

pub fn preview_subscription() -> Subscription<RuntimeMessage> {
    Subscription::run(|| {
        iced::stream::channel(
            2,
            |mut output: iced::futures::channel::mpsc::Sender<RuntimeMessage>| async move {
                let receiver = PREVIEW_RECEIVER
                    .get()
                    .expect("preview receiver not initialized")
                    .clone();
                loop {
                    let frame = {
                        let mut rx = receiver.lock().await;
                        rx.next().await
                    };
                    match frame {
                        Some(f) => {
                            let _ = output.send(RuntimeMessage::PreviewFrameReceived(f)).await;
                        }
                        None => break,
                    }
                }
            },
        )
    })
}

pub fn event_subscription() -> Subscription<RuntimeMessage> {
    Subscription::run(|| {
        iced::stream::channel(
            64,
            |mut output: iced::futures::channel::mpsc::Sender<RuntimeMessage>| async move {
                let receiver = EVENT_RECEIVER
                    .get()
                    .expect("event receiver not initialized")
                    .clone();
                loop {
                    let event = {
                        let mut rx = receiver.lock().await;
                        rx.next().await
                    };
                    match event {
                        Some(e) => {
                            let _ = output.send(RuntimeMessage::RuntimeEventReceived(e)).await;
                        }
                        None => break,
                    }
                }
            },
        )
    })
}
