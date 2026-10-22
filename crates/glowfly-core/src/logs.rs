//! Glowfly's log files, all in `settings::log_dir`:
//!
//! - `activity.jsonl`: the activity panel's lines across sessions, one JSON object per line
//!   (`{"v":1,"session":…,"at":…,"ok":…,"text":…,"repo":…,"run":{…}}`, times in Unix seconds,
//!   `session` the start of the session that wrote it, `repo` the repository of the tab it was
//!   logged in, left out for a tab without one, `run` the finished operation it records
//!   ([`RunInfo`]), left out for a plain line). Kept to the newest entries: trimmed when it is opened, and
//!   whenever appends take it past twice that many ([`ActivityFile`]).
//! - `errors.log`: warnings, errors and panics, as text lines from the app's tracing setup,
//!   rotated by size ([`RotatingFile`], [`ERROR_LOG_BYTES`]).
//! - `performance.log`: effects and git commands that took [`SLOW`] or longer, one tab-separated
//!   line each ([`perf_line`]), rotated the same way at [`PERF_LOG_BYTES`].
//!
//! A rotated file moves to `<name>.1.log`, the older ones up by one, keeping [`KEEP_ROTATED`].
//! Every write goes through `persist`; [`read_tail`] reads the end of a text log for the panel,
//! and [`output_lines`], [`classify_line`] and [`last_message`] read a job's output for it.
//! [`write_bundle`] writes the bug-report bundle: every log file verbatim under a short header,
//! in one plain-text file.

mod activity;
mod bundle;
mod lines;
mod output;
mod rotate;
mod tail;

pub use activity::{ActivityEntry, ActivityFile, MAX_ENTRY_BYTES, RunInfo, activity_line, parse_activity, same_repo};
pub use bundle::{BUNDLE_EXT, BundleReport, bundle_name, bundle_sources, bundle_text, platform, stamp, write_bundle};
pub use lines::{git_run_line, perf_line, redact_url_userinfo};
pub use output::{OutputLine, classify_line, last_message, output_lines};
pub use rotate::{RotatingFile, rotated_path};
pub use tail::{LogTail, read_tail};

use std::time::Duration;

pub const ACTIVITY_FILE: &str = "activity.jsonl";
pub const ERROR_FILE: &str = "errors.log";
pub const PERF_FILE: &str = "performance.log";

/// The size past which the error log rotates.
pub const ERROR_LOG_BYTES: u64 = 1 << 20;
/// The size past which the performance log rotates.
pub const PERF_LOG_BYTES: u64 = 512 << 10;
/// How many rotated files of each log are kept.
pub const KEEP_ROTATED: usize = 3;
/// Effects and git commands that take this long or longer go to the performance log.
pub const SLOW: Duration = Duration::from_millis(500);
/// How much of the end of the error or performance log the panel shows.
pub const TAIL_BYTES: u64 = 256 << 10;

/// A duration as people read it: "850 ms", "1.2 s", "2 min 5 s".
pub fn format_took(d: Duration) -> String {
    let ms = d.as_millis();
    if ms < 1000 {
        format!("{ms} ms")
    } else if ms < 60_000 {
        let tenths = ms / 100;
        format!("{}.{} s", tenths / 10, tenths % 10)
    } else {
        let secs = ms / 1000;
        format!("{} min {} s", secs / 60, secs % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_took_boundaries() {
        let ms = Duration::from_millis;
        assert_eq!(format_took(ms(0)), "0 ms");
        assert_eq!(format_took(ms(999)), "999 ms");
        assert_eq!(format_took(ms(1000)), "1.0 s");
        assert_eq!(format_took(ms(1250)), "1.2 s");
        assert_eq!(format_took(ms(59_960)), "59.9 s");
        assert_eq!(format_took(ms(60_000)), "1 min 0 s");
        assert_eq!(format_took(ms(125_400)), "2 min 5 s");
    }
}
