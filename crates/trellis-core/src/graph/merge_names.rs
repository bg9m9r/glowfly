//! The branches a merge commit merged, read from its summary line: git's default merge messages
//! and the code host's pull request title. They name the lanes of a merge's other parents.

use crate::branch_colour::branch_key;

/// The colour keys of the branches merged by a commit with summary `summary`, one per parent
/// after the first and in parent order; empty when the summary doesn't say. `remotes` are the
/// configured remote names, stripped from remote-tracking names.
pub(crate) fn merged_names(summary: &str, remotes: &[&str]) -> Vec<String> {
    let line = summary.lines().next().unwrap_or("").trim();
    if let Some(rest) = line.strip_prefix("Merge pull request #") {
        let Some((number, from)) = rest.split_once(" from ") else { return Vec::new() };
        if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
            return Vec::new();
        }
        let head = from.split_whitespace().next().unwrap_or("");
        return head.split_once('/').map(|(_, key)| key).filter(|k| !k.is_empty()).map(|k| vec![k.to_owned()]).unwrap_or_default();
    }
    let forms = [
        ("Merge remote-tracking branches ", true),
        ("Merge remote-tracking branch ", true),
        ("Merge branches ", false),
        ("Merge branch ", false),
    ];
    let Some((mut rest, remote)) = forms.iter().find_map(|(p, remote)| line.strip_prefix(p).map(|r| (r, *remote))) else {
        return Vec::new();
    };
    let mut names = Vec::new();
    while let Some(quoted) = rest.strip_prefix('\'') {
        let Some(end) = quoted.find('\'') else { return Vec::new() };
        names.push(&quoted[..end]);
        rest = &quoted[end + 1..];
        match rest.strip_prefix(", ").or_else(|| rest.strip_prefix(" and ")) {
            Some(next) => rest = next,
            None => break,
        }
    }
    if names.is_empty() || !(rest.is_empty() || rest.starts_with(" of ") || rest.starts_with(" into ")) {
        return Vec::new();
    }
    names
        .into_iter()
        .map(|n| if remote { branch_key(&format!("refs/remotes/{n}"), remotes).map(str::to_owned) } else { Some(n.to_owned()) })
        .collect::<Option<Vec<String>>>()
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(s: &str) -> Vec<String> {
        merged_names(s, &["origin"])
    }

    #[test]
    fn git_default_messages() {
        assert_eq!(names("Merge branch 'topic'"), ["topic"]);
        assert_eq!(names("Merge branch 'topic' into next"), ["topic"]);
        assert_eq!(names("Merge branch 'x' of https://h/r"), ["x"]);
        assert_eq!(names("Merge branch 'x' of https://h/r into y"), ["x"]);
        assert_eq!(names("Merge branches 'a' and 'b'"), ["a", "b"]);
        assert_eq!(names("Merge branches 'a', 'b' and 'c'"), ["a", "b", "c"]);
        assert_eq!(names("Merge remote-tracking branch 'origin/x'"), ["x"]);
        assert_eq!(names("Merge remote-tracking branch 'origin/feature/x' into main\n\nbody"), ["feature/x"]);
    }

    #[test]
    fn pull_request_titles() {
        assert_eq!(names("Merge pull request #12 from ada/fix/login"), ["fix/login"]);
        assert!(names("Merge pull request #x from ada/fix").is_empty());
    }

    #[test]
    fn other_summaries_name_nothing() {
        for s in ["Merge tag 'v1'", "Merge commit 'abc'", "Fix bug", "Merge branch 'unclosed", "Merge branch 'a' then more", ""] {
            assert!(names(s).is_empty(), "{s}");
        }
    }
}
