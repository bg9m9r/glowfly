//! What undoing (or redoing) a recorded action would do, as the Undo and Redo tooltips'
//! preview: the commits a move takes off, the branch a checkout goes back to, the file a
//! discard brings back. Every `Action` has its own wording; the commits come from the loaded
//! graph when it has them, else their short SHA alone. Pure.

use super::tip::{PREVIEW_ROWS, TipPreview, TipRow, commit_rows, short_sha};
use git2::Oid;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::Snapshot;
use glowfly_core::undo::{Action, Discarded, HeadPos};

#[cfg(test)]
#[path = "undo_preview_tests.rs"]
mod tests;

/// The preview of undoing `a` (`redo` false) or redoing it, in `t`.
pub fn preview(t: Lang, a: &Action, snap: Option<&Snapshot>, redo: bool) -> TipPreview {
    let heading = if redo { t.tr("toolbar.tip.redo_heading") } else { t.tr("toolbar.tip.undo_heading") };
    // The undo text, or the redo one.
    let pick = |undo: String, again: String| TipRow::Line(if redo { again } else { undo });
    let rows = match a {
        Action::Checkout { from, to, created } => {
            let back = head_name(if redo { to } else { from });
            let mut rows = vec![TipRow::Line(t.trf("undo.preview.checkout_again", &[("target", &back)]))];
            if let Some(c) = created {
                let b = [("branch", &c.name as &dyn std::fmt::Display)];
                rows.push(pick(t.trf("undo.preview.delete_created", &b), t.trf("undo.preview.create_again", &b)));
            }
            rows
        }
        Action::Commit { branch, from, to, .. } => {
            let mut rows = commits(snap, &[*to]);
            rows.push(goes(t, branch, *from, *to, redo));
            rows
        }
        Action::Reset { branch, from, to, .. } => {
            let target = if redo { *to } else { *from };
            let args: &[(&str, &dyn std::fmt::Display)] = &[("branch", &branch_name(branch)), ("sha", &short_sha(target))];
            let mut rows = vec![pick(t.trf("undo.preview.moves_back", args), t.trf("undo.preview.moves_again", args))];
            rows.extend(commits(snap, &[target]));
            rows
        }
        Action::SetBranch { name, from, to } => {
            let target = if redo { *to } else { *from };
            let args: &[(&str, &dyn std::fmt::Display)] = &[("branch", name), ("sha", &short_sha(target))];
            let mut rows = vec![pick(t.trf("undo.preview.moves_back", args), t.trf("undo.preview.moves_again", args))];
            rows.extend(commits(snap, &[target]));
            rows
        }
        Action::Moved { branch, from, to, .. } => return moved(t, snap, heading, branch, *from, *to, redo),
        Action::Restacked { moves, .. } => moves
            .iter()
            .take(PREVIEW_ROWS)
            .map(|m| {
                let sha = short_sha(if redo { m.to } else { m.from });
                let args: &[(&str, &dyn std::fmt::Display)] = &[("branch", &m.name), ("sha", &sha)];
                pick(t.trf("undo.preview.moves_back", args), t.trf("undo.preview.moves_again", args))
            })
            .collect(),
        Action::DeleteBranch { name, oid, .. } => {
            let args: &[(&str, &dyn std::fmt::Display)] = &[("branch", name), ("sha", &short_sha(*oid))];
            vec![pick(t.trf("undo.preview.restore_branch", args), t.trf("undo.preview.delete_branch_again", args))]
        }
        Action::CreateBranch { name, oid, .. } => {
            let args: &[(&str, &dyn std::fmt::Display)] = &[("branch", name), ("sha", &short_sha(*oid))];
            vec![pick(t.trf("undo.preview.delete_branch", args), t.trf("undo.preview.create_branch_again", args))]
        }
        Action::CreateTag { name, .. } => {
            let args: &[(&str, &dyn std::fmt::Display)] = &[("tag", name)];
            vec![pick(t.trf("undo.preview.delete_tag", args), t.trf("undo.preview.create_tag_again", args))]
        }
        Action::DiscardAll { .. } => vec![pick(t.tr("undo.preview.restore_changes").into(), t.tr("undo.preview.discard_all_again").into())],
        Action::Discard { path, what, .. } => {
            let reverted = matches!(what, Discarded::RevertedHunk | Discarded::RevertedLines(_) | Discarded::RevertedFile);
            let line = if reverted {
                pick(t.tr("undo.preview.restore_reverted").into(), t.tr("undo.preview.revert_again").into())
            } else {
                pick(t.tr("undo.preview.restore_discarded").into(), t.tr("undo.preview.discard_again").into())
            };
            vec![file('M', path), line]
        }
        Action::Restore { path, oid, .. } => {
            let again = t.trf("undo.preview.restore_file", &[("sha", &short_sha(*oid))]);
            vec![file('M', path), pick(t.tr("undo.preview.unrestore").into(), again)]
        }
        Action::Resolve { path, .. } => {
            vec![file('M', path), pick(t.tr("undo.preview.conflict_back").into(), t.tr("undo.preview.resolve_again").into())]
        }
        Action::DeleteFile { path, .. } => {
            vec![file('D', path), pick(t.tr("undo.preview.undelete").into(), t.tr("undo.preview.delete_file_again").into())]
        }
        Action::UnstageGone { path, .. } => {
            vec![file('A', path), pick(t.tr("undo.preview.restage").into(), t.tr("undo.preview.unstage_again").into())]
        }
        Action::StashPush { message, .. } => {
            vec![TipRow::Line(message.clone()), pick(t.tr("undo.preview.stash_back").into(), t.tr("undo.preview.stash_again").into())]
        }
        Action::StashApply { pop: true, message, .. } => {
            vec![TipRow::Line(message.clone()), pick(t.tr("undo.preview.unpop").into(), t.tr("undo.preview.pop_again").into())]
        }
        Action::StashApply { pop: false, message, .. } => {
            vec![TipRow::Line(message.clone()), pick(t.tr("undo.preview.unapply").into(), t.tr("undo.preview.apply_again").into())]
        }
        Action::AbortKept { .. } => vec![pick(t.tr("undo.preview.abort_back").into(), t.tr("undo.preview.abort_again").into())],
        Action::StashDrop { message, .. } => {
            vec![TipRow::Line(message.clone()), pick(t.tr("undo.preview.restore_stash").into(), t.tr("undo.preview.drop_again").into())]
        }
    };
    TipPreview::new(heading, rows)
}

