//! Which tag names look like a release. Git defines none, so this is a heuristic over the short
//! tag name: a version number (`1.2`, `v1.2.3`, `1.0.0-rc.1`), possibly after a prefix such as
//! `release/` or `glowfly-`. A bare major (`v2`) is a moving tag, not a release.

/// Whether `name` looks like the name of a release.
pub fn is_release_tag(name: &str) -> bool {
    let last = name.rsplit('/').next().unwrap_or("");
    let starts = std::iter::once(0).chain(last.match_indices(['-', '_', '@']).map(|(i, _)| i + 1));
    starts.into_iter().any(|at| last.get(at..).is_some_and(version_then_suffix))
}

/// `s` is `[vV]N.N[.N[.N]]` followed by nothing or a suffix of word characters.
fn version_then_suffix(s: &str) -> bool {
    let s = s.strip_prefix(['v', 'V']).unwrap_or(s);
    let Some(mut rest) = digits(s).and_then(|r| r.strip_prefix('.')).and_then(digits) else { return false };
    for _ in 0..2 {
        match rest.strip_prefix('.').and_then(digits) {
            Some(r) => rest = r,
            None => break,
        }
    }
    rest.chars().next().is_none_or(|c| matches!(c, '-' | '+' | '.' | '_') || c.is_ascii_alphabetic())
        && rest.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+' | '_'))
}

/// What follows the run of digits at the start of `s`; None without one.
fn digits(s: &str) -> Option<&str> {
    let rest = s.trim_start_matches(|c: char| c.is_ascii_digit());
    (rest.len() < s.len()).then_some(rest)
}

#[cfg(test)]
mod tests {
    use super::is_release_tag;

    #[test]
    fn versions_are_releases() {
        for t in
            ["v1.2.3", "1.2", "V2.0", "v1.2.3-rc.1", "1.0.0+build.5", "release/1.4", "glowfly-v0.3.0", "pkg@1.2.3", "2026.10", "1.0rc1"]
        {
            assert!(is_release_tag(t), "{t}");
        }
    }

    #[test]
    fn other_names_are_not() {
        for t in ["v1", "v2", "nightly", "latest", "backup-2024-01-01", "v1.x", "foo1.2", ""] {
            assert!(!is_release_tag(t), "{t}");
        }
    }
}
