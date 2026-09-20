use std::path::PathBuf;
use std::process::ExitCode;
use std::thread::sleep;
use std::time::Instant;

use clap::Parser;
use logtripwire::action::{format_alert, run_command, ActionKind};
use logtripwire::duration::parse_duration;
use logtripwire::matcher::Matcher;
use logtripwire::tail::TailSource;
use logtripwire::Tripwire;

#[derive(Parser)]
#[command(
    name = "logtripwire",
    about = "Tails a log file and fires an action when a pattern recurs N times within a rolling window"
)]
struct Cli {
    /// Log file to tail.
    file: PathBuf,
    /// Pattern to watch for (a regex, unless --literal is given).
    pattern: String,
    /// How many matches within the window trigger the action.
    #[arg(short = 'c', long, default_value_t = 3)]
    threshold: usize,
    /// Rolling window size, e.g. 30s, 5m, 1h.
    #[arg(short = 'w', long, default_value = "1m")]
    window: String,
    /// Treat PATTERN as a literal string instead of a regex.
    #[arg(long)]
    literal: bool,
    /// Case-insensitive match.
    #[arg(short = 'i', long)]
    ignore_case: bool,
    /// What to do when the threshold is crossed.
    #[arg(long, value_enum, default_value_t = ActionKind::Alert)]
    action: ActionKind,
    /// Shell command to run when --action command is used.
    #[arg(long)]
    command: Option<String>,
    /// Replay the file's existing content first instead of only watching
    /// for new lines.
    #[arg(long)]
    from_start: bool,
    /// How often to check the file for new lines.
    #[arg(long, default_value = "200ms")]
    poll_interval: String,
}

fn main() -> anyhow::Result<ExitCode> {
    let cli = Cli::parse();

    if cli.action == ActionKind::Command && cli.command.is_none() {
        anyhow::bail!("--action command requires --command \"<shell command>\"");
    }

    let window = parse_duration(&cli.window)?;
    let poll_interval = parse_duration(&cli.poll_interval)?;
    let matcher = Matcher::new(&cli.pattern, cli.literal, cli.ignore_case)?;
    let mut tripwire = Tripwire::new(matcher, window, cli.threshold);

    let mut source = if cli.from_start {
        TailSource::open_at_start(&cli.file)
    } else {
        TailSource::open_at_end(&cli.file)
    }
    .map_err(|e| anyhow::anyhow!("opening {}: {e}", cli.file.display()))?;

    eprintln!(
        "logtripwire: watching {} for '{}' ({} match(es) within {:.0}s)",
        cli.file.display(),
        cli.pattern,
        cli.threshold,
        window.as_secs_f64()
    );

    loop {
        let lines = source
            .poll()
            .map_err(|e| anyhow::anyhow!("reading {}: {e}", cli.file.display()))?;
        for line in lines {
            let now = Instant::now();
            if tripwire.check(&line, now) {
                let message = format_alert(
                    &cli.pattern,
                    tripwire.threshold(),
                    tripwire.window_duration().as_secs_f64(),
                    &line,
                );
                match cli.action {
                    ActionKind::Alert => println!("{message}"),
                    ActionKind::Command => {
                        println!("{message}");
                        let command = cli.command.as_ref().expect("checked above");
                        run_command(command)
                            .map_err(|e| anyhow::anyhow!("running command: {e}"))?;
                    }
                    ActionKind::Exit => {
                        println!("{message}");
                        return Ok(ExitCode::from(1));
                    }
                }
            }
        }
        sleep(poll_interval);
    }
}