/// "main goes back to 1a2b3c4" (undo, to `from`) or "main moves to 4d5e6f7" (redo, to `to`).
fn goes(t: Lang, branch: &Option<String>, from: Oid, to: Oid, redo: bool) -> TipRow {
    let name = branch_name(branch);
    TipRow::Line(if redo {
        t.trf("undo.preview.moves_to", &[("branch", &name), ("sha", &short_sha(to))])
    } else {
        t.trf("undo.preview.goes_back", &[("branch", &name), ("sha", &short_sha(from))])
    })
}

/// A merge, pull, cherry-pick, revert, rebase or history edit: the commits it added (on `to`
/// but not `from`), and where the branch goes.
fn moved(t: Lang, snap: Option<&Snapshot>, heading: &str, branch: &Option<String>, from: Oid, to: Oid, redo: bool) -> TipPreview {
    let found = snap.and_then(|s| Some((s, s.divergence(to, from, PREVIEW_ROWS)?)));
    let Some((s, d)) = found else {
        return TipPreview::new(heading, vec![TipRow::Line(short_sha(to)), goes(t, branch, from, to, redo)]);
    };
    let n = d.ahead_total;
    let mut rows = vec![TipRow::Line(if redo { t.trn("undo.preview.puts_back", n, &[]) } else { t.trn("undo.preview.takes_off", n, &[]) })];
    rows.extend(commit_rows(s, &d.ahead));
    let more = n.saturating_sub(d.ahead.len());
    if more > 0 {
        rows.push(TipRow::Line(t.trn("toolbar.tip.more", more, &[])));
    }
    rows.push(goes(t, branch, from, to, redo));
    let note = (!d.complete).then(|| t.tr("toolbar.tip.not_loaded").to_owned());
    TipPreview::new(heading, rows).note(note)
}

/// Rows for `oids`: commit rows from the loaded history, or their short SHAs without one.
fn commits(snap: Option<&Snapshot>, oids: &[Oid]) -> Vec<TipRow> {
    match snap {
        Some(s) => commit_rows(s, oids),
        None => oids.iter().map(|o| TipRow::Line(short_sha(*o))).collect(),
    }
}

fn file(status: char, path: &str) -> TipRow {
    TipRow::File { status, path: path.to_owned() }
}

fn branch_name(branch: &Option<String>) -> String {
    branch.clone().unwrap_or_else(|| "HEAD".to_owned())
}

fn head_name(h: &HeadPos) -> String {
    match h {
        HeadPos::Branch(b) => b.clone(),
        HeadPos::Detached(o) => short_sha(*o),
    }
}
