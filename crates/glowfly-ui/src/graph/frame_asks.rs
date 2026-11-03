//! What the graph's rows ask for while they draw, applied once the frame's rows are done: the
//! commits whose change counts are wanted (`RepoMsg::WantStats`), the commit whose ref chip the
//! pointer is over (its tip shows next frame) and the branch tips whose build checks were shown.

use crate::msg::{Msg, RepoMsg};
use crate::state::AppState;
use crate::state::build_status::MAX_SEEN;
use git2::Oid;

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
    /// Records the asks in `st.view` and asks update for the wanted change counts.
    pub fn apply(self, st: &mut AppState, msgs: &mut Vec<Msg>) {
        if self.hover_tip_next.is_some() {
            st.view.hover_tip_next = self.hover_tip_next;
        }
        let seen = &mut st.view.tips_seen;
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
