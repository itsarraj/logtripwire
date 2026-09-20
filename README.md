# logtripwire

Tails a log file and fires an alert, a shell command, or a nonzero exit
when a pattern recurs N times within a rolling time window — `swatch`/
`logwatch` (Perl) for simple on-call-style alerting when a full SIEM is
overkill.

## Usage

```bash
logtripwire app.log "ERROR" -c 5 -w 1m                       # print an alert
logtripwire app.log "OOM" -c 1 -w 1s --action exit            # exit 1 on the first hit
logtripwire app.log "5xx" -c 20 -w 30s --action command \
    --command "curl -X POST https://hooks.example.com/alert"  # fire a webhook
logtripwire app.log "timeout" --literal -i                    # literal string match, case-insensitive
```

Only watches for lines appended *after* it starts (like `tail -f`) unless
`--from-start` replays the file's existing content first.

## Edge-triggered, not level-triggered

The window fires once when the match count first *crosses* the
threshold — not once per matching line after that. If the log keeps
flooding with matches well past the threshold, that's still one alert
per flood, not a spam storm. It re-arms automatically once the count
ages back below the threshold (old matches fall out of the window), so a
second, later flood fires its own separate alert.

## Status: built and verified, including two real-time scenarios against a real running process

- **33 unit tests** (`cargo test --lib`) across `window` (the pure
  sliding-window decision logic — edge-triggered firing exactly once
  per flood, re-arming after aging out, manually constructed `Instant`s
  so no test needs a real sleep), `matcher` (regex vs. literal-string
  mode, case-insensitivity, and literal mode correctly not choking on
  regex metacharacters like `[503]`), `tail` (a real polling `tail -f`:
  pre-existing content correctly not replayed by default, `--from-start`
  correctly replaying it, a partial trailing line with no `\n` yet held
  back until complete, CRLF line endings handled), `duration` (parsing
  `30s`/`1m`/`2h`), and `action` (the alert message's exact wording, a
  real `sh -c` command execution and exit-code capture).
- **Live-verified against the actual compiled binary and a real running
  background process, not just the unit-level `SlidingWindow` tests**:
  started `logtripwire` watching a real empty scratch file with
  `--action exit`, `-c 3 -w 2s`, then really appended three real `ERROR`
  lines roughly 200ms apart from a separate shell — the real process
  printed the exact expected alert line and genuinely exited with code
  `1`. Separately, started it again with the same three matches spread
  1.5s apart against only a 1s window — confirmed via a real process
  liveness check that it was *still running* after all three lines
  landed, proving the slow-drip case genuinely never crosses the
  threshold, not just in the synthetic unit test.

**Not done / deliberately deferred**: multiple files/glob patterns in
one invocation (one file per process — run several `logtripwire`
instances, or pair with this workspace's `procrun` to supervise a few at
once); log rotation awareness (if the watched file gets rotated out from
under it — truncated and replaced — this keeps reading from its
previous byte offset into the new file's shorter content until it
catches up on its own; there's no `SIGHUP`-triggered file-handle
reopen); and structured-log field matching (JSON logs are matched as
plain text lines, same as anything else — no `jq`-style field
extraction).
