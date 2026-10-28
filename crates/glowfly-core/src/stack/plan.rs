//! Planning a restack (pure): one `git rebase --update-refs --onto` step per leaf of the stack
//! that needs moving, plus one per layer above the first that is itself behind. A step replays
//! the commits after the old base of the first layer it moves, so only the upper layers' own
//! commits are replayed; `--update-refs` moves the middle layers on the way.

use super::{RestackScope, Stacks};
use git2::Oid;
use std::collections::HashSet;

/// `git rebase --update-refs --onto refs/heads/<onto> <upstream> <top>`.
#[derive(Clone, Debug, PartialEq)]
pub struct RestackStep {
    pub onto: String,
    pub upstream: Oid,
    pub top: String,
}

/// The steps of a restack, the branch checked out before (checked out again afterwards, even
/// when the last step leaves HEAD on it, so an abort midway can go back), and whether the
/// uncommitted changes are stashed once before the first step (`autostash`) and put back there
/// at the end. `stash` is that stash while it waits: set on the steps left after a stop.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RestackPlan {
    pub steps: Vec<RestackStep>,
    pub return_to: Option<String>,
    pub autostash: bool,
    pub stash: Option<Oid>,
}

impl RestackPlan {
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// Every branch the steps move: for each step, its top and the layers below it down to (not
    /// including) the branch it goes onto.
    pub fn moved(&self, stacks: &Stacks) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for s in &self.steps {
            let mut at = s.top.as_str();
            for _ in 0..=stacks.links.len() {
                if at == s.onto {
                    break;
                }
                if !out.iter().any(|o| o == at) {
                    out.push(at.to_owned());
                }
                match stacks.below(at) {
                    Some(l) => at = &l.on,
                    None => break,
                }
            }
        }
        out
    }
}

/// The restack `scope` asks for, with `head_branch` the branch checked out now (`None`:
/// detached). Empty when nothing in the scope is behind.
pub fn plan(stacks: &Stacks, scope: &RestackScope, head_branch: Option<&str>, autostash: bool) -> RestackPlan {
    let paths: Vec<Vec<&str>> = match scope {
        RestackScope::Above(b) => paths_from(stacks, b),
        RestackScope::Onto(x) => match stacks.below(x) {
            Some(l) => paths_from(stacks, x).into_iter().map(|p| std::iter::once(l.on.as_str()).chain(p).collect()).collect(),
            None => Vec::new(),
        },
    };
    let mut moved: HashSet<&str> = HashSet::new();
    let mut steps = Vec::new();
    for p in paths.iter().filter(|p| p.len() > 1) {
        let first = (1..p.len()).find(|&j| {
            let behind = stacks.below(p[j]).is_some_and(|l| l.behind);
            !moved.contains(p[j]) && (behind || moved.contains(p[j - 1]))
        });
        let Some(j) = first else { continue };
        // A layer above `j` that is behind its own lower layer starts a step of its own: the
        // lower one's current tip isn't in the range replayed from `j`'s old base, so
        // `--update-refs` wouldn't move it.
        let mut start = j;
        for k in j + 1..=p.len() {
            if k < p.len() && !stacks.below(p[k]).is_some_and(|l| l.behind) {
                continue;
            }
            let Some(link) = stacks.below(p[start]) else { break };
            steps.push(RestackStep { onto: p[start - 1].to_owned(), upstream: link.base, top: p[k - 1].to_owned() });
            start = k;
        }
        moved.extend(&p[j..]);
    }
    let return_to = if steps.is_empty() { None } else { head_branch.map(str::to_owned) };
    RestackPlan { steps, return_to, autostash, stash: None }
}

/// Every path from `root` up to a branch with nothing stacked on it, children in name order;
/// just `[root]` when nothing is stacked on it.
fn paths_from<'a>(stacks: &'a Stacks, root: &'a str) -> Vec<Vec<&'a str>> {
    let mut out = Vec::new();
    let mut todo: Vec<Vec<&'a str>> = vec![vec![root]];
    while let Some(path) = todo.pop() {
        let Some(&last) = path.last() else { continue };
        let children: Vec<&str> = stacks.above(last).iter().map(|l| l.branch.as_str()).filter(|c| !path.contains(c)).collect();
        if children.is_empty() {
            out.push(path);
            continue;
        }
        // Reversed, so the first child is popped (and its paths listed) first.
        for c in children.into_iter().rev() {
            let mut next = path.clone();
            next.push(c);
            todo.push(next);
        }
    }
    out
}
