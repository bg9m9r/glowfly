//! What the graph's rows ask for while they draw, applied once the frame's rows are done: the
//! commits whose change counts are wanted (`RepoMsg::WantStats`), the commit whose ref chip the
//! pointer is over (its tip shows next frame) and the branch tips whose build checks were shown.

use git2::Oid;
use glowfly_model::msg::{Msg, RepoMsg};
use glowfly_model::state::ViewState;
use glowfly_model::state::build_status::MAX_SEEN;

/// The rows' asks this frame.
#[derive(Default)]
pub(super) struct Asks {
    /// Commits drawn with the CHANGES column but no counts read yet.
    pub want_stats: Vec<Oid>,
    /// The commit whose ref chip the pointer is over.
    pub hover_tip_next: Option<Oid>,
    /// Branch tips whose build checks were looked up.
    pub tips_seen: Vec<Oid>,
}

impl Asks {
    /// Records the asks in `view` and asks update for the wanted change counts.
    pub fn apply(self, view: &mut ViewState, msgs: &mut Vec<Msg>) {
        if self.hover_tip_next.is_some() {
            view.hover_tip_next = self.hover_tip_next;
        }
        let seen = &mut view.tips_seen;
        for o in self.tips_seen {
            if seen.len() < MAX_SEEN && !seen.contains(&o) {
                seen.push(o);
            }
        }
        if !self.want_stats.is_empty() {
            msgs.push(Msg::Repo(RepoMsg::WantStats(self.want_stats)));
        }
    }
}
