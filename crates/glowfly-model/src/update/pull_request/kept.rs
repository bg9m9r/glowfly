//! Settling the answer to a post sent from a pull request panel that has since closed: the text
//! kept for it (in the active tab, or in the parked tab showing the repository) loses what was
//! posted, or keeps it after a failure.

use crate::effect::ForgeError;
use crate::pull_request::PrReply;
use crate::state::AppState;
use crate::state::pull_request::PrAction;
use glowfly_core::forge::Target;
use std::path::Path;

/// The action `reply` answers and how it went; None for the page and the checks.
pub(super) fn action_of(reply: &PrReply) -> Option<(PrAction, Result<(), &ForgeError>)> {
    let (action, error) = match reply {
        PrReply::Loaded(_) | PrReply::Checks(_) => return None,
        PrReply::Commented(r) => (PrAction::Comment, r.as_ref().err()),
        PrReply::Reviewed(_, r) => (PrAction::Review, r.as_ref().err()),
        PrReply::Merged { result, .. } => (PrAction::Merge, result.as_ref().err()),
        PrReply::RepliedToThread { thread, result } => (PrAction::Reply(thread.clone()), result.as_ref().err()),
        PrReply::Resolved { thread, result, .. } => (PrAction::Resolve(thread.clone()), result.as_ref().err()),
        PrReply::LineCommented { result, .. } => (PrAction::LineComment, result.as_ref().err()),
    };
    Some((action, error.map_or(Ok(()), Err)))
}

/// The answer to `action` on pull request `id` of `target` in `workdir` (`active`: the active
/// tab's repository and service), for the text kept for it there. Nothing changes for a pull
/// request whose panel is open: opening it took its kept text out.
pub(super) fn settle(
    st: &mut AppState,
    workdir: &Path,
    target: &Target,
    active: bool,
    id: u64,
    action: PrAction,
    result: Result<(), String>,
) {
    let session = if active {
        Some(&mut st.session)
    } else {
        st.tabs
            .iter_mut()
            .find(|t| t.is(workdir))
            .and_then(|t| t.parked.as_mut())
            .map(|p| &mut p.session)
            .filter(|s| s.forge.as_ref().and_then(|f| f.host.as_ref()) == Some(target))
    };
    if let Some(s) = session {
        s.pr_drafts.settle(target, id, &action, result);
    }
}
