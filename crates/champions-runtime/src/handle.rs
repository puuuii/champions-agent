use tokio::sync::mpsc;

use champions_interface::{PreviewFrame, RuntimeCommand, RuntimeEvent};

pub struct RuntimeHandle {
    command_tx: mpsc::Sender<RuntimeCommand>,
    event_rx: mpsc::Receiver<RuntimeEvent>,
    preview_rx: mpsc::Receiver<PreviewFrame>,
}

impl RuntimeHandle {
    pub(crate) fn new(
        command_tx: mpsc::Sender<RuntimeCommand>,
        event_rx: mpsc::Receiver<RuntimeEvent>,
        preview_rx: mpsc::Receiver<PreviewFrame>,
    ) -> Self {
        Self {
            command_tx,
            event_rx,
            preview_rx,
        }
    }

    pub async fn send(&self, command: RuntimeCommand) -> Result<(), RuntimeSendError> {
        self.command_tx
            .send(command)
            .await
            .map_err(|_| RuntimeSendError::RuntimeShutDown)
    }

    pub async fn next_event(&mut self) -> Option<RuntimeEvent> {
        self.event_rx.recv().await
    }

    pub async fn next_preview(&mut self) -> Option<PreviewFrame> {
        self.preview_rx.recv().await
    }

    pub fn try_next_preview(&mut self) -> Option<PreviewFrame> {
        self.preview_rx.try_recv().ok()
    }

    pub fn split(self) -> (CommandSender, EventReceiver, PreviewReceiver) {
        (
            CommandSender {
                command_tx: self.command_tx,
            },
            EventReceiver {
                event_rx: self.event_rx,
            },
            PreviewReceiver {
                preview_rx: self.preview_rx,
            },
        )
    }
}

#[derive(Clone)]
pub struct CommandSender {
    command_tx: mpsc::Sender<RuntimeCommand>,
}

impl CommandSender {
    pub async fn send(&self, command: RuntimeCommand) -> Result<(), RuntimeSendError> {
        self.command_tx
            .send(command)
            .await
            .map_err(|_| RuntimeSendError::RuntimeShutDown)
    }
}

pub struct EventReceiver {
    event_rx: mpsc::Receiver<RuntimeEvent>,
}

impl EventReceiver {
    pub async fn next(&mut self) -> Option<RuntimeEvent> {
        self.event_rx.recv().await
    }
}

pub struct PreviewReceiver {
    preview_rx: mpsc::Receiver<PreviewFrame>,
}

impl PreviewReceiver {
    pub async fn next(&mut self) -> Option<PreviewFrame> {
        self.preview_rx.recv().await
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeSendError {
    #[error("runtime has shut down")]
    RuntimeShutDown,
}
