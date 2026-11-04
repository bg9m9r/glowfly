//! The palette's stack entries for the checked-out branch in a stack: Push the stack, and with a
//! code host create its pull requests. Each is listed only when it can run now.

use super::{Entry, PaletteCommand, entry};
use crate::msg::{Msg, StackMsg};
use crate::state::AppState;
use crate::state::stack_prs::prs_blocker;
use crate::state::stack_push::{layers, push_blocker};
use glowfly_core::repo::Snapshot;

#[derive(Clone, Debug, PartialEq)]
pub enum StackCommand {
    /// Push every layer of this branch's stack.
    Push(String),
    /// Create the pull requests of this branch's stack.
    CreatePrs(String),
}

impl StackCommand {
    pub fn msg(&self) -> Msg {
        Msg::Stack(match self {
            Self::Push(b) => StackMsg::Push { branch: b.clone() },
            Self::CreatePrs(b) => StackMsg::CreatePrs { branch: b.clone() },
        })
    }
}

/// Push the stack of {head} and Create {noun}s for the stack of {head}….
pub(super) fn entries(st: &AppState, snap: &Snapshot, out: &mut Vec<Entry>) {
    let Some(head) = snap.head_branch.as_deref() else { return };
    if layers(&snap.stacks, head).is_empty() {
        return;
    }
    let t = st.settings.lang();
    if push_blocker(t, snap, head).is_none() {
        let c = PaletteCommand::Stack(StackCommand::Push(head.to_owned()));
        out.push(entry(c, format!("Push the stack of {head}"), "stacked branches layers upload publish push all"));
    }
    if let Some(fs) = st.session.forge.as_ref().filter(|f| f.host.is_some())
        && prs_blocker(t, snap, fs, head).is_none()
    {
        let c = PaletteCommand::Stack(StackCommand::CreatePrs(head.to_owned()));
        let label = format!("Create {}s for the stack of {head}…", fs.pr_noun());
        out.push(entry(c, label, "stacked branches layers pull merge request pr mr review open new"));
    }
}
