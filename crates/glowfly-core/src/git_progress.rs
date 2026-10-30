//! git's `--progress` lines on stderr, read for the status bar and folded for the log.
//!
//! git redraws a progress line in place with `\r` ("Receiving objects:  45% (450/1000), 1.20 MiB
//! | 300.00 KiB/s") and ends it with `, done.` and `\n`; the server's own lines come prefixed
//! with `remote: `. [`parse`] reads one such line into a [`Progress`] (git's numbers as numbers,
//! so the screen formats them itself), [`Tail`] finds the newest one in a stream of chunks, and
//! [`fold`] keeps only the last redraw of each line, so a log shows what a terminal would.
//! Pure.

/// The stage a transfer is in, as git names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// The server (or a local push) lists the objects to send ("Enumerating objects").
    Enumerating,
    /// "Counting objects".
    Counting,
    /// "Compressing objects".
    Compressing,
    /// "Receiving objects": a fetch or clone downloading.
    Receiving,
    /// "Writing objects": a push uploading.
    Writing,
    /// "Resolving deltas".
    Resolving,
    /// "Updating files": a clone checking out its files.
    Checkout,
}

/// One progress line.
#[derive(Clone, Debug, PartialEq)]
pub struct Progress {
    pub phase: Phase,
    /// The server's own count (`remote: Counting objects`), not this computer's.
    pub remote: bool,
    /// 0 to 100, when git gave one.
    pub percent: Option<u8>,
    /// Objects done, and the total when git gave a percentage.
    pub done: u64,
    pub total: Option<u64>,
    /// Bytes moved so far, and the current rate in bytes a second.
    pub bytes: Option<u64>,
    pub rate: Option<u64>,
}

/// The progress `line` reports (one redraw, without its `\r` or `\n`), or None when it isn't one.
pub fn parse(line: &str) -> Option<Progress> {
    let line = line.trim();
    let (remote, line) = match line.strip_prefix("remote:") {
        Some(rest) => (true, rest.trim_start()),
        None => (false, line),
    };
    let (name, rest) = line.split_once(':')?;
    let phase = match name {
        "Enumerating objects" => Phase::Enumerating,
        "Counting objects" => Phase::Counting,
        "Compressing objects" => Phase::Compressing,
        "Receiving objects" => Phase::Receiving,
        "Writing objects" => Phase::Writing,
        "Resolving deltas" => Phase::Resolving,
        "Updating files" | "Checking out files" => Phase::Checkout,
        _ => return None,
    };
    let rest = rest.trim().trim_end_matches(", done.").trim_end_matches(", done");
    // "45% (450/1000), 1.20 MiB | 300.00 KiB/s" or a bare count "5".
    let (counts, transfer) = match rest.split_once("),") {
        Some((c, t)) => (format!("{c})"), Some(t.trim())),
        None => (rest.to_owned(), None),
    };
    let (percent, done, total) = match counts.split_once('%') {
        Some((p, tail)) => {
            let percent = p.trim().parse::<u8>().ok()?.min(100);
            let inner = tail.trim().strip_prefix('(')?.strip_suffix(')')?;
            let (d, t) = inner.split_once('/')?;
            (Some(percent), d.trim().parse().ok()?, Some(t.trim().parse().ok()?))
        }
        None => (None, counts.trim().parse().ok()?, None),
    };
    let (bytes, rate) = match transfer {
        Some(t) => {
            let (size, speed) = t.split_once('|').map_or((t, None), |(s, r)| (s, Some(r)));
            (size_of(size), speed.and_then(|r| size_of(r.trim().strip_suffix("/s")?)))
        }
        None => (None, None),
    };
    Some(Progress { phase, remote, percent, done, total, bytes, rate })
}

/// "1.20 MiB", "512 bytes", "300.00 KiB" as a number of bytes.
fn size_of(text: &str) -> Option<u64> {
    let (number, unit) = text.trim().split_once(' ')?;
    let value: f64 = number.trim().parse().ok()?;
    let scale = match unit.trim() {
        "byte" | "bytes" => 1.0,
        "KiB" => 1024.0,
        "MiB" => 1024.0 * 1024.0,
        "GiB" => 1024.0 * 1024.0 * 1024.0,
        "TiB" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => return None,
    };
    (value >= 0.0).then(|| (value * scale).round() as u64)
}

/// The newest progress in a stream of stderr chunks: the last complete redraw (ended by `\r` or
/// `\n`) that is a progress line.
#[derive(Debug, Default)]
pub struct Tail {
    /// What follows the last line end seen, waiting for its end.
    carry: Vec<u8>,
}

