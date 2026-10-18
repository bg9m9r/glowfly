//! Cherry-picking and reverting from the graph: the job labels, and the form that picks a merge
//! commit's mainline parent (`Modal::Mainline`).

use git2::Oid;
use trellis_core::repo::{RefKind, Snapshot};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PickOp {
    CherryPick,
    Revert,
}

impl PickOp {
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

/// Choosing which parent of a merge commit a cherry-pick or revert counts its changes against.
#[derive(Clone, PartialEq, Debug)]
pub struct MainlineForm {
    pub op: PickOp,
    pub oid: Oid,
    pub summary: String,
    pub parents: Vec<ParentChoice>,
    /// Index into `parents` (the mainline is `choice + 1`).
    pub choice: usize,
}

impl MainlineForm {
    /// The form for merge commit `oid`; `None` unless it is a graph row with two or more parents.
    pub fn new(op: PickOp, oid: Oid, snap: &Snapshot) -> Option<Self> {
        let c = &snap.commits[*snap.row_of.get(&oid)?];
        if c.parents.len() < 2 {
            return None;
        }
        let parents = c
            .parents
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
            .collect();
        Some(Self { op, oid, summary: c.summary.clone(), parents, choice: 0 })
    }
}
