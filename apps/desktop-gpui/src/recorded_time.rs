use std::time::{Duration, Instant};

pub const AUTO_STOP_COUNTDOWN: Duration = Duration::from_secs(60);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecordedClock {
    started_at: Instant,
    paused_total: Duration,
    paused_since: Option<Instant>,
}

impl RecordedClock {
    pub fn start(now: Instant) -> Self {
        Self {
            started_at: now,
            paused_total: Duration::ZERO,
            paused_since: None,
        }
    }

    pub fn pause(&mut self, now: Instant) {
        if self.paused_since.is_none() {
            self.paused_since = Some(now);
        }
    }

    pub fn resume(&mut self, now: Instant) {
        if let Some(since) = self.paused_since.take() {
            self.paused_total += now.saturating_duration_since(since);
        }
    }

    pub fn settle(&mut self, paused: bool, requested: Instant, confirmed: Instant) {
        if paused {
            self.pause(requested);
        } else {
            self.resume(confirmed);
        }
    }

    pub fn recorded_at(&self, now: Instant) -> Duration {
        let paused_now = self
            .paused_since
            .map(|since| now.saturating_duration_since(since))
            .unwrap_or_default();
        now.saturating_duration_since(self.started_at)
            .saturating_sub(self.paused_total + paused_now)
    }
}

pub fn auto_stop_limit(minutes: u32) -> Option<Duration> {
    (minutes > 0).then_some(Duration::from_secs(u64::from(minutes) * 60))
}

pub fn auto_stop_time_left(limit: Option<Duration>, recorded: Duration) -> Option<Duration> {
    limit.map(|limit| limit.saturating_sub(recorded))
}

pub fn auto_stop_countdown(limit: Option<Duration>, recorded: Duration) -> Option<Duration> {
    auto_stop_time_left(limit, recorded).filter(|left| *left <= AUTO_STOP_COUNTDOWN)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secs(seconds: u64) -> Duration {
        Duration::from_secs(seconds)
    }

    #[test]
    fn recorded_time_counts_wall_time_while_recording() {
        let start = Instant::now();
        let clock = RecordedClock::start(start);
        assert_eq!(clock.recorded_at(start), Duration::ZERO);
        assert_eq!(clock.recorded_at(start + secs(42)), secs(42));
    }

    #[test]
    fn paused_stretches_do_not_count_as_recorded_time() {
        let start = Instant::now();
        let mut clock = RecordedClock::start(start);
        clock.pause(start + secs(10));
        assert_eq!(clock.recorded_at(start + secs(10)), secs(10));
        assert_eq!(clock.recorded_at(start + secs(70)), secs(10));
        clock.resume(start + secs(70));
        assert_eq!(clock.recorded_at(start + secs(75)), secs(15));
        clock.pause(start + secs(80));
        clock.resume(start + secs(200));
        assert_eq!(clock.recorded_at(start + secs(205)), secs(25));
    }

    #[test]
    fn a_pause_stops_counting_when_asked_and_a_resume_counts_from_the_recorder_confirming() {
        let start = Instant::now();
        let mut clock = RecordedClock::start(start);
        clock.settle(
            true,
            start + secs(10),
            start + Duration::from_millis(11_500),
        );
        assert_eq!(
            clock.recorded_at(start + Duration::from_millis(11_500)),
            secs(10)
        );
        clock.settle(
            false,
            start + secs(20),
            start + Duration::from_millis(20_400),
        );
        assert_eq!(
            clock.recorded_at(start + Duration::from_millis(25_400)),
            secs(15)
        );
    }

    #[test]
    fn a_settled_request_that_changed_nothing_leaves_the_clock_alone() {
        let start = Instant::now();
        let mut clock = RecordedClock::start(start);
        clock.settle(false, start + secs(5), start + secs(6));
        assert_eq!(clock.recorded_at(start + secs(10)), secs(10));
        clock.settle(true, start + secs(10), start + secs(11));
        clock.settle(true, start + secs(30), start + secs(31));
        assert_eq!(clock.recorded_at(start + secs(40)), secs(10));
    }

    #[test]
    fn repeated_pause_or_resume_calls_change_nothing() {
        let start = Instant::now();
        let mut clock = RecordedClock::start(start);
        clock.resume(start + secs(5));
        clock.pause(start + secs(10));
        clock.pause(start + secs(30));
        clock.resume(start + secs(40));
        clock.resume(start + secs(50));
        assert_eq!(clock.recorded_at(start + secs(60)), secs(30));
    }

    #[test]
    fn auto_stop_is_off_at_zero_minutes() {
        assert_eq!(auto_stop_limit(0), None);
        assert_eq!(auto_stop_limit(5), Some(secs(300)));
        assert_eq!(auto_stop_limit(30), Some(secs(1_800)));
    }

    #[test]
    fn time_left_runs_out_at_the_limit_and_never_goes_negative() {
        let limit = Some(secs(300));
        assert_eq!(auto_stop_time_left(None, secs(10_000)), None);
        assert_eq!(auto_stop_time_left(limit, secs(120)), Some(secs(180)));
        assert_eq!(auto_stop_time_left(limit, secs(300)), Some(Duration::ZERO));
        assert_eq!(auto_stop_time_left(limit, secs(301)), Some(Duration::ZERO));
    }

    #[test]
    fn a_take_paused_past_the_limit_in_wall_time_is_not_stopped() {
        let start = Instant::now();
        let mut clock = RecordedClock::start(start);
        clock.pause(start + secs(200));
        let recorded = clock.recorded_at(start + secs(900));
        assert_eq!(
            auto_stop_time_left(auto_stop_limit(5), recorded),
            Some(secs(100))
        );
    }

    #[test]
    fn the_bar_shows_time_left_only_in_the_last_minute() {
        let limit = Some(secs(300));
        assert_eq!(auto_stop_countdown(None, secs(299)), None);
        assert_eq!(auto_stop_countdown(limit, secs(239)), None);
        assert_eq!(auto_stop_countdown(limit, secs(240)), Some(secs(60)));
        assert_eq!(
            auto_stop_countdown(limit, Duration::from_millis(290_500)),
            Some(Duration::from_millis(9_500))
        );
        assert_eq!(auto_stop_countdown(limit, secs(300)), Some(Duration::ZERO));
    }
}
