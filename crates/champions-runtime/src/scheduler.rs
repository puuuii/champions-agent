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
    identification_completed_for_current_selection: bool,
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
            identification_completed_for_current_selection: false,
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
            && !self.identification_completed_for_current_selection
    }

    pub fn on_ocr_result(&mut self, screen_state: ScreenState, now: Instant) {
        self.last_ocr_at = Some(now);

        match (self.state, screen_state) {
            (SchedulerState::Idle, ScreenState::SelectionScreen) => {
                self.transition_to(SchedulerState::MaybeSelectionScreen, now);
                self.consecutive_misses = 0;
                self.identification_completed_for_current_selection = false;
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
                self.identification_completed_for_current_selection = false;
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
            self.identification_completed_for_current_selection = true;
        }
    }

    pub fn force_selection_screen(&mut self, now: Instant) {
        self.last_ocr_at = Some(now);
        self.transition_to(SchedulerState::SelectionScreenEntered, now);
        self.consecutive_misses = 0;
        self.identification_completed_for_current_selection = false;
    }

    pub fn reset(&mut self) {
        self.state = SchedulerState::Idle;
        self.last_ocr_at = None;
        self.consecutive_misses = 0;
        self.state_entered_at = Instant::now();
        self.identification_completed_for_current_selection = false;
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

#[cfg(test)]
mod tests {
    use super::{RecognitionScheduler, SchedulerState};
    use champions_domain::recognition::ScreenState;
    use std::time::{Duration, Instant};

    #[test]
    fn identification_runs_only_once_per_selection_cycle() {
        let mut scheduler = RecognitionScheduler::new();
        let start = Instant::now();

        scheduler.on_ocr_result(ScreenState::SelectionScreen, start);
        scheduler.on_ocr_result(
            ScreenState::SelectionScreen,
            start + Duration::from_millis(2500),
        );

        assert_eq!(scheduler.state(), SchedulerState::SelectionScreenEntered);
        assert!(scheduler.should_run_identification());

        scheduler.on_identification_complete(start + Duration::from_millis(2600));
        assert_eq!(scheduler.state(), SchedulerState::SelectionScreenStable);
        assert!(!scheduler.should_run_identification());

        scheduler.on_ocr_result(
            ScreenState::SelectionScreen,
            start + Duration::from_millis(6000),
        );
        assert!(!scheduler.should_run_identification());

        scheduler.on_ocr_result(ScreenState::Other, start + Duration::from_millis(9000));
        scheduler.on_ocr_result(ScreenState::Other, start + Duration::from_millis(12000));
        scheduler.on_ocr_result(ScreenState::Other, start + Duration::from_millis(15000));
        assert_eq!(scheduler.state(), SchedulerState::Idle);

        scheduler.on_ocr_result(
            ScreenState::SelectionScreen,
            start + Duration::from_millis(16000),
        );
        scheduler.on_ocr_result(
            ScreenState::SelectionScreen,
            start + Duration::from_millis(18500),
        );
        assert_eq!(scheduler.state(), SchedulerState::SelectionScreenEntered);
        assert!(scheduler.should_run_identification());
    }
}
