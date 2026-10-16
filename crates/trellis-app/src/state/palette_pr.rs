//! The palette's pull request entries for the checked-out branch: create one, and push the
//! branch first when the code host lacks it or some of its commits. Neither is listed once the
//! branch has an open pull request.

use super::{Entry, PaletteCommand, entry};
use crate::state::AppState;
use crate::state::push_pr;
use trellis_core::repo::Snapshot;

/// Create {noun} for {head}… and, when the branch needs a push, Push {head} and create a {noun}….
pub(super) fn pr_entries(st: &AppState, snap: &Snapshot, out: &mut Vec<Entry>) {
    let Some(fs) = st.forge.as_ref().filter(|f| f.host.is_some()) else { return };
    let Some(b) = snap.head_info() else { return };
    if fs.pr_for_branch(&b.name).is_some() {
        return;
    }
    let (name, noun) = (&b.name, fs.pr_noun());
    out.push(entry(
        PaletteCommand::CreatePr(name.clone()),
        format!("Create {noun} for {name}…"),
        "pull merge request pr mr review open new start",
    ));
    if push_pr::push_item(snap, fs).is_ok() {
        out.push(entry(
            PaletteCommand::PushAndCreatePr(name.clone()),
            format!("Push {name} and create a {noun}…"),
            "pull merge request pr mr upload publish push start",
        ));
    }
}
