//! The text rules of the grouping: squash landings, closing lines, merge titles and who counts
//! as the same author.

use crate::graph::merge_names::merged_names;
use crate::repo::Author;

/// The issue or pull request number in a subject that ends in `(#<digits>)` (trailing
/// whitespace ignored): what a squash landing's subject looks like.
pub(super) fn squash_number(summary: &str) -> Option<&str> {
    let s = summary.trim_end().strip_suffix(')')?;
    let at = s.rfind("(#")?;
    let digits = &s[at + 2..];
    (!digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())).then_some(digits)
}

/// Whether the (flattened) body has a closing line: the word "closes" or "fixes" (any case, an
/// optional colon) followed by `#<digits>`. The body is one line of single-spaced words, so word
/// pairs are matched.
pub(super) fn closes_issue(body: &str) -> bool {
    let mut words = body.split_whitespace().peekable();
    while let Some(w) = words.next() {
        let w = w.strip_suffix(':').unwrap_or(w);
        if !(w.eq_ignore_ascii_case("closes") || w.eq_ignore_ascii_case("fixes")) {
            continue;
        }
        let Some(next) = words.peek() else { return false };
        let n = next.trim_end_matches(['.', ',', ';', ':', ')']);
        if n.strip_prefix('#').is_some_and(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit())) {
            return true;
        }
    }
    false
}

/// The title of a merge chapter: the branch it merged for git's default messages and a pull
/// request's ("<branch> (#n)"), otherwise the subject itself.
pub(super) fn merge_title(summary: &str, remotes: &[&str]) -> String {
    let line = summary.lines().next().unwrap_or("").trim();
    let names = merged_names(line, remotes);
    if let Some(rest) = line.strip_prefix("Merge pull request #") {
        let number: String = rest.chars().take_while(char::is_ascii_digit).collect();
        if let [name] = names.as_slice() {
            return format!("{name} (#{number})");
        }
    }
    if names.is_empty() { line.to_owned() } else { names.join(", ") }
}

/// Whether two commits are by the same person: the same email ignoring ASCII case, or equal
/// names when either email is empty.
pub(super) fn same_author(a: &Author, b: &Author) -> bool {
    if a.email().is_empty() || b.email().is_empty() { a.name() == b.name() } else { a.email().eq_ignore_ascii_case(b.email()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn squash_numbers() {
        assert_eq!(squash_number("Add x (#12)"), Some("12"));
        assert_eq!(squash_number("Add x (#12)  "), Some("12"));
        assert_eq!(squash_number("(#12) Add x"), None);
        assert_eq!(squash_number("Add x (#)"), None);
        assert_eq!(squash_number("Add x (#1a)"), None);
        assert_eq!(squash_number("Add x"), None);
    }

    #[test]
    fn closing_lines() {
        for yes in ["Closes #7", "closes: #7.", "this fixes #12, really", "Why. FIXES #3", "Fixes: #9)"] {
            assert!(closes_issue(yes), "{yes}");
        }
        for no in ["see #7", "Closes 7", "encloses #7", "Closes", "Closes #", "Closes #7x", ""] {
            assert!(!closes_issue(no), "{no}");
        }
    }

    #[test]
    fn merge_titles() {
        let t = |s: &str| merge_title(s, &["origin"]);
        assert_eq!(t("Merge branch 'topic'"), "topic");
        assert_eq!(t("Merge branch 'topic' into next"), "topic");
        assert_eq!(t("Merge branches 'a' and 'b'"), "a, b");
        assert_eq!(t("Merge remote-tracking branch 'origin/feature/x'"), "feature/x");
        assert_eq!(t("Merge pull request #12 from ada/fix/login"), "fix/login (#12)");
        assert_eq!(t("Merge tag 'v1'"), "Merge tag 'v1'");
        assert_eq!(t("Land the importer"), "Land the importer");
        assert_eq!(t("Merge pull request #x from ada/fix"), "Merge pull request #x from ada/fix");
    }

    #[test]
    fn authors() {
        let a = |n: &str, e: &str| Author::new(n, e);
        assert!(same_author(&a("Ada", "ada@x.org"), &a("A. Lovelace", "ADA@x.org")));
        assert!(!same_author(&a("Ada", "ada@x.org"), &a("Ada", "bob@x.org")));
        assert!(same_author(&a("Ada", ""), &a("Ada", "ada@x.org")));
        assert!(!same_author(&a("Ada", ""), &a("Bob", "ada@x.org")));
    }
}
