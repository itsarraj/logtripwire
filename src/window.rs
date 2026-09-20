//! The rolling-window match counter. This is the pure decision logic —
//! no file I/O, no process spawning, no sleeping — so every branch is
//! directly unit-testable with manually constructed `Instant`s instead of
//! real wall-clock waits.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Counts how many matches landed within the trailing `window` and
/// reports when that count first *crosses* `threshold` — edge-triggered,
/// not level-triggered. That distinction matters: a log flooding with
/// matches well past the threshold should fire the configured action
/// once per flood, not once per line. It re-arms automatically once the
/// count drops back below the threshold (old matches age out of the
/// window) and then crosses again — so a second, later flood still
/// fires its own alert.
pub struct SlidingWindow {
    window: Duration,
    threshold: usize,
    timestamps: VecDeque<Instant>,
}

impl SlidingWindow {
    pub fn new(window: Duration, threshold: usize) -> Self {
        Self {
            window,
            threshold: threshold.max(1),
            timestamps: VecDeque::new(),
        }
    }

    /// Drops every recorded timestamp older than `window` relative to
    /// `now`. A timestamp exactly `window` old is still kept — the window
    /// is the closed interval `[now - window, now]`, so a threshold of 2
    /// with a 10s window genuinely fires for two matches 10.000s apart,
    /// not just for anything strictly inside 10s.
    fn prune(&mut self, now: Instant) {
        while let Some(&front) = self.timestamps.front() {
            if now.saturating_duration_since(front) > self.window {
                self.timestamps.pop_front();
            } else {
                break;
            }
        }
    }

    /// Records one match at `now`. Returns `true` exactly on the instant
    /// the in-window count reaches `threshold` for the first time since
    /// it last fell below it.
    pub fn record(&mut self, now: Instant) -> bool {
        self.prune(now);
        let was_below = self.timestamps.len() < self.threshold;
        self.timestamps.push_back(now);
        was_below && self.timestamps.len() >= self.threshold
    }

    /// The current in-window count, as of `now` (pruning first). Exposed
    /// mainly for tests and for a `--verbose` running-count display.
    pub fn count(&mut self, now: Instant) -> usize {
        self.prune(now);
        self.timestamps.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn below_threshold_never_fires() {
        let mut w = SlidingWindow::new(Duration::from_secs(10), 3);
        let t0 = Instant::now();
        assert!(!w.record(t0));
        assert!(!w.record(t0 + Duration::from_secs(1)));
        assert_eq!(w.count(t0 + Duration::from_secs(1)), 2);
    }

    #[test]
    fn reaching_threshold_fires_exactly_on_the_crossing_match() {
        let mut w = SlidingWindow::new(Duration::from_secs(10), 3);
        let t0 = Instant::now();
        assert!(!w.record(t0));
        assert!(!w.record(t0 + Duration::from_millis(100)));
        assert!(w.record(t0 + Duration::from_millis(200)));
    }

    #[test]
    fn threshold_of_one_fires_on_the_first_match() {
        let mut w = SlidingWindow::new(Duration::from_secs(10), 1);
        assert!(w.record(Instant::now()));
    }

    #[test]
    fn sustained_matches_past_threshold_do_not_refire_every_line() {
        let mut w = SlidingWindow::new(Duration::from_secs(10), 3);
        let t0 = Instant::now();
        assert!(!w.record(t0));
        assert!(!w.record(t0 + Duration::from_millis(100)));
        assert!(w.record(t0 + Duration::from_millis(200)));
        // Three more matches, all still well inside the window — none of
        // these should refire, since the count never drops below
        // threshold in between.
        assert!(!w.record(t0 + Duration::from_millis(300)));
        assert!(!w.record(t0 + Duration::from_millis(400)));
        assert!(!w.record(t0 + Duration::from_millis(500)));
    }

    #[test]
    fn matches_older_than_the_window_are_pruned_out_of_the_count() {
        let mut w = SlidingWindow::new(Duration::from_secs(1), 3);
        let t0 = Instant::now();
        w.record(t0);
        w.record(t0 + Duration::from_millis(100));
        // Both matches are now 5s in the past relative to `later` — well
        // outside the 1s window — so the count must be back to 0.
        let later = t0 + Duration::from_secs(5);
        assert_eq!(w.count(later), 0);
    }

    #[test]
    fn dropping_below_threshold_then_crossing_again_refires() {
        let mut w = SlidingWindow::new(Duration::from_millis(500), 2);
        let t0 = Instant::now();
        assert!(!w.record(t0));
        assert!(w.record(t0 + Duration::from_millis(100)));

        // Long enough gap that both earlier matches have aged out.
        let t1 = t0 + Duration::from_secs(2);
        assert_eq!(w.count(t1), 0);
        assert!(!w.record(t1));
        // Second flood, well after the window has re-armed.
        assert!(w.record(t1 + Duration::from_millis(50)));
    }

    #[test]
    fn a_match_exactly_at_the_window_boundary_still_counts() {
        let mut w = SlidingWindow::new(Duration::from_secs(10), 2);
        let t0 = Instant::now();
        assert!(!w.record(t0));
        // Exactly 10s later — the closed-interval boundary.
        assert!(w.record(t0 + Duration::from_secs(10)));
    }

    #[test]
    fn a_match_just_past_the_window_boundary_does_not_count_the_first_one() {
        let mut w = SlidingWindow::new(Duration::from_secs(10), 2);
        let t0 = Instant::now();
        w.record(t0);
        // 10.001s later: the first match has just aged out, so this
        // second one alone is only a count of 1, not 2.
        assert!(!w.record(t0 + Duration::from_millis(10_001)));
    }

    #[test]
    fn count_reflects_pruning_without_recording_a_new_match() {
        let mut w = SlidingWindow::new(Duration::from_millis(500), 5);
        let t0 = Instant::now();
        w.record(t0);
        w.record(t0 + Duration::from_millis(10));
        w.record(t0 + Duration::from_millis(20));
        assert_eq!(w.count(t0 + Duration::from_millis(20)), 3);
        assert_eq!(w.count(t0 + Duration::from_secs(1)), 0);
    }

    #[test]
    fn threshold_zero_is_treated_as_one() {
        // A threshold of 0 would otherwise fire before any match ever
        // arrives, which isn't a meaningful "N times" configuration —
        // clamped to 1 rather than allowed to misbehave silently.
        let mut w = SlidingWindow::new(Duration::from_secs(10), 0);
        assert!(w.record(Instant::now()));
    }
}
