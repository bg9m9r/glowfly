//! The operation journal's view state (`AppState::journal`, parked with its tab) and the pure
//! words it shows: a row's label (its note's, else its event's, else git's raw message), the
//! subject's name, the tip a state is compared with, and the sentence saying what Restore would
//! move (and that only the reflog keeps an unreachable state). Restore's own rules are in
//! `journal_restore.rs`; the Lost commits tab's are in `journal_lost.rs`.

use git2::Oid;
use trellis_core::i18n::Lang;
use trellis_core::journal::{Count, Event, Note, Op, Preview, Row, RowKey, Subject, Timeline};
use trellis_core::repo::Snapshot;

/// The open journal.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct JournalView {
    /// The load in flight or last asked for (`AppState::journal_gen`).
    pub generation: u64,
    pub loading: bool,
    /// The last load's result; `None` until the first arrives.
    pub timeline: Option<Result<Timeline, String>>,
    pub selected: Option<RowKey>,
    pub preview: Option<PreviewState>,
    /// The header's States | Lost commits control; States each time the journal opens.
    pub tab: super::journal_lost::JournalTab,
    /// The Lost commits tab, once it has shown.
    pub lost: Option<super::journal_lost::LostView>,
}

/// The preview of the selected row.
#[derive(Clone, Debug, PartialEq)]
pub struct PreviewState {
    pub key: RowKey,
    pub generation: u64,
    /// `None` while it loads.
    pub result: Option<Result<Preview, String>>,
}

impl JournalView {
    /// The rows of the last good load.
    pub fn rows(&self) -> &[Row] {
        match &self.timeline {
            Some(Ok(t)) => &t.rows,
            _ => &[],
        }
    }

    pub fn row(&self, key: &RowKey) -> Option<&Row> {
        self.rows().iter().find(|r| r.key() == *key)
    }

    pub fn selected_row(&self) -> Option<&Row> {
        self.selected.as_ref().and_then(|k| self.row(k))
    }

    /// The loaded preview of row `key`.
    pub fn preview_of(&self, key: &RowKey) -> Option<&Preview> {
        match &self.preview {
            Some(PreviewState { key: k, result: Some(Ok(p)), .. }) if k == key => Some(p),
            _ => None,
        }
    }

    pub fn capped(&self) -> bool {
        matches!(&self.timeline, Some(Ok(t)) if t.capped)
    }
}

/// "HEAD", or the branch's name.
pub fn subject_name(s: &Subject) -> &str {
    match s {
        Subject::Head => "HEAD",
        Subject::Branch(b) => b,
    }
}

/// What `event` did, or `None` for `Other` (shown as git's raw text) and rebase steps.
pub fn event_label(t: Lang, event: &Event) -> Option<String> {
    Some(match event {
        Event::Commit { .. } => t.tr("journal.event.commit").into(),
        Event::Initial { .. } => t.tr("journal.event.initial").into(),
        Event::Amend { .. } => t.tr("journal.event.amend").into(),
        Event::MergeCommit { .. } => t.tr("journal.event.merge_commit").into(),
        Event::Checkout { to, .. } => t.trf("journal.event.checkout", &[("to", to)]),
        Event::Reset { to } => t.trf("journal.event.reset", &[("to", to)]),
        Event::Merge { from, fast_forward: false } => t.trf("journal.event.merge", &[("from", from)]),
        Event::Merge { from, fast_forward: true } => t.trf("journal.event.fast_forward", &[("from", from)]),
        Event::Pull { .. } => t.tr("journal.event.pull").into(),
        Event::RebaseFinish { onto: Some(onto) } => t.trf("journal.event.rebase", &[("onto", onto)]),
        Event::RebaseFinish { onto: None } => t.tr("journal.event.rebase_plain").into(),
        Event::RebaseAbort => t.tr("journal.event.rebase_abort").into(),
        Event::CherryPick { .. } => t.tr("journal.event.cherry_pick").into(),
        Event::Revert { .. } => t.tr("journal.event.revert").into(),
        Event::BranchCreated { from } => t.trf("journal.event.branch_created", &[("from", from)]),
        Event::BranchReset { to } => t.trf("journal.event.branch_reset", &[("to", to)]),
        Event::BranchRenamed { from } => t.trf("journal.event.branch_renamed", &[("from", from)]),
        Event::Clone => t.tr("journal.event.clone").into(),
        Event::Fetch => t.tr("journal.event.fetch").into(),
        Event::Am { .. } => t.tr("journal.event.am").into(),
        Event::RebaseStep | Event::Other => return None,
    })
}

