//! Reading the end of a text log (the error and performance logs) for the logs panel.

use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

/// The last lines of a log file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LogTail {
    pub lines: Vec<String>,
    /// The file is longer than what was read: earlier lines are left out.
    pub truncated: bool,
}

/// The whole lines in the last `max_bytes` of the file at `path`, decoded lossily, without line
/// endings ("\r\n" too). A missing file has none.
pub fn read_tail(path: &Path, max_bytes: u64) -> io::Result<LogTail> {
    let mut f = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(LogTail::default()),
        Err(e) => return Err(e),
    };
    let len = f.metadata()?.len();
    let start = len.saturating_sub(max_bytes);
    let truncated = start > 0;
    // One byte before the cut too: when it is a line end, the first line read is whole.
    let from = start.saturating_sub(u64::from(truncated));
    f.seek(SeekFrom::Start(from))?;
    let mut buf = Vec::new();
    f.take(len - from).read_to_end(&mut buf)?;
    let text = String::from_utf8_lossy(&buf);
    let body = if truncated {
        match text.find('\n') {
            Some(i) => &text[i + 1..],
            None => "",
        }
    } else {
        &text
    };
    let lines = body.lines().map(|l| l.trim_end_matches('\r').to_owned()).collect();
    Ok(LogTail { lines, truncated })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn scratch(name: &str) -> PathBuf {
        let dir = crate::paths::temp_dir().join(format!("trellis-tail-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn read_tail_missing_is_empty() {
        let dir = scratch("missing");
        assert_eq!(read_tail(&dir.join("errors.log"), 100).unwrap(), LogTail::default());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_tail_small_file_whole() {
        let dir = scratch("small");
        let path = dir.join("errors.log");
        std::fs::write(&path, "one\ntwo\nthree\n").unwrap();
        let tail = read_tail(&path, 100).unwrap();
        assert_eq!(tail, LogTail { lines: vec!["one".into(), "two".into(), "three".into()], truncated: false });
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_tail_cuts_partial_first_line() {
        let dir = scratch("cut");
        let path = dir.join("errors.log");
        // 29 bytes; the last 12 start in the middle of "second line".
        std::fs::write(&path, "first line\nsecond line\nthird\n").unwrap();
        let tail = read_tail(&path, 12).unwrap();
        assert_eq!(tail, LogTail { lines: vec!["third".into()], truncated: true });
        // A cut right after a line end keeps the line after it.
        let tail = read_tail(&path, 18).unwrap();
        assert_eq!(tail, LogTail { lines: vec!["second line".into(), "third".into()], truncated: true });
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_tail_handles_crlf() {
        let dir = scratch("crlf");
        let path = dir.join("performance.log");
        std::fs::write(&path, "a\r\nb\r\n").unwrap();
        assert_eq!(read_tail(&path, 100).unwrap().lines, ["a", "b"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
