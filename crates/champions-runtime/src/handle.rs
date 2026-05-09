use tokio::sync::mpsc;

use champions_interface::{PreviewFrame, RuntimeCommand, RuntimeEvent};

use crate::latest::LatestPreview;

pub struct RuntimeHandle {
    command_tx: mpsc::Sender<RuntimeCommand>,
    event_rx: mpsc::Receiver<RuntimeEvent>,
    preview_rx: PreviewReceiver,
}

impl RuntimeHandle {
    pub(crate) fn new(
        command_tx: mpsc::Sender<RuntimeCommand>,
        event_rx: mpsc::Receiver<RuntimeEvent>,
        preview_notify_rx: mpsc::Receiver<()>,
        latest_preview: LatestPreview,
    ) -> Self {
        Self {
            command_tx,
            event_rx,
            preview_rx: PreviewReceiver::new(preview_notify_rx, latest_preview),
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
        self.preview_rx.next().await
    }

    pub fn try_next_preview(&mut self) -> Option<PreviewFrame> {
        self.preview_rx.try_next()
    }

    pub fn split(self) -> (CommandSender, EventReceiver, PreviewReceiver) {
        (
            CommandSender {
                command_tx: self.command_tx,
            },
            EventReceiver {
                event_rx: self.event_rx,
            },
            self.preview_rx,
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
    preview_notify_rx: mpsc::Receiver<()>,
    latest_preview: LatestPreview,
}

impl PreviewReceiver {
    pub(crate) fn new(
        preview_notify_rx: mpsc::Receiver<()>,
        latest_preview: LatestPreview,
    ) -> Self {
        Self {
            preview_notify_rx,
            latest_preview,
        }
    }

    pub async fn next(&mut self) -> Option<PreviewFrame> {
        loop {
            self.preview_notify_rx.recv().await?;
            self.drain_pending_notifications();

            if let Some(frame) = self.latest_preview.take() {
                return Some(frame);
            }
        }
    }

    pub fn try_next(&mut self) -> Option<PreviewFrame> {
        if self.preview_notify_rx.try_recv().is_err() {
            return None;
        }

        self.drain_pending_notifications();
        self.latest_preview.take()
    }

    fn drain_pending_notifications(&mut self) {
        while self.preview_notify_rx.try_recv().is_ok() {}
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeSendError {
    #[error("runtime has shut down")]
    RuntimeShutDown,
}
