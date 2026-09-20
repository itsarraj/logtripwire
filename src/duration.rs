//! Parses human-friendly duration strings (`"30s"`, `"5m"`, `"200ms"`) for
//! the `--window`/`--poll-interval` flags.

use anyhow::{anyhow, Result};
use std::time::Duration;

/// A number followed by (case-insensitive) `ms`, `s`, `m`, or `h`; a bare
/// number is seconds. Fractional values are allowed (`"1.5s"`).
pub fn parse_duration(input: &str) -> Result<Duration> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("empty duration string"));
    }

    let lower = trimmed.to_ascii_lowercase();
    let (digits, unit_seconds) = if let Some(d) = lower.strip_suffix("ms") {
        (d, 0.001)
    } else if let Some(d) = lower.strip_suffix('h') {
        (d, 3600.0)
    } else if let Some(d) = lower.strip_suffix('m') {
        (d, 60.0)
    } else if let Some(d) = lower.strip_suffix('s') {
        (d, 1.0)
    } else {
        (lower.as_str(), 1.0)
    };

    let digits = digits.trim();
    if digits.is_empty() {
        return Err(anyhow!("'{input}' has a unit but no number"));
    }
    let value: f64 = digits
        .parse()
        .map_err(|_| anyhow!("'{input}' is not a valid duration (expected e.g. 30s, 5m, 200ms)"))?;
    if !value.is_finite() || value < 0.0 {
        return Err(anyhow!("duration '{input}' must be a non-negative number"));
    }
    Ok(Duration::from_secs_f64(value * unit_seconds))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_milliseconds() {
        assert_eq!(parse_duration("200ms").unwrap(), Duration::from_millis(200));
    }

    #[test]
    fn parses_seconds() {
        assert_eq!(parse_duration("30s").unwrap(), Duration::from_secs(30));
    }

    #[test]
    fn parses_minutes() {
        assert_eq!(parse_duration("5m").unwrap(), Duration::from_secs(300));
    }

    #[test]
    fn bare_number_is_seconds() {
        assert_eq!(parse_duration("10").unwrap(), Duration::from_secs(10));
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_duration("banana").is_err());
    }

    #[test]
    fn rejects_negative() {
        assert!(parse_duration("-5s").is_err());
    }
}
