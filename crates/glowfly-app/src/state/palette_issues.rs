//! The palette's issue entries: Create issue… whenever the repository has a tracker (with the
//! reason as its detail while creating is blocked) or is on Bitbucket Data Center (which has no
//! tracker of its own: the detail says to choose Jira), and View issue… while the tracker has
//! issue details in Glowfly and issues are listed.

use super::{Entry, PaletteCommand, entry, with_detail};
use crate::state::AppState;
use crate::state::issue_form::create_blocked;
use glowfly_core::forge::{Provider, issues_in_app};

const CREATE_WORDS: &str = "issue new ticket bug task story create report file add";

pub(in crate::state) fn issue_entries(st: &AppState, out: &mut Vec<Entry>) {
    let Some(f) = st.forge.as_ref() else { return };
    if f.tracker.is_none() {
        if f.host.as_ref().is_some_and(|h| h.provider == Provider::BitbucketDataCenter) {
            out.push(with_detail(entry(PaletteCommand::CreateIssue, "Create issue…", CREATE_WORDS), create_blocked(st)));
        }
        return;
    }
    let noun = f.issue_noun();
    let e = entry(PaletteCommand::CreateIssue, format!("Create {noun}…"), CREATE_WORDS);
    out.push(with_detail(e, create_blocked(st)));
    let listed = listed(st);
    if can_view(st) {
        let e = entry(PaletteCommand::ChooseIssueToView, format!("View {noun}…"), "issue open show details ticket bug task story read");
        out.push(with_detail(e, Some(format!("{listed} listed"))));
    }
}

/// How many issues the ISSUES section lists: assigned ones and the shown filter's results.
fn listed(st: &AppState) -> usize {
    st.forge.as_ref().map_or(0, |f| crate::state::palette::issues_to_view(f).len())
}

/// Whether View issue… can be offered: a tracker with issue details in Glowfly, and issues
/// listed.
pub fn can_view(st: &AppState) -> bool {
    st.forge.as_ref().and_then(|f| f.tracker.as_ref()).is_some_and(|t| issues_in_app(t.provider)) && listed(st) > 0
}
