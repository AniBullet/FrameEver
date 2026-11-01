use std::{sync::Mutex, time::Instant};

const AUDIO_SLACK: f64 = 0.06;

#[derive(Debug, Clone, Copy)]
struct State {
    position: f64,
    at: Instant,
    running: bool,
    slack: f64,
}

impl State {
    fn position_at(&self, now: Instant) -> f64 {
        if !self.running {
            return self.position;
        }
        let elapsed = now.saturating_duration_since(self.at).as_secs_f64();
        self.position + elapsed.min(self.slack)
    }
}

#[derive(Debug)]
pub struct Clock {
    state: Mutex<State>,
}

impl Clock {
    pub fn new(position: f64, now: Instant) -> Self {
        Self {
            state: Mutex::new(State {
                position,
                at: now,
                running: false,
                slack: f64::INFINITY,
            }),
        }
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        super::lock(&self.state)
    }

    pub fn position(&self) -> f64 {
        self.position_at(Instant::now())
    }

    pub fn position_at(&self, now: Instant) -> f64 {
        self.state().position_at(now)
    }

    pub fn set_running(&self, running: bool, now: Instant) {
        let mut state = self.state();
        state.position = state.position_at(now);
        state.at = now;
        state.running = running;
    }

    pub fn follow_audio(&self, now: Instant) {
        let mut state = self.state();
        state.position = state.position_at(now);
        state.at = now;
        state.slack = AUDIO_SLACK;
    }

    pub fn follow_wall(&self, now: Instant) {
        let mut state = self.state();
        state.position = state.position_at(now);
        state.at = now;
        state.slack = f64::INFINITY;
    }

    pub fn place(&self, position: f64, now: Instant) {
        let mut state = self.state();
        state.position = position;
        state.at = now;
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn after(start: Instant, millis: u64) -> Instant {
        start + Duration::from_millis(millis)
    }

    #[test]
    fn a_stopped_clock_holds_its_position() {
        let start = Instant::now();
        let clock = Clock::new(3., start);
        assert_eq!(clock.position_at(after(start, 500)), 3.);
    }

    #[test]
    fn a_running_clock_follows_the_wall_until_paused() {
        let start = Instant::now();
        let clock = Clock::new(1., start);
        clock.set_running(true, start);
        assert!((clock.position_at(after(start, 500)) - 1.5).abs() < 1e-9);
        clock.set_running(false, after(start, 500));
        assert!((clock.position_at(after(start, 900)) - 1.5).abs() < 1e-9);
    }

    #[test]
    fn audio_ticks_set_the_position_and_it_runs_on_between_them() {
        let start = Instant::now();
        let clock = Clock::new(0., start);
        clock.follow_audio(start);
        clock.set_running(true, start);
        clock.place(2., after(start, 100));
        assert!((clock.position_at(after(start, 110)) - 2.01).abs() < 1e-9);
    }

    #[test]
    fn a_silent_audio_device_stops_the_clock_within_the_slack() {
        let start = Instant::now();
        let clock = Clock::new(0., start);
        clock.follow_audio(start);
        clock.set_running(true, start);
        clock.place(2., start);
        assert!((clock.position_at(after(start, 5000)) - (2. + AUDIO_SLACK)).abs() < 1e-9);
    }

    #[test]
    fn falling_back_to_the_wall_keeps_the_position_and_runs_free() {
        let start = Instant::now();
        let clock = Clock::new(0., start);
        clock.follow_audio(start);
        clock.set_running(true, start);
        clock.place(2., start);
        clock.follow_wall(after(start, 10));
        assert!((clock.position_at(after(start, 1010)) - 3.01).abs() < 1e-9);
    }

    #[test]
    fn placing_a_running_clock_does_not_stop_it() {
        let start = Instant::now();
        let clock = Clock::new(0., start);
        clock.set_running(true, start);
        clock.place(10., after(start, 100));
        assert!((clock.position_at(after(start, 200)) - 10.1).abs() < 1e-9);
    }
}
