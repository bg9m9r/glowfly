//! Text from a hosting service's API made safe to show, once, as it comes in (from `links`,
//! which every pull request, issue and detail result passes through): titles, names, states,
//! labels and milestones printable (`text::printable`), a description without control, bidi or
//! invisible characters but with its lines, and branch names and issue keys with any such
//! character shown as U+FFFD, so a hostile name can't pass for another branch or issue (and the
//! branch or key, which the service and git lack, is refused where it is used).

use super::{Issue, PullRequest, Reviewer};
use crate::text::{is_bidi_control, is_invisible, printable, printable_lines};

/// `p` with every text field cleaned.
pub(super) fn pr(p: &mut PullRequest) {
    p.title = printable(&p.title);
    p.description = printable_lines(&p.description);
    p.source = name(&p.source);
    p.target = name(&p.target);
    p.author = printable(&p.author);
    p.merge_status = p.merge_status.as_deref().map(printable);
    reviewers(&mut p.reviewers);
    facets(&mut p.facets.labels, &mut p.facets.milestone);
}

/// Labels and a milestone made printable.
fn facets(labels: &mut [String], milestone: &mut Option<String>) {
    for l in labels {
        *l = printable(l);
    }
    *milestone = milestone.as_deref().map(printable);
}

pub(super) fn reviewers(list: &mut [Reviewer]) {
    for r in list {
        r.name = printable(&r.name);
        r.state = printable(&r.state);
    }
}

/// `i` with every text field cleaned.
pub(super) fn issue(i: &mut Issue) {
    i.key = name(&i.key);
    facets(&mut i.facets.labels, &mut i.facets.milestone);
    i.title = printable(&i.title);
    i.state = printable(&i.state);
    i.kind = printable(&i.kind);
    i.assignee = i.assignee.as_deref().map(printable);
}

/// A branch name or issue key with each whitespace, control, bidi or invisible character
/// replaced by U+FFFD.
fn name(s: &str) -> String {
    s.chars().map(|c| if c.is_whitespace() || c.is_control() || is_bidi_control(c) || is_invisible(c) { '\u{fffd}' } else { c }).collect()
}

#[cfg(test)]
mod tests {
    use super::super::{IssueFacets, PrFacets};
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
            facets: PrFacets {
                labels: vec!["bug\u{202e}".into(), "ui\u{200b}".into()],
                milestone: Some("v1\u{feff}\n".into()),
                ..Default::default()
            },
        };
        pr(&mut p);
        assert_eq!(p.title, "Fixgpj.exe [31m it");
        assert_eq!(p.description, "Line one\nLine two");
        assert_eq!(p.source, "feat/\u{fffd}x\u{fffd}y");
        assert_eq!(p.target, "main\u{fffd}");
        assert_eq!(p.author, "mallory");
        assert_eq!(p.merge_status.as_deref(), Some("clean"));
        assert_eq!((p.reviewers[0].name.as_str(), p.reviewers[0].state.as_str()), ("eve", "approved"));
        assert_eq!(p.facets.labels, ["bug", "ui"]);
        assert_eq!(p.facets.milestone.as_deref(), Some("v1"));
    }

    #[test]
    fn an_issue_is_cleaned_in_every_text_field() {
        let mut i = Issue {
            key: "PROJ-1".into(),
            title: "\u{2067}Title\r\n".into(),
            state: "Open\u{7}".into(),
            kind: "Bug\u{202e}".into(),
            assignee: Some("a\u{200e}b".into()),
            url: String::new(),
            facets: IssueFacets { labels: vec!["\u{2066}urgent\x07".into()], milestone: Some("Sprint\u{200d} 4".into()) },
        };
        issue(&mut i);
        assert_eq!((i.key.as_str(), i.title.as_str(), i.state.as_str(), i.kind.as_str()), ("PROJ-1", "Title", "Open", "Bug"));
        assert_eq!(i.assignee.as_deref(), Some("ab"));
        assert_eq!(i.facets.labels, ["urgent"]);
        assert_eq!(i.facets.milestone.as_deref(), Some("Sprint 4"));
    }

    #[test]
    fn a_key_with_hidden_characters_shows_them_and_matches_no_issue() {
        for (raw, shown) in [("PROJ-1\u{202e}", "PROJ-1\u{fffd}"), ("12\u{200b}3", "12\u{fffd}3"), ("7 8", "7\u{fffd}8")] {
            let mut i = Issue {
                key: raw.into(),
                title: String::new(),
                state: String::new(),
                kind: String::new(),
                assignee: None,
                url: String::new(),
                facets: Default::default(),
            };
            issue(&mut i);
            assert_eq!(i.key, shown, "{raw:?}");
        }
    }
}