/// How much of an unended line is kept (git's lines are short; anything longer isn't progress).
const MAX_CARRY: usize = 4096;

impl Tail {
    /// Reads `chunk`, returning the newest progress line it ended, if any.
    pub fn push(&mut self, chunk: &[u8]) -> Option<Progress> {
        self.carry.extend_from_slice(chunk);
        let end = self.carry.iter().rposition(|&b| b == b'\r' || b == b'\n');
        let found = end.and_then(|end| {
            let text = String::from_utf8_lossy(&self.carry[..end]);
            text.split(['\r', '\n']).rev().find_map(parse)
        });
        if let Some(end) = end {
            self.carry.drain(..=end);
        }
        if self.carry.len() > MAX_CARRY {
            self.carry.clear();
        }
        found
    }
}

/// `text` with each line's in-place redraws folded to the last one git drew, so the log holds
/// "Receiving objects: 100% (10/10), done." once instead of every step towards it.
pub fn fold(text: &str) -> String {
    if !text.contains('\r') {
        return text.to_owned();
    }
    let lines = text.split('\n').map(|line| line.split('\r').rev().find(|s| !s.trim().is_empty()).unwrap_or(""));
    lines.collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_lines_git_draws() {
        let p = parse("Receiving objects:  45% (450/1000), 1.20 MiB | 300.00 KiB/s").unwrap();
        assert_eq!(p.phase, Phase::Receiving);
        assert_eq!((p.percent, p.done, p.total), (Some(45), 450, Some(1000)));
        assert_eq!(p.bytes, Some(1_258_291));
        assert_eq!(p.rate, Some(307_200));
        assert!(!p.remote);
        let w = parse("Writing objects: 100% (3/3), 260 bytes | 260.00 KiB/s, done.").unwrap();
        assert_eq!((w.phase, w.percent, w.bytes), (Phase::Writing, Some(100), Some(260)));
        let r = parse("remote: Counting objects:  50% (5/10)").unwrap();
        assert!(r.remote);
        assert_eq!((r.phase, r.percent, r.bytes), (Phase::Counting, Some(50), None));
        let e = parse("remote: Enumerating objects: 5, done.").unwrap();
        assert_eq!((e.phase, e.percent, e.done, e.total), (Phase::Enumerating, None, 5, None));
        assert_eq!(parse("Resolving deltas: 100% (2/2), done.").map(|p| p.phase), Some(Phase::Resolving));
        assert_eq!(parse("Updating files:  10% (1/10)").map(|p| p.phase), Some(Phase::Checkout));
        assert_eq!(parse("Compressing objects:   0% (0/2)").map(|p| p.phase), Some(Phase::Compressing));
    }

    #[test]
    fn other_lines_are_not_progress() {
        for line in ["From github.com:a/b", " * branch main -> FETCH_HEAD", "remote: Create a pull request:", "To x", "", "error: x: 5"] {
            assert_eq!(parse(line), None, "{line}");
        }
        assert_eq!(parse("Receiving objects: lots"), None);
        assert_eq!(parse("Receiving objects: 45% (x/y)"), None);
    }

    #[test]
    fn the_tail_finds_the_newest_ended_redraw_across_chunks() {
        let mut tail = Tail::default();
        assert_eq!(tail.push(b"Receiving objects:  10% (1/10)\rReceiving obj"), parse("Receiving objects:  10% (1/10)"));
        assert_eq!(tail.push(b"ects:  20% (2/10)"), None, "not ended yet");
        assert_eq!(tail.push(b"\r").map(|p| p.percent), Some(Some(20)));
        assert_eq!(tail.push(b"From x\n"), None, "the newest ended line isn't progress");
        assert_eq!(tail.push(&[b'x'; 5000]), None);
        assert!(tail.carry.is_empty(), "a long unended line isn't kept");
    }

    #[test]
    fn folding_keeps_the_last_redraw_of_each_line() {
        let text = "$ git fetch\nremote: Counting objects:  50% (1/2)\rremote: Counting objects: 100% (2/2), done.\nReceiving objects:  50% (1/2)\rReceiving objects: 100% (2/2), done.\r\nFrom x";
        assert_eq!(fold(text), "$ git fetch\nremote: Counting objects: 100% (2/2), done.\nReceiving objects: 100% (2/2), done.\nFrom x");
        assert_eq!(fold("no redraws\nhere"), "no redraws\nhere");
    }
}
