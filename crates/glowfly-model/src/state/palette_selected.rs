//! Command palette entries for the selected commit: Create branch and Create tag at it (or at
//! HEAD with no commit selected), comparing it with the working directory, and cherry-picking
//! or reverting the selected commits.

use super::palette_commands::{Entry, PaletteCommand, entry};
use super::{AppState, Selection};
use glowfly_core::repo::Snapshot;

/// What Create branch and Create tag start at: the selected commit's short sha, else HEAD.
fn at_label(st: &AppState) -> String {
    match &st.session.selection {
        Selection::Commit(oid) => short(*oid),
        _ => "HEAD".to_owned(),
    }
}

fn short(oid: git2::Oid) -> String {
    oid.to_string()[..7].to_owned()
}

/// The selected commit, when it is exactly one graph row (not a stash selected in the sidebar).
fn selected_row(st: &AppState, snap: &Snapshot) -> Option<git2::Oid> {
    match &st.session.selection {
        Selection::Commit(oid) if snap.row_of.contains_key(oid) => Some(*oid),
        _ => None,
    }
}

/// The selected commits, as the graph's menu cherry-picks or reverts them: the one selected
/// commit, or a multi-selection's.
pub(crate) fn selected_commits(st: &AppState) -> Option<Vec<git2::Oid>> {
    match &st.session.selection {
        Selection::Commit(oid) => Some(vec![*oid]),
        Selection::Commits(m) => Some(m.oids.clone()),
        Selection::VsWorkdir(_) | Selection::Wip | Selection::None => None,
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
    if let Some(oids) = selected_commits(st) {
        let t = st.settings.lang();
        let picks = super::selection::cherry_pick_blocker(snap, &oids) != Some(super::selection::CommitBlock::PickOneOnHead);
        if picks {
            out.push(entry(
                C::PickSelected { revert: false },
                t.trn("palette.pick_selected", oids.len(), &[]),
                "cherry-pick copy apply commit",
            ));
        }
        out.push(entry(C::PickSelected { revert: true }, t.trn("palette.revert_selected", oids.len(), &[]), "revert undo inverse commit"));
    }
}
