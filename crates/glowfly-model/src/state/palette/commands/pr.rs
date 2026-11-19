//! The palette's pull request entries for the checked-out branch: show its open pull request in
//! the panel, or else create one and push the branch first when the code host lacks it or some of
//! its commits. The stack's entries follow (palette/commands/stack.rs).

use super::{Entry, PaletteCommand, entry};
use crate::state::AppState;
use crate::state::push_pr;
use glowfly_core::repo::Snapshot;

/// The branch's pull request entries, then its stack's.
pub(super) fn pr_entries(st: &AppState, snap: &Snapshot, out: &mut Vec<Entry>) {
    branch_entries(st, snap, out);
    crate::state::palette::commands::stack::entries(st, snap, out);
}

/// Show {noun} {pr} for {head} while it has an open pull request; otherwise Create {noun} for
/// {head}… and, when the branch needs a push, Push {head} and create a {noun}….
fn branch_entries(st: &AppState, snap: &Snapshot, out: &mut Vec<Entry>) {
    let Some(fs) = st.session.forge.as_ref().filter(|f| f.host.is_some()) else { return };
    let Some(b) = snap.head_info() else { return };
    if let Some(pr) = push_pr::open_pr(snap, fs, &b.name) {
        let label = format!("Show {} {} for {}", fs.pr_noun(), fs.pr_label(pr.id), b.name);
        out.push(entry(PaletteCommand::ShowPr(pr.id), label, "pull merge request pr mr review open new start"));
        return;
    }
    let (name, noun) = (&b.name, fs.pr_noun());
    out.push(entry(
        PaletteCommand::CreatePr(name.clone()),
        format!("Create {noun} for {name}…"),
        "pull merge request pr mr review open new start",
    ));
    if push_pr::push_item(st.settings.lang(), snap, fs).is_ok() {
        out.push(entry(
            PaletteCommand::PushAndCreatePr(name.clone()),
            format!("Push {name} and create a {noun}…"),
            "pull merge request pr mr upload publish push start",
        ));
    }
}
