//! Stacked branches: a local branch built on another local branch rather than straight on the
//! main branch. Recognising them is pure (`link`: each branch's first-parent history, read up
//! to the main branch, meets another branch's tip or one of its former tips) on what `read`
//! gathers with libgit2 (the walked commits outside the base, and the former tips from each
//! branch's reflog). A link through a former tip is behind: the lower branch was amended,
//! rebased or gained commits, so the branches above it need a restack. `plan` turns a request
//! into `git rebase --update-refs --onto` steps, one per leaf that needs moving (and one per
//! layer above it that is itself behind), and `restack` runs them through gitcli (so hooks,
//! signing and config apply), resuming after a stop. `push` sends every layer through gitcli
//! with `git push --porcelain` (`porcelain` builds the commands and reads the answer), leasing
//! only layers rewritten locally and never pushing one that is behind or diverged.

use git2::Oid;

mod link;
mod plan;
mod porcelain;
mod push;
mod read;
mod restack;

pub use link::{BranchTip, StackInput, link};
pub use plan::{RestackPlan, RestackStep, plan};
pub use porcelain::{commands as push_commands, parse as parse_porcelain};
pub use push::{LayerOutcome, LayerPushed, MAX_LEASE_REFLOG, PushPlan, PushTarget, StackPush, classify, push};
pub use read::read;
pub use restack::{RestackRun, restack, resume};

/// Why a restack refuses to run when the stack no longer matches its plan.
pub const STALE_PLAN: &str = "The stack changed since this restack was planned. Try again.";
/// The most commits outside the base that recognition walks.
pub const MAX_WALK: usize = 2000;
/// The most reflog entries per branch read for its former tips.
pub const MAX_REFLOG: usize = 50;

/// Which local branches are stacked on which, sorted by branch name.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stacks {
    pub links: Vec<StackLink>,
}

/// `branch` is stacked on `on`: built on `base`, a commit of `on` (its tip, or a former tip
/// when `behind`).
#[derive(Clone, Debug, PartialEq)]
pub struct StackLink {
    pub branch: String,
    pub on: String,
    pub base: Oid,
    /// `base` is no longer `on`'s tip: the branch needs a restack.
    pub behind: bool,
}

impl Stacks {
    /// The link of `branch` to the branch it is stacked on.
    pub fn below(&self, branch: &str) -> Option<&StackLink> {
        self.links.iter().find(|l| l.branch == branch)
    }

    /// The branches stacked directly on `branch`, in name order.
    pub fn above(&self, branch: &str) -> Vec<&StackLink> {
        self.links.iter().filter(|l| l.on == branch).collect()
    }

    /// Every branch stacked on `branch`, directly or not, depth first.
    pub fn descendants(&self, branch: &str) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        let mut todo: Vec<&str> = self.above(branch).iter().rev().map(|l| l.branch.as_str()).collect();
        while let Some(b) = todo.pop() {
            if b == branch || out.contains(&b) {
                continue;
            }
            out.push(b);
            todo.extend(self.above(b).iter().rev().map(|l| l.branch.as_str()));
        }
        out
    }

    /// How many layers sit below `branch` (0 for a branch that isn't stacked).
    pub fn depth(&self, branch: &str) -> usize {
        let mut seen: Vec<&str> = vec![branch];
        let mut at = branch;
        while let Some(l) = self.below(at) {
            if seen.contains(&l.on.as_str()) {
                break;
            }
            seen.push(&l.on);
            at = &l.on;
        }
        seen.len() - 1
    }

    pub fn is_empty(&self) -> bool {
        self.links.is_empty()
    }
}

/// What a restack moves: the branches stacked on a branch, or a branch (and the ones above it)
/// onto the tip of the branch it is stacked on.
#[derive(Clone, Debug, PartialEq)]
pub enum RestackScope {
    Above(String),
    Onto(String),
}

#[cfg(test)]
mod link_tests;
#[cfg(test)]
mod plan_tests;
#[cfg(test)]
mod porcelain_tests;
#[cfg(test)]
mod push_tests;
#[cfg(test)]
mod read_tests;
#[cfg(test)]
mod restack_tests;
