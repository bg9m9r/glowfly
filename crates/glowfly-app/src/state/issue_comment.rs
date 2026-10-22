//! The issue panel's comment box (update/issue_comment.rs, ui/issue/comment_box.rs): its draft,
//! why its Comment button is disabled, the draft's own problem, and a quoted comment added to
//! the draft. Pure.

use super::issue::IssueView;
use super::touched::Touched;
use glowfly_core::forge::{comment_problem, comment_text};

/// The draft's field name, for its touched state.
pub const FIELD: &str = "comment";
/// Why Comment and Reply are disabled while a comment is being posted.
pub const POSTING: &str = "Posting the comment…";

/// The comment box: its draft, whether it was edited, whether it is being posted, and the last
/// post's failure.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CommentDraft {
    pub text: String,
    pub touched: Touched,
    pub posting: bool,
    pub error: Option<String>,
}

/// Why the draft can't be posted now, in this order: the details haven't loaded, a post is in
/// flight, the repository isn't trusted, or the draft is blank or too long.
pub fn comment_blocked(v: &IssueView, trusted: bool) -> Option<String> {
    if v.loaded().is_none() {
        return Some(format!("The {} hasn't loaded yet", v.noun));
    }
    if v.comment.posting {
        return Some(POSTING.to_owned());
    }
    if !trusted {
        return Some("Trust this repository to comment".to_owned());
    }
    comment_problem(v.provider(), &v.comment.text)
}

/// The problem shown under the field: only a draft that is too long. A blank draft is the
/// button's reason alone, so the same words don't show twice.
pub fn field_problem(v: &IssueView) -> Option<String> {
    if comment_text(&v.comment.text).is_empty() { None } else { comment_problem(v.provider(), &v.comment.text) }
}

/// `draft` with `quote` added at its end, followed by a blank line to write the reply on.
pub fn with_quote(draft: &str, quote: &str) -> String {
    if draft.trim().is_empty() { format!("{quote}\n\n") } else { format!("{}\n\n{quote}\n\n", draft.trim_end()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::ForgeState;
    use crate::state::issue::Details;
    use glowfly_core::forge::{Issue, IssueDetails, Provider, Target};

    fn view() -> IssueView {
        let t =
            Target { provider: Provider::GitHub, host: "github.com".into(), owner: "o".into(), project: String::new(), repo: "r".into() };
        let fs = ForgeState::new(Some(t.clone()), "origin".into(), Some(t));
        let issue = Issue {
            key: "12".into(),
            title: "Crash".into(),
            state: "open".into(),
            kind: String::new(),
            assignee: None,
            url: String::new(),
            facets: Default::default(),
        };
        IssueView::new(issue, &fs).unwrap()
    }

    fn loaded(v: &mut IssueView) {
        let d = IssueDetails {
            issue: v.issue.clone(),
            body: String::new(),
            author: String::new(),
            created: 0,
            assignees: Vec::new(),
            priority: None,
            epic: None,
            comments: Vec::new(),
        };
        v.details = Details::Loaded(Box::new(d));
    }

    #[test]
    fn quotes_go_after_the_draft() {
        assert_eq!(with_quote("", "> a"), "> a\n\n");
        assert_eq!(with_quote(" \n ", "> a"), "> a\n\n");
        assert_eq!(with_quote("Agree  \n", "> a"), "Agree\n\n> a\n\n");
    }

    #[test]
    fn only_a_long_draft_is_the_fields_problem() {
        let mut v = view();
        v.comment.text = "  \n".into();
        assert_eq!(field_problem(&v), None);
        v.comment.text = "x".repeat(65_537);
        assert_eq!(field_problem(&v).as_deref(), Some("Comments on GitHub can have at most 65,536 characters"));
        v.comment.text = "Thanks".into();
        assert_eq!(field_problem(&v), None);
    }

    #[test]
    fn blocked_reasons_in_order() {
        let mut v = view();
        v.comment.posting = true;
        assert_eq!(comment_blocked(&v, false).as_deref(), Some("The issue hasn't loaded yet"));
        loaded(&mut v);
        assert_eq!(comment_blocked(&v, false).as_deref(), Some(POSTING));
        v.comment.posting = false;
        assert_eq!(comment_blocked(&v, false).as_deref(), Some("Trust this repository to comment"));
        assert_eq!(comment_blocked(&v, true).as_deref(), Some("Write a comment first"));
        v.comment.text = "Thanks".into();
        assert_eq!(comment_blocked(&v, true), None);
    }
}
