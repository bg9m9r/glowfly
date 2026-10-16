//! Conflict markers left in working files: counting the complete marker regions git (or a
//! merge tool) wrote, so Mark resolved can warn before staging them.

use std::path::Path;

/// Files larger than this are not read for markers (their count is unknown).
pub const MARKER_SCAN_MAX: u64 = 16 * 1024 * 1024;

/// How far into a file a NUL byte makes it binary (git looks as far).
const BINARY_PROBE: usize = 8000;

/// Where a scan is inside a marker region of marker size `n`.
#[derive(Clone, Copy)]
enum At {
    Outside,
    Ours(usize),
    Base(usize),
    Theirs(usize),
}

/// The number of complete conflict marker regions in `text`: a start (7 or more `<`), an
/// optional base marker (as many `|`), a separator (as many `=`) and an end (as many `>`), each
/// at the start of a line. A new start inside a region begins it again; a region that never
/// ends doesn't count, and neither does a lone `=======` line (a Markdown heading rule).
pub fn marker_regions(text: &[u8]) -> usize {
    let mut at = At::Outside;
    let mut count = 0;
    for line in text.split(|&b| b == b'\n') {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if let Some(n) = run(line, b'<') {
            at = At::Ours(n);
            continue;
        }
        at = match at {
            At::Outside => At::Outside,
            At::Ours(n) if run(line, b'|') == Some(n) => At::Base(n),
            At::Ours(n) | At::Base(n) if separator(line, n) => At::Theirs(n),
            At::Theirs(n) if run(line, b'>') == Some(n) => {
                count += 1;
                At::Outside
            }
            other => other,
        };
    }
    count
}

/// The length of the run of `c` that starts `line`, when it is a marker: at least 7 long and
/// followed by the end of the line or a space or tab (then the label).
fn run(line: &[u8], c: u8) -> Option<usize> {
    let n = line.iter().take_while(|&&b| b == c).count();
    (n >= 7 && matches!(line.get(n), None | Some(b' ' | b'\t'))).then_some(n)
}

/// Whether `line` is exactly `n` `=`, optionally followed by whitespace.
fn separator(line: &[u8], n: usize) -> bool {
    line.len() >= n && line[..n].iter().all(|&b| b == b'=') && line[n..].iter().all(u8::is_ascii_whitespace)
}

/// The marker regions in each of `paths` (repo-relative, `/`-separated) in `workdir`'s working
/// tree, in order. `None` when the count is unknown or means nothing: a missing path, anything
/// but a regular file (links are not followed), a file over `MARKER_SCAN_MAX`, or a binary one.
pub fn conflict_markers(workdir: &Path, paths: &[String]) -> Vec<(String, Option<usize>)> {
    paths.iter().map(|p| (p.clone(), scan_with_limit(workdir, p, MARKER_SCAN_MAX))).collect()
}

/// `conflict_markers` for one path, reading files of at most `max` bytes.
pub(super) fn scan_with_limit(workdir: &Path, path: &str, max: u64) -> Option<usize> {
    let full = workdir.join(path);
    let meta = std::fs::symlink_metadata(&full).ok()?;
    if !meta.file_type().is_file() || meta.len() > max {
        return None;
    }
    let bytes = std::fs::read(&full).ok()?;
    // It may have grown since.
    if bytes.len() as u64 > max || bytes[..bytes.len().min(BINARY_PROBE)].contains(&0) {
        return None;
    }
    Some(marker_regions(&bytes))
}
