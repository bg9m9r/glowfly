//! A compact copy of the index's entries, kept with a status read so the next index change can
//! be compared with it: which paths' entries changed (`IndexMemo::changed`), and so which rows
//! of the staged and unstaged lists can differ. Each entry is compared by path, stage, mode,
//! object id, skip-worktree and intent-to-add; stat data is left out, so a rewrite that only
//! refreshes it (or the extensions) changes nothing.

use git2::{Index, IndexEntryExtendedFlag, Oid};
use std::cmp::Ordering;

/// The most entries a memo holds; a larger index gives no memo, and its changes read the whole
/// status.
pub const MAX_INDEX_MEMO: usize = 1_000_000;

/// The index entries a status read was taken from, sorted by (path bytes, stage).
#[derive(Clone, PartialEq, Eq)]
pub struct IndexMemo {
    entries: Vec<MemoEntry>,
}

#[derive(Clone, PartialEq, Eq)]
struct MemoEntry {
    path: Box<[u8]>,
    id: Oid,
    mode: u32,
    stage: u8,
    /// Skip-worktree and intent-to-add, from the extended flags.
    bits: u16,
}

impl MemoEntry {
    fn key(&self) -> (&[u8], u8) {
        (&self.path, self.stage)
    }
}

impl std::fmt::Debug for IndexMemo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "IndexMemo({} entries)", self.entries.len())
    }
}

/// The paths whose entries differ between two memos.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IndexDelta {
    /// The changed paths that are UTF-8, sorted, each once.
    pub paths: Vec<String>,
    /// A changed entry has a conflict stage (1-3) on either side.
    pub conflicted: bool,
    /// A changed path isn't UTF-8 (it isn't in `paths`).
    pub not_utf8: bool,
}

impl IndexMemo {
    /// The memo of `index`; None past `MAX_INDEX_MEMO` entries.
    pub fn of(index: &Index) -> Option<IndexMemo> {
        Self::of_with(index, MAX_INDEX_MEMO)
    }

    pub(crate) fn of_with(index: &Index, max: usize) -> Option<IndexMemo> {
        if index.len() > max {
            return None;
        }
        let kept = (IndexEntryExtendedFlag::SKIP_WORKTREE | IndexEntryExtendedFlag::INTENT_TO_ADD).bits();
        let mut entries: Vec<MemoEntry> = index
            .iter()
            .map(|e| MemoEntry {
                stage: ((e.flags >> 12) & 3) as u8,
                bits: e.flags_extended & kept,
                path: e.path.into_boxed_slice(),
                id: e.id,
                mode: e.mode,
            })
            .collect();
        // Under core.ignorecase libgit2 may iterate in folded order: the memo keeps its own.
        entries.sort_unstable_by(|a, b| a.key().cmp(&b.key()));
        Some(IndexMemo { entries })
    }

    /// How many entries the memo holds.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The paths whose entries differ from this memo to `newer`: an entry added, removed or with
    /// another mode, object id, skip-worktree or intent-to-add.
    pub fn changed(&self, newer: &IndexMemo) -> IndexDelta {
        let mut delta = IndexDelta::default();
        let mut note = |e: &MemoEntry| {
            delta.conflicted |= e.stage > 0;
            match std::str::from_utf8(&e.path) {
                Ok(p) => delta.paths.push(p.to_owned()),
                Err(_) => delta.not_utf8 = true,
            }
        };
        let (mut old, mut new) = (self.entries.iter().peekable(), newer.entries.iter().peekable());
        loop {
            match (old.peek(), new.peek()) {
                (None, None) => break,
                (Some(a), None) => {
                    note(a);
                    old.next();
                }
                (None, Some(b)) => {
                    note(b);
                    new.next();
                }
                (Some(a), Some(b)) => match a.key().cmp(&b.key()) {
                    Ordering::Less => {
                        note(a);
                        old.next();
                    }
                    Ordering::Greater => {
                        note(b);
                        new.next();
                    }
                    Ordering::Equal => {
                        if a != b {
                            note(a);
                            note(b);
                        }
                        old.next();
                        new.next();
                    }
                },
            }
        }
        delta.paths.sort();
        delta.paths.dedup();
        delta
    }
}

#[cfg(test)]
#[path = "index_memo_tests.rs"]
mod tests;
