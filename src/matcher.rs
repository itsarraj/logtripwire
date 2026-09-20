//! Compiles the user's `--pattern` into something that can be tested
//! against each tailed line.

use anyhow::{Context, Result};
use regex::Regex;

pub struct Matcher {
    re: Regex,
}

impl Matcher {
    /// Compiles `pattern` as a regex. If `literal` is set, every regex
    /// metacharacter in `pattern` is escaped first, so a user watching
    /// for a literal string like `[503]` doesn't need to know regex
    /// syntax or get a confusing "unterminated character class" error.
    /// `ignore_case` prepends the inline `(?i)` flag.
    pub fn new(pattern: &str, literal: bool, ignore_case: bool) -> Result<Self> {
        let body = if literal {
            regex::escape(pattern)
        } else {
            pattern.to_string()
        };
        let full = if ignore_case {
            format!("(?i){body}")
        } else {
            body
        };
        let re = Regex::new(&full).with_context(|| format!("invalid pattern '{pattern}'"))?;
        Ok(Self { re })
    }

    pub fn is_match(&self, line: &str) -> bool {
        self.re.is_match(line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_regex_matches() {
        let m = Matcher::new(r"ERROR|FATAL", false, false).unwrap();
        assert!(m.is_match("2026-09-20 ERROR disk full"));
        assert!(!m.is_match("2026-09-20 INFO all fine"));
    }

    #[test]
    fn invalid_regex_is_a_clean_error_not_a_panic() {
        let result = Matcher::new("[unterminated", false, false);
        assert!(result.is_err());
    }

    #[test]
    fn ignore_case_matches_regardless_of_case() {
        let m = Matcher::new("error", false, true).unwrap();
        assert!(m.is_match("ERROR: boom"));
        assert!(m.is_match("Error: boom"));
    }

    #[test]
    fn case_sensitive_by_default() {
        let m = Matcher::new("ERROR", false, false).unwrap();
        assert!(!m.is_match("error: boom"));
        assert!(m.is_match("ERROR: boom"));
    }

    #[test]
    fn literal_mode_escapes_regex_metacharacters() {
        // `[503]` would be an invalid/misleading character class if used
        // as a raw regex; in literal mode it must match the bracket text
        // verbatim instead.
        let m = Matcher::new("[503]", true, false).unwrap();
        assert!(m.is_match("upstream returned [503] bad gateway"));
        assert!(!m.is_match("upstream returned 5 or 0 or 3"));
    }

    #[test]
    fn literal_mode_can_be_combined_with_ignore_case() {
        let m = Matcher::new("Connection Refused", true, true).unwrap();
        assert!(m.is_match("connection refused by peer"));
    }
}
