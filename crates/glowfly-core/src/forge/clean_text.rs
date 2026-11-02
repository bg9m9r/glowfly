//! Text from a hosting service's API made safe to show, once, as it comes in (from `links`,
//! which every pull request, issue and detail result passes through): titles, names and states
//! printable (`text::printable`), a description without control or bidi characters but with its
//! lines, and branch names with any such character shown as U+FFFD, so a hostile name can't pass
//! for another branch (and git, which lacks that branch, refuses to check it out).

use super::{Issue, PullRequest, Reviewer};
use crate::text::{is_bidi_control, printable, printable_lines};

/// `p` with every text field cleaned.
pub(super) fn pr(p: &mut PullRequest) {
    p.title = printable(&p.title);
    p.description = printable_lines(&p.description);
    p.source = branch(&p.source);
    p.target = branch(&p.target);
    p.author = printable(&p.author);
    p.merge_status = p.merge_status.as_deref().map(printable);
    reviewers(&mut p.reviewers);
}

pub(super) fn reviewers(list: &mut [Reviewer]) {
    for r in list {
        r.name = printable(&r.name);
        r.state = printable(&r.state);
    }
}

/// `i` with every text field cleaned (its key stays: it is checked where it is used in a link).
pub(super) fn issue(i: &mut Issue) {
    i.title = printable(&i.title);
    i.state = printable(&i.state);
    i.kind = printable(&i.kind);
    i.assignee = i.assignee.as_deref().map(printable);
}

/// A branch name with each whitespace, control or bidi character replaced by U+FFFD.
fn branch(name: &str) -> String {
    name.chars().map(|c| if c.is_whitespace() || c.is_control() || is_bidi_control(c) { '\u{fffd}' } else { c }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pull_request_is_cleaned_in_every_text_field() {
        let mut p = PullRequest {
            id: 7,
            title: "Fix\u{202e}gpj.exe\n\x1b[31m it ".into(),
            description: "Line one\r\n\x07Line\u{2066} two".into(),
            source: "feat/\u{202e}x y".into(),
            target: "main\t".into(),
            author: "mal\u{200f}lory\r\n".into(),
            draft: false,
            reviewers: vec![Reviewer { name: "eve\u{1b}".into(), state: "approved\n".into(), required: false }],
            head: None,
            head_repo: Default::default(),
            merge_status: Some("clean\u{202a}".into()),
            checks: None,
            url: String::new(),
            facets: Default::default(),
        };
        pr(&mut p);
        assert_eq!(p.title, "Fixgpj.exe [31m it");
        assert_eq!(p.description, "Line one\nLine two");
        assert_eq!(p.source, "feat/\u{fffd}x\u{fffd}y");
        assert_eq!(p.target, "main\u{fffd}");
        assert_eq!(p.author, "mallory");
        assert_eq!(p.merge_status.as_deref(), Some("clean"));
        assert_eq!((p.reviewers[0].name.as_str(), p.reviewers[0].state.as_str()), ("eve", "approved"));
    }

    #[test]
    fn an_issue_is_cleaned_but_keeps_its_key() {
        let mut i = Issue {
            key: "PROJ-1".into(),
            title: "\u{2067}Title\r\n".into(),
            state: "Open\u{7}".into(),
            kind: "Bug\u{202e}".into(),
            assignee: Some("a\u{200e}b".into()),
            url: String::new(),
            facets: Default::default(),
        };
        issue(&mut i);
        assert_eq!((i.key.as_str(), i.title.as_str(), i.state.as_str(), i.kind.as_str()), ("PROJ-1", "Title", "Open", "Bug"));
        assert_eq!(i.assignee.as_deref(), Some("ab"));
    }
}
