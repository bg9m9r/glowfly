//! Rows Glowfly won't act on (`glowfly_core::repo::Hazard`): names that aren't UTF-8, and paths
//! that share one file on disk with another index path. Which messages act on a path, and
//! whether the action destroys something; `update::hazards` refuses them before any effect.

use super::{AppState, Selection};
use crate::msg::{BlameMsg, ConflictMsg, FilesMsg, HistoryMsg, Msg, RepoMsg, StageMsg, StashMsg, WorkFileMsg};
use crate::patch::PatchMsg;
use crate::tools::ToolsMsg;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{Collision, FileEntry, Hazard, HunkOp, Snapshot};

/// The hazard of `path` in any list on screen: the working tree's, the selected commit's, the
/// selected commits' comparison or their stacked lists.
pub fn hazard_of<'a>(st: &'a AppState, path: &str) -> Option<&'a Hazard> {
    let snap = st.snap.iter().flat_map(|s| s.unstaged.iter().chain(&s.staged));
    let detail = st.detail.iter().flat_map(|d| &d.files);
    let compared = match &st.selection {
        Selection::Commits(m) => m.changes.as_ref().and_then(|c| c.as_ref().ok()).map(|c| c.files.as_slice()).unwrap_or_default(),
        Selection::VsWorkdir(w) => w.changes.as_ref().and_then(|c| c.as_ref().ok()).map(|c| c.files.as_slice()).unwrap_or_default(),
        _ => &[],
    };
    let stacked = match &st.selection {
        Selection::Commits(m) => m.stacked.as_ref().and_then(|s| s.as_ref().ok()).map(|s| s.as_slice()).unwrap_or_default(),
        _ => &[],
    };
    let stacked = stacked.iter().flat_map(|c| &c.files);
    snap.chain(detail).chain(compared).chain(stacked).filter(|f: &&FileEntry| f.path == path).find_map(|f| f.hazard.as_ref())
}

/// The path `msg` acts on and whether it destroys contents (discard, delete, restore over the
/// file). A confirmation counts as what it confirms.
pub fn target(msg: &Msg) -> Option<(&str, bool)> {
    Some(match msg {
        Msg::Confirm { then, .. } | Msg::ConfirmDanger { then, .. } => return target(then),
        Msg::Stage(StageMsg::StageFile(p) | StageMsg::UnstageFile(p)) => (p, false),
        Msg::Stage(StageMsg::DiscardFile(p)) => (p, true),
        Msg::Stage(StageMsg::Hunk { path, op, .. }) => (path, *op == HunkOp::Discard),
        Msg::Files(FilesMsg::RestoreFile { path, .. }) => (path, true),
        Msg::Files(FilesMsg::OpenWorkingFile(p) | FilesMsg::ShowWorkingFile(p)) => (p, false),
        Msg::WorkFile(WorkFileMsg::DiscardUntracked(p) | WorkFileMsg::DeleteFile { path: p, .. }) => (p, true),
        Msg::WorkFile(WorkFileMsg::RenameFileDialog(p) | WorkFileMsg::RenameFile { from: p, .. }) => (p, false),
        Msg::Conflict(ConflictMsg::OpenConflict(p) | ConflictMsg::ResolveWithSide { path: p, .. }) => (p, false),
        Msg::History(HistoryMsg::OpenFileHistory { path, .. }) => (path, false),
        Msg::Blame(BlameMsg::Open { path, .. }) => (path, false),
        Msg::Repo(RepoMsg::OpenDiff(t)) => (t.path(), false),
        Msg::Tools(ToolsMsg::OpenFile { path, .. }) => (path, false),
        // Stashing a file puts its HEAD version back on disk.
        Msg::Stash(StashMsg::PushDialog { path: Some(p), .. }) => (p, true),
        Msg::Patch(PatchMsg::ChangesDialog { path: Some(p), .. }) => (p, false),
        Msg::Patch(PatchMsg::ExportFiles { paths, .. }) => (paths.first()?, false),
        _ => return None,
    })
}

/// Why `msg` must not run, if it mustn't: nothing may act on a name that isn't UTF-8 (its
/// shown path names another file or none), and nothing may destroy a file another index path
/// shares.
pub fn refusal(st: &AppState, msg: &Msg) -> Option<String> {
    let (path, destroys) = target(msg)?;
    let t = st.settings.lang();
    match hazard_of(st, path)? {
        h @ Hazard::NotUtf8(_) => Some(h.reason(t)),
        h @ Hazard::Twins(_) if destroys => Some(h.reason(t)),
        Hazard::Twins(_) => None,
    }
}

/// The working-tree banner's lines about paths: index paths that are one name to some file
/// system, and a `core.ignorecase` that disagrees with this one. Empty when all is well.
pub fn warnings(t: Lang, snap: &Snapshot) -> Vec<String> {
    let mut out = Vec::new();
    let (here, elsewhere): (Vec<&Collision>, Vec<&Collision>) =
        snap.collisions.iter().partition(|c| c.shares_a_file(snap.folding, snap.ignorecase));
    let list = |cs: &[&Collision]| {
        let mut names: Vec<String> = cs.iter().take(3).map(|c| c.paths.join(" = ")).collect();
        if cs.len() > 3 {
            names.push(t.trn("files.warnings.more", cs.len() - 3, &[]));
        }
        names.join("; ")
    };
    if !here.is_empty() {
        out.push(t.trf("files.warnings.shared", &[("paths", &list(&here))]));
    }
    if !elsewhere.is_empty() {
        out.push(t.trf("files.warnings.elsewhere", &[("paths", &list(&elsewhere))]));
    }
    out.extend(snap.case_mismatch().map(str::to_owned));
    out
}
