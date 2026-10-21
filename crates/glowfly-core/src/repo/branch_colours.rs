//! Laying the graph out with its branch colours, and the colour of a branch, ref or commit.
//! The colours are worked out with the layout (the background load, and the app when the work
//! tree turns clean or dirty), so views only read them.

use super::{RefKind, Snapshot, worktrees::worktree_wip_heads};
use crate::branch_colour::{Colouring, Swatch, branch_key, resolve};
use crate::graph::{self, owners};
use git2::Oid;
use std::collections::HashMap;

impl Snapshot {
    /// Lays the graph out again from the commits, the WIP rows and the refs, with each lane's
    /// owner and colour, keeping how colours are picked (`Layout::colouring`).
    pub fn lay_out_graph(&mut self) {
        let colouring = std::mem::take(&mut self.graph.colouring);
        let mut layout = graph::layout(&self.commits, self.head_oid, self.is_dirty(), &worktree_wip_heads(&self.worktrees));
        {
            let remotes = self.remote_names();
            let names = owners::LaneNames {
                tips: self.lane_tips(&remotes),
                head_branch: self.head_branch.as_deref(),
                head: self.head_oid,
                mainline: self.mainline(),
                remotes,
                worktree_branches: self.worktrees.iter().map(|w| w.branch.clone()).collect(),
            };
            layout.owners = owners::assign(&layout, &self.commits, &names);
        }
        layout.swatches = resolve(&layout.owners, &colouring);
        layout.colouring = colouring;
        self.graph = layout;
    }

    /// Colours the graph's owners again with `c` (a pin or the palette's slot count changed),
    /// without laying it out again; later relayouts keep `c`.
    pub fn set_colouring(&mut self, c: Colouring) {
        self.graph.swatches = resolve(&self.graph.owners, &c);
        self.graph.colouring = c;
    }

    fn remote_names(&self) -> Vec<&str> {
        self.remote_info.iter().map(|r| r.name.as_str()).collect()
    }

    /// The branch keys at each commit, best first (HEAD's branch, other local branches, then
    /// keys only on remotes), from the shown labels.
    fn lane_tips(&self, remotes: &[&str]) -> HashMap<Oid, Vec<String>> {
        let mut tips = HashMap::new();
        for (oid, labels) in &self.refs {
            let mut keys: Vec<String> = Vec::new();
            for l in labels {
                let key = match l.kind {
                    RefKind::Local => Some(l.name.as_str()),
                    RefKind::Remote => branch_key(&l.full, remotes),
                    RefKind::Tag | RefKind::Head => None,
                };
                if let Some(k) = key
                    && !keys.iter().any(|have| have == k)
                {
                    keys.push(k.to_owned());
                }
            }
            if !keys.is_empty() {
                tips.insert(*oid, keys);
            }
        }
        tips
    }

    /// The repository's main branch: the home remote's (`origin`, else the first) recorded HEAD
    /// when that branch exists, else `main`, then `master`, when it exists locally or on any
    /// remote; None otherwise.
    pub fn mainline(&self) -> Option<String> {
        let exists = |b: &str| self.local_oid(b).is_some() || self.remotes.iter().any(|(_, list)| list.iter().any(|(n, _)| n == b));
        let names = self.remote_names();
        let home = names.iter().find(|r| **r == "origin").or(names.first());
        let recorded = home.and_then(|h| self.remote_heads.iter().find(|(r, _)| r == h)).map(|(_, b)| b.as_str());
        recorded.into_iter().chain(["main", "master"]).find(|b| exists(b)).map(str::to_owned)
    }

    /// The colour of the branch with key `key` (`branch_colour::branch_key`): the accent for the
    /// main branch, its resolved colour when it is in the graph, else its pin or name's slot.
    pub fn branch_swatch(&self, key: &str) -> Swatch {
        let g = &self.graph;
        if g.owners.mainline.as_deref() == Some(key) {
            return Swatch::Main;
        }
        match g.owners.owner_of_key(key) {
            Some(o) => g.swatch(o),
            None => Swatch::Slot(g.colouring.slot_for(key)),
        }
    }

    /// The colour of the full ref name `full`; None for tags, remote HEADs and other refs.
    pub fn ref_swatch(&self, full: &str) -> Option<Swatch> {
        branch_key(full, &self.remote_names()).map(|k| self.branch_swatch(k))
    }

    /// The checked-out branch's colour; None when HEAD is detached or unborn.
    pub fn head_swatch(&self) -> Option<Swatch> {
        self.head_oid?;
        self.head_branch.as_deref().map(|b| self.branch_swatch(b))
    }

    /// The branch whose line the commit `oid` sits on in the graph, with its colour; None for an
    /// unnamed line or a commit not in the graph.
    pub fn commit_branch(&self, oid: Oid) -> Option<(&str, Swatch)> {
        let o = self.commit_owner(oid)?;
        Some((self.graph.owners.key(o)?, self.graph.swatch(o)))
    }

    /// The colour of the line the commit `oid` sits on; None for a commit not in the graph.
    pub fn commit_swatch(&self, oid: Oid) -> Option<Swatch> {
        self.commit_owner(oid).map(|o| self.graph.swatch(o))
    }

    fn commit_owner(&self, oid: Oid) -> Option<u32> {
        self.graph.owners.commit.get(*self.row_of.get(&oid)?).copied()
    }
}
