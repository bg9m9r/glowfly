//! The words of the confirmation before Fetch full history downloads a large repository.

use glowfly_core::history_size::{HistorySize, SizeSource};
use glowfly_core::lfs::pointer::size_text;

/// The confirmation's title, body and button when `size` is large; None otherwise. `commits` is
/// how many commits the clone has (`truncated`: the graph stopped loading, so at least that).
pub fn confirm(size: &HistorySize, remote: &str, commits: usize, truncated: bool) -> Option<(String, String, String)> {
    let HistorySize::Known { bytes, source } = size else { return None };
    if !size.is_large() {
        return None;
    }
    let about = size_text(*bytes);
    let first = match source {
        SizeSource::Service(name) => format!("{name} reports this repository's size as about {about}."),
        SizeSource::Folder(url) => format!("The repository at {url} takes about {about} on disk."),
    };
    let count = match (commits, truncated) {
        (1, false) => "1 commit".to_owned(),
        (n, false) => format!("{n} commits"),
        (n, true) => format!("at least {n} commits"),
    };
    let body = format!(
        "{first} This clone has {count} so far; fetching the full history downloads the rest from {remote}, which can take a long \
         time and use about that much disk space. You can cancel it from the status bar while it runs."
    );
    Some(("Fetch full history?".to_owned(), body, "Fetch full history".to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use glowfly_core::history_size::LARGE_HISTORY_BYTES;

    #[test]
    fn asks_only_for_a_large_known_size() {
        let gib = |n: u64| HistorySize::Known { bytes: n * 1024 * 1024 * 1024, source: SizeSource::Service("GitHub".into()) };
        let (title, body, button) = confirm(&gib(2), "origin", 1, false).unwrap();
        assert_eq!((title.as_str(), button.as_str()), ("Fetch full history?", "Fetch full history"));
        assert!(body.starts_with("GitHub reports this repository's size as about 2.0 GB. This clone has 1 commit so far;"), "{body}");
        assert!(body.contains("the rest from origin") && body.contains("cancel it from the status bar"), "{body}");
        let folder = HistorySize::Known { bytes: LARGE_HISTORY_BYTES, source: SizeSource::Folder("../big".into()) };
        let (_, body, _) = confirm(&folder, "up", 40, true).unwrap();
        assert!(body.starts_with("The repository at ../big takes about 500.0 MB on disk. This clone has at least 40 commits"), "{body}");
        let small = HistorySize::Known { bytes: LARGE_HISTORY_BYTES - 1, source: SizeSource::Service("GitLab".into()) };
        assert!(confirm(&small, "origin", 1, false).is_none());
        assert!(confirm(&HistorySize::Unknown, "origin", 1, false).is_none());
    }
}
