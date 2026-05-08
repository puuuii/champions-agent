use tokio::sync::watch;

#[derive(Clone)]
pub struct ShutdownToken {
    receiver: watch::Receiver<bool>,
}

impl ShutdownToken {
    pub fn is_shutdown(&self) -> bool {
        *self.receiver.borrow()
    }

    pub async fn wait(&mut self) {
        while !*self.receiver.borrow() {
            if self.receiver.changed().await.is_err() {
                return;
            }
        }
    }
}

pub struct ShutdownSignal {
    sender: watch::Sender<bool>,
}

impl ShutdownSignal {
    pub fn trigger(&self) {
        let _ = self.sender.send(true);
    }
}

pub fn shutdown_pair() -> (ShutdownSignal, ShutdownToken) {
    let (sender, receiver) = watch::channel(false);
    (ShutdownSignal { sender }, ShutdownToken { receiver })
}
