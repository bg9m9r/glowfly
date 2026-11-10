//! The unposted text of pull requests whose panel closed or gave way to another view (or
//! another pull request), by pull request id with the hosting service it belongs to, kept while
//! the tab is open and restored when the pull request opens again; and settling a kept post's
//! answer. Pure; update/pull_request/ keeps, restores and settles.

use super::actions::PrForms;
use super::line_comment::LineDraft;
use super::{PrAction, PrTab, PrView};
use glowfly_core::forge::Target;
use std::collections::HashMap;

/// Whether the forms or the line comment hold text typed and not posted: a comment, a review's
/// summary, a thread reply or a comment on lines. The merge's title and message are filled in,
/// so they don't count.
pub fn unposted_text(forms: &PrForms, line: Option<&LineDraft>) -> bool {
    let typed = |x: &str| !x.trim().is_empty();
    typed(&forms.comment) || typed(&forms.review_body) || forms.replies.values().any(|r| typed(r)) || line.is_some_and(|d| typed(&d.text))
}

/// What a closed panel leaves behind: its forms, its line comment, the action it was waiting
/// for and the tab it was on.
#[derive(Clone, Debug)]
struct Kept {
    forms: PrForms,
    line_draft: Option<LineDraft>,
    busy: Option<PrAction>,
    tab: PrTab,
}

impl Kept {
    fn typed(&self) -> bool {
        unposted_text(&self.forms, self.line_draft.as_ref())
    }
}

/// The unposted text of pull requests whose panel closed, by pull request id with the service
/// it belongs to. Only text worth keeping is held, and a change of service drops the other
/// service's, so it holds at most one entry per pull request the user wrote in this tab.
#[derive(Clone, Debug, Default)]
pub struct KeptPrDrafts(HashMap<u64, (Target, Kept)>);

impl KeptPrDrafts {
    /// Keeps the text of panel `v` (of service `host`), which is closing; a panel with nothing
    /// typed, or one without a service, replaces what was kept for its pull request with nothing.
    pub fn keep(&mut self, host: Option<&Target>, v: PrView) {
        match host {
            Some(host) if unposted_text(&v.forms, v.line_draft.as_ref()) => {
                let kept = Kept { forms: v.forms, line_draft: v.line_draft, busy: v.busy, tab: v.tab };
                self.0.insert(v.pr.id, (host.clone(), kept));
            }
            _ => {
                self.0.remove(&v.pr.id);
            }
        }
    }

    /// Gives panel `v` (of service `host`), which is opening, the text kept for its pull
    /// request, taking it out; an entry for another service stays.
    pub fn restore(&mut self, host: &Target, v: &mut PrView) {
        match self.0.remove(&v.pr.id) {
            Some((t, k)) if &t == host => {
                v.forms = k.forms;
                v.line_draft = k.line_draft;
                v.busy = k.busy;
                v.tab = k.tab;
            }
            Some(other) => {
                self.0.insert(v.pr.id, other);
            }
            None => {}
        }
    }

    /// The answer to `action` on pull request `id` of `host`, for its kept text while it waits
    /// for that action: posted text goes, a failed line comment keeps its text with the reason
    /// (`result`'s error), and an entry left with nothing typed goes.
    pub fn settle(&mut self, host: &Target, id: u64, action: &PrAction, result: Result<(), String>) {
        let Some((_, k)) = self.0.get_mut(&id).filter(|(t, k)| t == host && k.busy.as_ref() == Some(action)) else { return };
        k.busy = None;
        match (action, result) {
            (PrAction::Comment, Ok(())) => k.forms.comment.clear(),
            (PrAction::Review, Ok(())) => k.forms.review_body.clear(),
            (PrAction::Reply(t), Ok(())) => {
                k.forms.replies.remove(t);
            }
            (PrAction::LineComment, Ok(())) => k.line_draft = None,
            (PrAction::LineComment, Err(e)) => {
                if let Some(d) = k.line_draft.as_mut() {
                    d.posting = None;
                    d.error = Some(e);
                }
            }
            _ => {}
        }
        if !k.typed() {
            self.0.remove(&id);
        }
    }

    /// Drops the text kept for every service but `host`.
    pub fn retain_host(&mut self, host: Option<&Target>) {
        self.0.retain(|_, (t, _)| Some(&*t) == host);
    }

    /// Whether any kept entry has text, which closing the tab would lose.
    pub fn any_typed(&self) -> bool {
        self.0.values().any(|(_, k)| k.typed())
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

    /// The forms, line comment and awaited action kept for pull request `id`.
    #[cfg(any(test, feature = "test-support"))]
    #[doc(hidden)]
    pub fn get(&self, id: u64) -> Option<(&PrForms, Option<&LineDraft>, Option<&PrAction>)> {
        self.0.get(&id).map(|(_, k)| (&k.forms, k.line_draft.as_ref(), k.busy.as_ref()))
    }
}

#[cfg(test)]
mod tests;
