//! Cherry-picking and reverting from the graph: the job labels, and the form that picks the
//! mainline parent of the merge commits among them (`Modal::Mainline`).

use git2::Oid;
use trellis_core::repo::{RefKind, Snapshot};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PickOp {
    CherryPick,
    Revert,
}

impl PickOp {
    /// The command it runs, as trellis-core names it.
    pub fn core(self) -> trellis_core::pick_ops::Picking {
        match self {
            PickOp::CherryPick => trellis_core::pick_ops::Picking::CherryPick,
            PickOp::Revert => trellis_core::pick_ops::Picking::Revert,
        }
    }

    /// "Cherry-pick" or "Revert": the dialog's button and a one-commit job's label.
    pub fn verb(self) -> &'static str {
        match self {
            PickOp::CherryPick => "Cherry-pick",
            PickOp::Revert => "Revert",
        }
    }
}

/// The job's label: "Cherry-pick", "Cherry-pick 3 commits", "Revert", "Revert 2 commits".
pub fn label(op: PickOp, n: usize) -> String {
    if n == 1 { op.verb().to_owned() } else { format!("{} {n} commits", op.verb()) }
}

/// One parent of the merge, as the dialog lists it.
#[derive(Clone, PartialEq, Debug)]
pub struct ParentChoice {
    pub oid: Oid,
    /// Its summary; `None` when it is beyond the loaded history.
    pub summary: Option<String>,
    /// Its branch and tag names.
    pub labels: Vec<String>,
}

/// One merge among the commits to pick or revert, with its parents.
#[derive(Clone, PartialEq, Debug)]
pub struct MergeParents {
    pub oid: Oid,
    pub summary: String,
    pub parents: Vec<ParentChoice>,
}

/// Choosing which parent of the merge commits a cherry-pick or revert counts their changes
/// against: one number (`-m N`) for every commit of the job.
#[derive(Clone, PartialEq, Debug)]
pub struct MainlineForm {
    pub op: PickOp,
    /// Every commit to pick or revert, in the command's order (oldest first for a cherry-pick,
    /// newest first for a revert).
    pub oids: Vec<Oid>,
    /// The merges among them, in that order (at least one).
    pub merges: Vec<MergeParents>,
    /// The first of them that isn't a merge, if any (named in the limit note).
    pub plain: Option<Oid>,
    /// Index into the offered parents (the mainline is `choice + 1`).
    pub choice: usize,
}

impl MainlineForm {
    /// The form for `oids` (in the command's order); `None` unless one of them is a graph row
    /// with two or more parents.
    pub fn new(op: PickOp, oids: &[Oid], snap: &Snapshot) -> Option<Self> {
        let mut merges = Vec::new();
        let mut plain = None;
        for &oid in oids {
            match snap.row_of.get(&oid).map(|&r| &snap.commits[r]) {
                Some(c) if c.parents.len() > 1 => {
                    merges.push(MergeParents { oid, summary: c.summary.clone(), parents: parent_choices(&c.parents, snap) })
                }
                _ => plain = plain.or(Some(oid)),
            }
        }
        if merges.is_empty() {
            return None;
        }
        Some(Self { op, oids: oids.to_vec(), merges, plain, choice: 0 })
    }

    /// How many parent numbers git accepts for every commit: 1 with a commit that isn't a merge
    /// (git takes `-m N` for it only when N names one of its parents), else the fewest parents
    /// among the merges.
    pub fn choices(&self) -> usize {
        if self.plain.is_some() { 1 } else { self.merges.iter().map(|m| m.parents.len()).min().unwrap_or(1) }
    }

    /// The most parents any of the merges has.
    pub fn most_parents(&self) -> usize {
        self.merges.iter().map(|m| m.parents.len()).max().unwrap_or(1)
    }

    /// Whether mainline `n` (1-based) fits every commit.
    pub fn fits(&self, n: u32) -> bool {
        n >= 1 && n as usize <= self.choices()
    }

    /// One commit (a merge) rather than several.
    pub fn single(&self) -> bool {
        self.oids.len() == 1
    }
}

/// Each of `parents` as the dialog lists it.
fn parent_choices(parents: &[Oid], snap: &Snapshot) -> Vec<ParentChoice> {
    parents
        .iter()
        .map(|&p| ParentChoice {
            oid: p,
            summary: snap.row_of.get(&p).map(|&r| snap.commits[r].summary.clone()),
            labels: snap
                .refs
                .get(&p)
                .map(|refs| refs.iter().filter(|r| r.kind != RefKind::Head).map(|r| r.name.clone()).collect())
                .unwrap_or_default(),
        })
        .collect()
}
