//! Command palette entries for the selected commit: Create branch and Create tag at it (or at
//! HEAD with no commit selected), and comparing it with the working directory.

use super::palette_commands::{Entry, PaletteCommand, entry};
use super::{AppState, Selection};
use glowfly_core::repo::Snapshot;

/// What Create branch and Create tag start at: the selected commit's short sha, else HEAD.
fn at_label(st: &AppState) -> String {
    match &st.selection {
        Selection::Commit(oid) => short(*oid),
        _ => "HEAD".to_owned(),
    }
}

fn short(oid: git2::Oid) -> String {
    oid.to_string()[..7].to_owned()
}

/// The selected commit, when it is exactly one graph row (not a stash selected in the sidebar).
fn selected_row(st: &AppState, snap: &Snapshot) -> Option<git2::Oid> {
    match &st.selection {
        Selection::Commit(oid) if snap.row_of.contains_key(oid) => Some(*oid),
        _ => None,
    }
}

pub(super) fn entries(st: &AppState, snap: &Snapshot, out: &mut Vec<Entry>) {
    use PaletteCommand as C;
    if snap.head_oid.is_some() {
        let at = at_label(st);
        out.push(entry(C::CreateBranch, format!("Create branch at {at}…"), "new branch"));
        out.push(entry(C::CreateTag { annotated: false }, format!("Create tag at {at}…"), "new tag release version label"));
        out.push(entry(
            C::CreateTag { annotated: true },
            format!("Create annotated tag at {at}…"),
            "new tag release version message annotate",
        ));
    }
    if let Some(oid) = selected_row(st, snap) {
        let label = format!("Compare {} with working directory", short(oid));
        out.push(entry(C::CompareWithWorkdir(oid), label, "diff working tree directory uncommitted changes since compare commit"));
    }
}
