//! Building the journal's rows from reflog entries and notes (pure): rebase steps dropped, HEAD
//! entries that repeat a branch's folded into it, each note attached to the state it labels
//! (or a row of its own), newest first, capped, and each subject's newest row marked current
//! when it is still where that subject points.

use super::events::{Event, parse};
use super::notes::Note;
use super::reflog::RawEntry;
use super::{MAX_ROWS, Row, Subject};
use git2::Oid;

/// How far apart (seconds) a HEAD entry and a branch entry for the same commit may be and still
/// be one move.
const FOLD_SECS: i64 = 2;

/// The rows for `entries` and `notes` (oldest first), and whether they were cut at `MAX_ROWS`.
/// `head` is HEAD's commit and `tips` each local branch's.
pub(super) fn build(entries: Vec<RawEntry>, notes: Vec<Note>, head: Option<Oid>, tips: &[(String, Oid)]) -> (Vec<Row>, bool) {
    let parsed: Vec<(RawEntry, Event)> = entries
        .into_iter()
        .filter(|e| !e.new.is_zero())
        .map(|e| {
            let ev = parse(&e.message);
            (e, ev)
        })
        .filter(|(_, ev)| !ev.hidden())
        .collect();
    // A checkout moves HEAD alone, and creating, resetting or renaming a branch moves the
    // branch alone, so neither is the other half of one move.
    let branch_only = |ev: &Event| matches!(ev, Event::BranchCreated { .. } | Event::BranchReset { .. } | Event::BranchRenamed { .. });
    let folds = |e: &RawEntry, ev: &Event| {
        e.subject == Subject::Head
            && !matches!(ev, Event::Checkout { .. })
            && parsed
                .iter()
                .any(|(b, bev)| b.subject != Subject::Head && b.new == e.new && (b.at - e.at).abs() <= FOLD_SECS && !branch_only(bev))
    };
    let keep: Vec<bool> = parsed.iter().map(|(e, ev)| !folds(e, ev)).collect();
    let mut rows: Vec<(Row, usize)> = parsed
        .into_iter()
        .zip(keep)
        .filter(|(_, k)| *k)
        .map(|((e, event), _)| {
            let row = Row {
                subject: e.subject,
                oid: e.new,
                prev: e.old,
                at: e.at,
                event,
                message: e.message,
                note: None,
                summary: None,
                current: false,
            };
            (row, e.index)
        })
        .collect();
    for note in notes {
        for r in &note.refs {
            let Some(oid) = r.oid() else { continue };
            let subject = r.branch.clone().map_or(Subject::Head, Subject::Branch);
            let found = rows
                .iter_mut()
                .filter(|(row, _)| row.subject == subject && row.oid == oid && row.at <= note.at && row.note.is_none())
                .max_by_key(|(row, i)| (row.at, std::cmp::Reverse(*i)));
            match found {
                Some((row, _)) => row.note = Some(note.clone()),
                None => rows.push((
                    Row {
                        subject,
                        oid,
                        prev: None,
                        at: note.at,
                        event: Event::Other,
                        message: String::new(),
                        note: Some(note.clone()),
                        summary: None,
                        current: false,
                    },
                    0,
                )),
            }
        }
    }
    rows.sort_by(|(a, ai), (b, bi)| b.at.cmp(&a.at).then_with(|| a.subject.cmp(&b.subject)).then_with(|| ai.cmp(bi)));
    let capped = rows.len() > MAX_ROWS;
    rows.truncate(MAX_ROWS);
    let mut seen: Vec<&Subject> = Vec::new();
    let mut current = Vec::new();
    for (i, (row, _)) in rows.iter().enumerate() {
        if seen.contains(&&row.subject) {
            continue;
        }
        seen.push(&row.subject);
        let tip = match &row.subject {
            Subject::Head => head,
            Subject::Branch(b) => tips.iter().find(|(n, _)| n == b).map(|(_, o)| *o),
        };
        if tip == Some(row.oid) {
            current.push(i);
        }
    }
    for i in current {
        rows[i].0.current = true;
    }
    (rows.into_iter().map(|(r, _)| r).collect(), capped)
}
