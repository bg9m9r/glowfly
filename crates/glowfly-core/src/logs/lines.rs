//! The text of log lines: git commands for the activity log and slow work for the performance
//! log. Pure formatting, no I/O.

use super::format_took;
use crate::gitcli::GitRun;
use std::borrow::Cow;
use std::time::Duration;

/// The longest activity line a git command gets, in characters.
const GIT_LINE_CHARS: usize = 300;

/// `arg` with the user name and password of any `scheme://user:pass@host` URL in it replaced by
/// `***`. An scp-style address (`git@host:org/repo`) has no secret and is left alone.
pub fn redact_url_userinfo(arg: &str) -> Cow<'_, str> {
    if !arg.contains("://") || !arg.contains('@') {
        return Cow::Borrowed(arg);
    }
    let mut out = String::with_capacity(arg.len());
    let mut rest = arg;
    let mut changed = false;
    while let Some(i) = rest.find("://") {
        let (head, after) = rest.split_at(i + 3);
        out.push_str(head);
        let end = after.find(|c: char| c == '/' || c == '?' || c == '#' || c.is_whitespace()).unwrap_or(after.len());
        let authority = &after[..end];
        match authority.rfind('@') {
            Some(at) => {
                out.push_str("***");
                out.push_str(&authority[at..]);
                changed = true;
            }
            None => out.push_str(authority),
        }
        rest = &after[end..];
    }
    if !changed {
        return Cow::Borrowed(arg);
    }
    out.push_str(rest);
    Cow::Owned(out)
}

/// `text` cut to `max` characters, ending in "…" when it was longer.
fn cap_chars(text: String, max: usize) -> String {
    match text.char_indices().nth(max) {
        None => text,
        Some(_) => {
            let end = text.char_indices().nth(max - 1).map_or(text.len(), |(i, _)| i);
            format!("{}…", &text[..end])
        }
    }
}

/// The activity line for a git command: "git status --porcelain=v2 · glowfly · 34 ms", with
/// " · exit 1" for a non-zero exit code and " · timed out"; at most 300 characters.
pub fn git_run_line(run: &GitRun) -> String {
    let args: Vec<Cow<str>> = run.args.iter().map(|a| redact_url_userinfo(a)).collect();
    let folder = run.cwd.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| crate::native_path::shown(&run.cwd));
    let mut line = format!("git {} · {folder} · {}", args.join(" "), format_took(run.took));
    if let Some(code) = run.code.filter(|c| *c != 0) {
        line.push_str(&format!(" · exit {code}"));
    }
    if run.timed_out {
        line.push_str(" · timed out");
    }
    cap_chars(line, GIT_LINE_CHARS)
}

/// One line of the performance log: `YYYY-MM-DD HH:MM:SS.mmm\t<ms> ms\t<what>\t<detail>\n`, the
/// time `at_ms` (Unix milliseconds) shown `utc_offset_secs` from UTC. Tabs and line breaks in
/// `what` and `detail` become spaces, so each line keeps its four columns.
pub fn perf_line(at_ms: i64, utc_offset_secs: i32, took: Duration, what: &str, detail: &str) -> String {
    use chrono::Offset;
    let offset = chrono::FixedOffset::east_opt(utc_offset_secs).unwrap_or_else(|| chrono::Utc.fix());
    let at = chrono::DateTime::from_timestamp_millis(at_ms).unwrap_or_default().with_timezone(&offset);
    let clean = |s: &str| s.replace(['\t', '\r', '\n'], " ");
    format!("{}\t{} ms\t{}\t{}\n", at.format("%Y-%m-%d %H:%M:%S%.3f"), took.as_millis(), clean(what), clean(detail))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn run(args: &[&str]) -> GitRun {
        GitRun {
            cwd: crate::paths::temp_dir().join("glowfly"),
            args: args.iter().map(|s| s.to_string()).collect(),
            took: Duration::from_millis(34),
            code: Some(0),
            ok: true,
            timed_out: false,
        }
    }

    #[test]
    fn redact_url_userinfo_hides_credentials() {
        assert_eq!(redact_url_userinfo("https://user:tok@host/x"), "https://***@host/x");
        assert_eq!(redact_url_userinfo("remote.o.url=https://tok@host/x"), "remote.o.url=https://***@host/x");
        assert!(matches!(redact_url_userinfo("https://host/x"), Cow::Borrowed(_)));
        assert!(matches!(redact_url_userinfo("git@host:org/x"), Cow::Borrowed(_)));
        assert_eq!(redact_url_userinfo("https://host/a@b"), "https://host/a@b", "an @ in the path isn't user info");
    }

    #[test]
    fn git_run_line_text() {
        assert_eq!(git_run_line(&run(&["status", "--porcelain=v2", "-z"])), "git status --porcelain=v2 -z · glowfly · 34 ms");
        let failed = GitRun { code: Some(1), ..run(&["config", "--get", "x.y"]) };
        assert_eq!(git_run_line(&failed), "git config --get x.y · glowfly · 34 ms · exit 1");
        let slow = GitRun { code: None, ok: false, timed_out: true, took: Duration::from_millis(30_000), ..run(&["fetch"]) };
        assert_eq!(git_run_line(&slow), "git fetch · glowfly · 30.0 s · timed out");
        let secret = run(&["push", "https://me:secret@example.com/r.git"]);
        assert_eq!(git_run_line(&secret), "git push https://***@example.com/r.git · glowfly · 34 ms");
        let long = git_run_line(&run(&["add", &"é".repeat(400)]));
        assert_eq!(long.chars().count(), 300);
        assert!(long.ends_with('…'));
        let root = GitRun { cwd: PathBuf::from(std::path::MAIN_SEPARATOR_STR), ..run(&["version"]) };
        assert!(git_run_line(&root).starts_with("git version · "), "a folder without a name shows its path");
    }

    #[test]
    fn perf_line_text() {
        // 2026-03-04 05:06:07.089 UTC, shown at +02:00.
        let at = 1_772_600_767_089;
        let line = perf_line(at, 2 * 3600, Duration::from_millis(1234), "load-repo", "repo\tx");
        assert_eq!(line, "2026-03-04 07:06:07.089\t1234 ms\tload-repo\trepo x\n");
    }
}
