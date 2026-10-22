//! The bug-report bundle: every log file verbatim, oldest first within each log, under a short
//! header (when it was exported, the versions, the platform), written as one plain-text file.

use super::{ACTIVITY_FILE, ERROR_FILE, KEEP_ROTATED, PERF_FILE, rotated_path};
use crate::persist;
use std::io;
use std::path::{Path, PathBuf};

/// The bundle's extension, for the save dialog's filter.
pub const BUNDLE_EXT: &str = "txt";

/// `at_secs` (Unix seconds) shown `utc_offset_secs` from UTC (UTC when the offset is invalid).
fn local(at_secs: i64, utc_offset_secs: i32) -> chrono::DateTime<chrono::FixedOffset> {
    use chrono::Offset;
    let offset = chrono::FixedOffset::east_opt(utc_offset_secs).unwrap_or_else(|| chrono::Utc.fix());
    chrono::DateTime::from_timestamp(at_secs, 0).unwrap_or_default().with_timezone(&offset)
}

/// The suggested file name: "glowfly-logs-YYYYMMDD-HHMMSS.txt", at `at_secs` in the shown offset.
pub fn bundle_name(at_secs: i64, utc_offset_secs: i32) -> String {
    format!("glowfly-logs-{}.{BUNDLE_EXT}", local(at_secs, utc_offset_secs).format("%Y%m%d-%H%M%S"))
}

/// "YYYY-MM-DD HH:MM:SS +HH:MM": a time that reads the same in every bug report, whatever the
/// date-format setting.
pub fn stamp(at_secs: i64, utc_offset_secs: i32) -> String {
    local(at_secs, utc_offset_secs).format("%Y-%m-%d %H:%M:%S %:z").to_string()
}

/// The platform for a bundle's header: the OS and architecture Glowfly was built for, such as
/// "windows x86_64".
pub fn platform() -> String {
    format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)
}

/// The log files a bundle holds, in its order: the activity file, then the error log and the
/// performance log, each from its oldest rotated file to the live one. Some may not exist.
pub fn bundle_sources(dir: &Path) -> Vec<PathBuf> {
    let mut out = vec![dir.join(ACTIVITY_FILE)];
    for name in [ERROR_FILE, PERF_FILE] {
        let live = dir.join(name);
        out.extend((1..=KEEP_ROTATED).rev().map(|n| rotated_path(&live, n)));
        out.push(live);
    }
    out
}

/// What went into a bundle: the files read and their size in bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BundleReport {
    pub files: usize,
    pub bytes: u64,
}

fn file_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// The bundle of the logs in `dir`: "Glowfly logs", a "<label>: <value>" line for each `header`
/// pair (line breaks in values become spaces), the files it holds, a blank line, then each file
/// under a "===== <name> (<n> bytes) =====" line, decoded lossily. Missing files are left out;
/// one that can't be read gets a line saying so.
pub fn bundle_text(dir: &Path, header: &[(String, String)]) -> (String, BundleReport) {
    let mut report = BundleReport { files: 0, bytes: 0 };
    let mut names = Vec::new();
    let mut body = String::new();
    for src in bundle_sources(dir) {
        let name = file_name(&src);
        match std::fs::read(&src) {
            Ok(bytes) => {
                body.push_str(&format!("===== {name} ({} bytes) =====\n", bytes.len()));
                let text = String::from_utf8_lossy(&bytes);
                body.push_str(&text);
                if !text.is_empty() && !text.ends_with('\n') {
                    body.push('\n');
                }
                report.files += 1;
                report.bytes += bytes.len() as u64;
                names.push(name);
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => body.push_str(&format!("===== {name} (couldn't read: {e}) =====\n")),
        }
    }
    let mut out = String::from("Glowfly logs\n");
    for (label, value) in header {
        out.push_str(&format!("{label}: {}\n", value.replace(['\r', '\n'], " ")));
    }
    let files = if names.is_empty() { "none".to_owned() } else { names.join(", ") };
    out.push_str(&format!("Files: {files}\n\n"));
    out.push_str(&body);
    (out, report)
}

/// Writes the bundle of the logs in `dir` to `dest` atomically. Refuses a `dest` that is one of
/// the logs themselves (by identity, so a differently cased name on a case-insensitive file
/// system is caught too).
pub fn write_bundle(dir: &Path, header: &[(String, String)], dest: &Path) -> io::Result<BundleReport> {
    if dest.exists()
        && let Some(src) = bundle_sources(dir).into_iter().find(|src| same_file::is_same_file(src, dest).unwrap_or(false))
    {
        let msg = format!("Choose another file: {} is one of the logs being exported.", file_name(&src));
        return Err(io::Error::new(io::ErrorKind::InvalidInput, msg));
    }
    let (text, report) = bundle_text(dir, header);
    persist::atomic_write(dest, text.as_bytes())?;
    Ok(report)
}

#[cfg(test)]
#[path = "bundle_tests.rs"]
mod tests;
