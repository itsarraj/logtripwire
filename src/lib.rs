pub mod action;
pub mod duration;
pub mod matcher;
pub mod tail;
pub mod window;

use std::time::{Duration, Instant};

use matcher::Matcher;
use window::SlidingWindow;

/// Ties a compiled pattern to its rolling window: every tailed line is
/// fed through `check`, which returns `Some(line)` only on the exact
/// line that pushes the in-window match count across the threshold.
pub struct Tripwire {
    matcher: Matcher,
    window: SlidingWindow,
    threshold: usize,
    window_duration: Duration,
}

impl Tripwire {
    pub fn new(matcher: Matcher, window_duration: Duration, threshold: usize) -> Self {
        Self {
            matcher,
            window: SlidingWindow::new(window_duration, threshold),
            threshold,
            window_duration,
        }
    }

    pub fn threshold(&self) -> usize {
        self.threshold
    }

    pub fn window_duration(&self) -> Duration {
        self.window_duration
    }

    /// Feeds one line through the matcher and, if it matches, the
    /// sliding window. Returns `true` exactly when this line is the one
    /// that crossed the threshold.
    pub fn check(&mut self, line: &str, now: Instant) -> bool {
        if !self.matcher.is_match(line) {
            return false;
        }
        self.window.record(now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_matching_lines_never_advance_the_window() {
        let matcher = Matcher::new("ERROR", false, false).unwrap();
        let mut tw = Tripwire::new(matcher, Duration::from_secs(10), 2);
        let t0 = Instant::now();
        assert!(!tw.check("all is fine", t0));
        assert!(!tw.check("still fine", t0 + Duration::from_millis(1)));
    }

    #[test]
    fn matching_lines_fire_exactly_at_the_threshold() {
        let matcher = Matcher::new("ERROR", false, false).unwrap();
        let mut tw = Tripwire::new(matcher, Duration::from_secs(10), 3);
        let t0 = Instant::now();
        assert!(!tw.check("ERROR one", t0));
        assert!(!tw.check("noise", t0 + Duration::from_millis(50)));
        assert!(!tw.check("ERROR two", t0 + Duration::from_millis(100)));
        assert!(tw.check("ERROR three", t0 + Duration::from_millis(150)));
    }

    #[test]
    fn matches_spread_out_slower_than_the_window_never_fire() {
        // Same three matches as the test above, but each one lands after
        // the previous has already aged out of a short window — this is
        // the exact "slow drip never trips the wire" scenario the live
        // test also exercises against the real binary.
        let matcher = Matcher::new("ERROR", false, false).unwrap();
        let mut tw = Tripwire::new(matcher, Duration::from_millis(500), 3);
        let t0 = Instant::now();
        assert!(!tw.check("ERROR one", t0));
        assert!(!tw.check("ERROR two", t0 + Duration::from_secs(1)));
        assert!(!tw.check("ERROR three", t0 + Duration::from_secs(2)));
    }
}
