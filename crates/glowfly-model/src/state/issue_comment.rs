//! The issue panel's comment box (update/issue_comment.rs, ui/issue/comment_box.rs): its draft,
//! the drafts kept for issues whose panel closed, why its Comment button is disabled, the
//! draft's own problem, and a quoted comment added to the draft. Pure.

use super::issue::IssueView;
use super::touched::Touched;
use glowfly_core::forge::{Target, comment_problem, comment_text};
use std::collections::HashMap;

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

impl CommentDraft {
    /// Whether closing the panel would lose it: it has text, or it is being posted.
    fn worth_keeping(&self) -> bool {
        self.posting || !self.text.trim().is_empty()
    }
}

/// The comment drafts of issues whose panel closed or gave way to another issue, by issue key
/// with the tracker they belong to, kept while the tab is open. Only drafts worth keeping are
/// held, and a tracker change drops the other tracker's, so it holds at most one per issue the
/// user wrote to in this tab.
#[derive(Clone, Debug, Default)]
pub struct KeptDrafts(HashMap<String, (Target, CommentDraft)>);

impl KeptDrafts {
    /// Keeps the draft of panel `v`, which is closing; an empty one replaces nothing kept.
    pub fn keep(&mut self, v: IssueView) {
        if v.comment.worth_keeping() {
            self.0.insert(v.key, (v.target, v.comment));
        } else {
            self.0.remove(&v.key);
        }
    }

    /// The draft kept for issue `key` on `target`, taken out; a new one when there is none.
    pub fn take(&mut self, target: &Target, key: &str) -> CommentDraft {
        match self.0.remove(key) {
            Some((t, d)) if &t == target => d,
            Some(other) => {
                self.0.insert(key.to_owned(), other);
                CommentDraft::default()
            }
            None => CommentDraft::default(),
        }
    }

    /// The kept draft of issue `key` on `target`, while it is being posted.
    pub fn posting_mut(&mut self, target: &Target, key: &str) -> Option<&mut CommentDraft> {
        self.0.get_mut(key).filter(|(t, d)| t == target && d.posting).map(|(_, d)| d)
    }

    pub fn remove(&mut self, key: &str) {
        self.0.remove(key);
    }

    /// Drops the drafts of every tracker but `tracker`.
    pub fn retain_tracker(&mut self, tracker: Option<&Target>) {
        self.0.retain(|_, (t, _)| Some(&*t) == tracker);
    }

    /// Whether any kept draft has text, which closing the tab would lose.
    pub fn any_typed(&self) -> bool {
        self.0.values().any(|(_, d)| !d.text.trim().is_empty())
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    #[cfg(any(test, feature = "test-support"))]
    #[doc(hidden)]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[cfg(any(test, feature = "test-support"))]
    #[doc(hidden)]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
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

    #[test]
    fn kept_drafts_hold_only_written_ones_of_their_tracker() {
        let mut kept = KeptDrafts::default();
        let mut v = view();
        let target = v.target.clone();
        v.comment.text = "  ".into();
        kept.keep(v.clone());
        assert!(kept.is_empty(), "a blank draft isn't kept");
        v.comment.text = "Seen".into();
        kept.keep(v.clone());
        assert_eq!(kept.len(), 1);
        assert!(kept.any_typed());
        let other = Target { owner: "x".into(), ..target.clone() };
        assert_eq!(kept.take(&other, "12"), CommentDraft::default(), "another tracker's issue 12");
        assert_eq!(kept.take(&target, "12").text, "Seen");
        assert!(kept.is_empty(), "taken out");

        kept.keep(v.clone());
        v.comment.text.clear();
        kept.keep(v.clone());
        assert!(kept.is_empty(), "emptied and closed again: nothing is kept");

        v.comment.posting = true;
        kept.keep(v.clone());
        assert!(kept.posting_mut(&target, "12").is_some(), "a draft being posted is kept");
        kept.retain_tracker(Some(&other));
        assert!(kept.is_empty(), "a tracker change drops the old tracker's drafts");
    }
}
