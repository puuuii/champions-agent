use std::sync::{Arc, Mutex};

use champions_interface::CapturedFrame;

#[derive(Clone)]
pub struct LatestFrame {
    inner: Arc<Mutex<Option<CapturedFrame>>>,
}

impl LatestFrame {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(None)),
        }
    }

    pub fn store(&self, frame: CapturedFrame) {
        let mut slot = self.inner.lock().unwrap();
        *slot = Some(frame);
    }

    pub fn take(&self) -> Option<CapturedFrame> {
        let mut slot = self.inner.lock().unwrap();
        slot.take()
    }

    pub fn peek(&self) -> Option<CapturedFrame> {
        let slot = self.inner.lock().unwrap();
        slot.clone()
    }
}

impl Default for LatestFrame {
    fn default() -> Self {
        Self::new()
    }
}