/// "Before rebasing feature onto main", naming the note's first ref ("HEAD" when detached).
pub fn note_label(t: Lang, note: &Note) -> String {
    let branch = note.refs.first().and_then(|r| r.branch.as_deref()).unwrap_or("HEAD");
    let b: (&str, &dyn std::fmt::Display) = ("branch", &branch);
    match &note.op {
        Op::Rebase { onto: Some(onto) } => t.trf("journal.note.rebase", &[b, ("onto", onto)]),
        Op::Rebase { onto: None } => t.trf("journal.note.rebase_plain", &[b]),
        Op::Rewrite => t.trf("journal.note.rewrite", &[b]),
        Op::Merge { from: Some(from) } => t.trf("journal.note.merge", &[b, ("from", from)]),
        Op::Merge { from: None } => t.trf("journal.note.merge_plain", &[b]),
        Op::Pull => t.trf("journal.note.pull", &[b]),
        Op::CherryPick { commits: Some(c) } => t.trf("journal.note.cherry_pick", &[b, ("commits", c)]),
        Op::CherryPick { commits: None } => t.trf("journal.note.cherry_pick_plain", &[b]),
        Op::Revert { commits: Some(c) } => t.trf("journal.note.revert", &[b, ("commits", c)]),
        Op::Revert { commits: None } => t.trf("journal.note.revert_plain", &[b]),
        Op::Reset { to, .. } => t.trf("journal.note.reset", &[b, ("to", to)]),
        Op::Amend => t.trf("journal.note.amend", &[b]),
        Op::DeleteBranch { name } => t.trf("journal.note.delete_branch", &[("branch", name)]),
        Op::Restore { to, .. } => t.trf("journal.note.restore", &[b, ("to", to)]),
        Op::Undo { what } => t.trf("journal.note.undo", &[("what", what)]),
        Op::Redo { what } => t.trf("journal.note.redo", &[("what", what)]),
    }
}

/// A row's label: its note's, else its event's, else git's raw message.
pub fn row_label(t: Lang, row: &Row) -> String {
    if let Some(n) = &row.note {
        return note_label(t, n);
    }
    event_label(t, &row.event).unwrap_or_else(|| trellis_core::text::printable(&row.message))
}

/// Where `subject` points now: a branch's tip, or HEAD's commit (`None`: gone or unborn).
pub fn against(snap: &Snapshot, subject: &Subject) -> Option<Oid> {
    match subject {
        Subject::Head => snap.head_oid,
        Subject::Branch(b) => snap.locals.iter().find(|l| l.name == *b).and_then(|l| l.oid),
    }
}

pub fn short(oid: Oid) -> String {
    oid.to_string()[..7].to_owned()
}

fn leaving(t: Lang, c: Count) -> String {
    if c.more { t.tr("journal.preview.leaving_more").into() } else { t.trn("journal.preview.leaving", c.n, &[]) }
}

fn arriving(t: Lang, c: Count) -> String {
    if c.more { t.tr("journal.preview.arriving_more").into() } else { t.trn("journal.preview.arriving", c.n, &[]) }
}

/// "Restore moves feature from 9f8e7d6 to 1a2b3c4: 3 commits leave the branch, 1 comes back",
/// with the counts once `preview` has them; "Restore recreates old at 1a2b3c4" for a branch
/// that is gone. `None` when the row is where its subject already is.
pub fn moves_sentence(t: Lang, row: &Row, preview: Option<&Preview>, snap: &Snapshot) -> Option<String> {
    let name = subject_name(&row.subject);
    let to = short(row.oid);
    let Some(from) = against(snap, &row.subject) else {
        return Some(t.trf("journal.preview.recreates", &[("branch", &name), ("to", &to)]));
    };
    if from == row.oid {
        return None;
    }
    let moves = t.trf("journal.preview.moves", &[("branch", &name), ("from", &short(from)), ("to", &to)]);
    let parts: Vec<String> = match preview {
        Some(Preview { leaving: Some(l), arriving: Some(a), .. }) => {
            let mut parts = Vec::new();
            if l.n > 0 {
                parts.push(leaving(t, *l));
            }
            if a.n > 0 {
                parts.push(arriving(t, *a));
            }
            parts
        }
        _ => Vec::new(),
    };
    Some(match parts.as_slice() {
        [] => moves,
        [one] => t.trf("journal.preview.with_count", &[("moves", &moves), ("first", one)]),
        [one, two, ..] => t.trf("journal.preview.with_counts", &[("moves", &moves), ("first", one), ("second", two)]),
    })
}

/// The preview's sentence under an unreachable row's moves sentence.
pub fn unreachable_line(t: Lang) -> &'static str {
    t.tr("journal.preview.unreachable")
}

/// The footer under a list cut at `MAX_ROWS`.
pub fn capped_line(t: Lang) -> &'static str {
    t.tr("journal.capped")
}

#[cfg(test)]
#[path = "journal_tests.rs"]
mod tests;
