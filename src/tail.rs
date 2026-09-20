//! Polls one file for newly-appended lines, the same `tail -f` core used
//! by this workspace's `logview` tool: a trailing line with no `\n` yet is
//! held back until it's completed rather than surfaced early.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

pub struct TailSource {
    file: File,
    pos: u64,
    leftover: Vec<u8>,
}

impl TailSource {
    /// Opens `path` positioned at the current end of the file — only
    /// lines appended *after* this call will ever be returned. This is
    /// the default, `tail -f`-like behavior: a tripwire watching for a
    /// pattern shouldn't re-alert on years of pre-existing log history
    /// every time it's (re)started.
    pub fn open_at_end(path: &Path) -> io::Result<Self> {
        let file = File::open(path)?;
        let pos = file.metadata()?.len();
        Ok(Self {
            file,
            pos,
            leftover: Vec::new(),
        })
    }

    /// Opens `path` positioned at its start — every existing line is
    /// replayed through `poll` before genuinely new ones. Used for
    /// `--from-start`.
    pub fn open_at_start(path: &Path) -> io::Result<Self> {
        let file = File::open(path)?;
        Ok(Self {
            file,
            pos: 0,
            leftover: Vec::new(),
        })
    }

    /// Reads whatever is new since the last call and returns complete
    /// lines (`\n`- or `\r\n`-terminated). Bytes since the last newline
    /// are held in `leftover` and prefixed onto the next read.
    pub fn poll(&mut self) -> io::Result<Vec<String>> {
        self.file.seek(SeekFrom::Start(self.pos))?;
        let mut chunk = Vec::new();
        self.file.read_to_end(&mut chunk)?;
        if chunk.is_empty() {
            return Ok(Vec::new());
        }
        self.pos += chunk.len() as u64;
        self.leftover.extend_from_slice(&chunk);

        let mut lines = Vec::new();
        while let Some(idx) = self.leftover.iter().position(|&b| b == b'\n') {
            let line_bytes: Vec<u8> = self.leftover.drain(..=idx).collect();
            let without_newline = &line_bytes[..line_bytes.len() - 1];
            let text = String::from_utf8_lossy(without_newline);
            lines.push(text.trim_end_matches('\r').to_string());
        }
        Ok(lines)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("logtripwire-test-{}-{}", std::process::id(), name))
    }

    #[test]
    fn open_at_end_does_not_see_pre_existing_content() {
        let path = temp_path("at-end");
        std::fs::write(&path, b"already here before we started\n").unwrap();
        let mut source = TailSource::open_at_end(&path).unwrap();
        assert_eq!(source.poll().unwrap(), Vec::<String>::new());

        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        writeln!(f, "genuinely new line").unwrap();
        drop(f);

        assert_eq!(source.poll().unwrap(), vec!["genuinely new line"]);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn open_at_start_replays_existing_content() {
        let path = temp_path("at-start");
        std::fs::write(&path, b"line one\nline two\n").unwrap();
        let mut source = TailSource::open_at_start(&path).unwrap();
        assert_eq!(source.poll().unwrap(), vec!["line one", "line two"]);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn a_partial_trailing_line_is_held_back_until_completed() {
        let path = temp_path("partial");
        std::fs::write(&path, b"").unwrap();
        let mut source = TailSource::open_at_start(&path).unwrap();

        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        write!(f, "no newline yet").unwrap();
        f.flush().unwrap();
        assert_eq!(source.poll().unwrap(), Vec::<String>::new());

        writeln!(f, " — now complete").unwrap();
        drop(f);
        assert_eq!(
            source.poll().unwrap(),
            vec!["no newline yet — now complete"]
        );
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn crlf_line_endings_are_handled() {
        let path = temp_path("crlf");
        std::fs::write(&path, b"windows style\r\nanother\r\n").unwrap();
        let mut source = TailSource::open_at_start(&path).unwrap();
        assert_eq!(source.poll().unwrap(), vec!["windows style", "another"]);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn poll_with_nothing_new_returns_empty() {
        let path = temp_path("idle");
        std::fs::write(&path, b"one line\n").unwrap();
        let mut source = TailSource::open_at_start(&path).unwrap();
        assert_eq!(source.poll().unwrap(), vec!["one line"]);
        assert_eq!(source.poll().unwrap(), Vec::<String>::new());
        std::fs::remove_file(&path).ok();
    }
}
