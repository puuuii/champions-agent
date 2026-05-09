use std::sync::{Arc, Mutex};

use champions_interface::{CapturedFrame, PreviewFrame};

#[derive(Clone)]
pub struct Latest<T> {
    inner: Arc<Mutex<Option<T>>>,
}

impl<T> Latest<T> {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(None)),
        }
    }

    pub fn store(&self, value: T) {
        let mut slot = self.inner.lock().unwrap();
        *slot = Some(value);
    }

    pub fn take(&self) -> Option<T> {
        let mut slot = self.inner.lock().unwrap();
        slot.take()
    }
}

impl<T: Clone> Latest<T> {
    pub fn peek(&self) -> Option<T> {
        let slot = self.inner.lock().unwrap();
        slot.clone()
    }
}

impl<T> Default for Latest<T> {
    fn default() -> Self {
        Self::new()
    }
}

pub type LatestFrame = Latest<CapturedFrame>;
pub type LatestPreview = Latest<PreviewFrame>;
