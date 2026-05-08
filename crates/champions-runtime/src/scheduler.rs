use std::time::{Duration, Instant};

use champions_domain::recognition::ScreenState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulerState {
    Idle,
    MaybeSelectionScreen,
    SelectionScreenEntered,
    SelectionScreenStable,
    SelectionScreenExited,
}

pub struct RecognitionScheduler {
    state: SchedulerState,
    ocr_interval: Duration,
    confirm_timeout: Duration,
    exit_timeout: Duration,
    last_ocr_at: Option<Instant>,
    state_entered_at: Instant,
    consecutive_misses: u32,
    confirm_threshold: u32,
}

impl RecognitionScheduler {
    pub fn new() -> Self {
        Self {
            state: SchedulerState::Idle,
            ocr_interval: Duration::from_millis(500),
            confirm_timeout: Duration::from_millis(2000),
            exit_timeout: Duration::from_millis(3000),
            last_ocr_at: None,
            state_entered_at: Instant::now(),
            consecutive_misses: 0,
            confirm_threshold: 2,
        }
    }

    pub fn state(&self) -> SchedulerState {
        self.state
    }

    pub fn should_run_ocr(&self, now: Instant) -> bool {
        match self.state {
            SchedulerState::Idle | SchedulerState::MaybeSelectionScreen => match self.last_ocr_at {
                Some(last) => now.duration_since(last) >= self.ocr_interval,
                None => true,
            },
            SchedulerState::SelectionScreenStable | SchedulerState::SelectionScreenExited => {
                match self.last_ocr_at {
                    Some(last) => now.duration_since(last) >= self.exit_timeout,
                    None => true,
                }
            }
            SchedulerState::SelectionScreenEntered => false,
        }
    }

    pub fn should_run_identification(&self) -> bool {
        self.state == SchedulerState::SelectionScreenEntered
    }

    pub fn on_ocr_result(&mut self, screen_state: ScreenState, now: Instant) {
        self.last_ocr_at = Some(now);

        match (self.state, screen_state) {
            (SchedulerState::Idle, ScreenState::SelectionScreen) => {
                self.transition_to(SchedulerState::MaybeSelectionScreen, now);
                self.consecutive_misses = 0;
            }
            (SchedulerState::Idle, ScreenState::Other) => {}
            (SchedulerState::MaybeSelectionScreen, ScreenState::SelectionScreen) => {
                self.consecutive_misses = 0;
                if now.duration_since(self.state_entered_at) >= self.confirm_timeout
                    || self.confirm_threshold <= 1
                {
                    self.transition_to(SchedulerState::SelectionScreenEntered, now);
                }
            }
            (SchedulerState::MaybeSelectionScreen, ScreenState::Other) => {
                self.consecutive_misses += 1;
                if self.consecutive_misses >= self.confirm_threshold {
                    self.transition_to(SchedulerState::Idle, now);
                }
            }
            (SchedulerState::SelectionScreenStable, ScreenState::Other) => {
                self.consecutive_misses += 1;
                if self.consecutive_misses >= self.confirm_threshold {
                    self.transition_to(SchedulerState::SelectionScreenExited, now);
                }
            }
            (SchedulerState::SelectionScreenStable, ScreenState::SelectionScreen) => {
                self.consecutive_misses = 0;
            }
            (SchedulerState::SelectionScreenExited, ScreenState::Other) => {
                self.transition_to(SchedulerState::Idle, now);
            }
            (SchedulerState::SelectionScreenExited, ScreenState::SelectionScreen) => {
                self.transition_to(SchedulerState::SelectionScreenStable, now);
                self.consecutive_misses = 0;
            }
            _ => {}
        }
    }

    pub fn on_identification_complete(&mut self, now: Instant) {
        if self.state == SchedulerState::SelectionScreenEntered {
            self.transition_to(SchedulerState::SelectionScreenStable, now);
            self.consecutive_misses = 0;
        }
    }

    pub fn reset(&mut self) {
        self.state = SchedulerState::Idle;
        self.last_ocr_at = None;
        self.consecutive_misses = 0;
        self.state_entered_at = Instant::now();
    }

    fn transition_to(&mut self, new_state: SchedulerState, now: Instant) {
        self.state = new_state;
        self.state_entered_at = now;
    }
}

impl Default for RecognitionScheduler {
    fn default() -> Self {
        Self::new()
    }
}
