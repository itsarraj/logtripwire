//! What happens once the sliding window fires.

use std::process::Command;

#[derive(Clone, Debug, clap::ValueEnum, PartialEq, Eq)]
pub enum ActionKind {
    /// Print an alert line to stdout and keep tailing.
    Alert,
    /// Run a configured shell command (`sh -c <command>`) and keep tailing.
    Command,
    /// Exit the whole process nonzero.
    Exit,
}

/// Builds the human-readable alert line — pulled out as a pure function
/// so its exact wording is unit-testable without printing anything.
pub fn format_alert(pattern: &str, count: usize, window_secs: f64, matched_line: &str) -> String {
    format!(
        "[logtripwire] ALERT: pattern '{pattern}' matched {count} time(s) within {window_secs:.1}s (last: {matched_line})"
    )
}

/// Runs `command` via `sh -c`, inheriting this process's stdio so the
/// child's own output shows up immediately in the terminal. Returns the
/// child's exit code (or `None` if it was killed by a signal instead of
/// exiting).
pub fn run_command(command: &str) -> std::io::Result<Option<i32>> {
    let status = Command::new("sh").arg("-c").arg(command).status()?;
    Ok(status.code())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_alert_includes_pattern_count_and_window() {
        let msg = format_alert("ERROR", 5, 30.0, "2026-09-20 ERROR disk full");
        assert!(msg.contains("ERROR"));
        assert!(msg.contains('5'));
        assert!(msg.contains("30.0s"));
        assert!(msg.contains("disk full"));
    }

    #[test]
    fn run_command_executes_a_real_shell_command_and_reports_its_exit_code() {
        let code = run_command("exit 0").unwrap();
        assert_eq!(code, Some(0));

        let code = run_command("exit 7").unwrap();
        assert_eq!(code, Some(7));
    }

    #[test]
    fn run_command_can_produce_a_real_side_effect() {
        let path = std::env::temp_dir().join(format!(
            "logtripwire-test-run-command-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        run_command(&format!("touch {}", path.display())).unwrap();
        assert!(path.exists());
        std::fs::remove_file(&path).ok();
    }
}
