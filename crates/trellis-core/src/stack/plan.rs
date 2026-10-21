//! Planning a restack (pure): one `git rebase --update-refs --onto` step per leaf of the stack
//! that needs moving. A step replays the commits after the old base of the first layer that
//! moves, so only the upper layers' own commits are replayed; `--update-refs` moves the middle
//! layers on the way.

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

/// The steps of a restack, the branch to check out again afterwards, and whether each step
/// stashes uncommitted changes first (`--autostash`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RestackPlan {
    pub steps: Vec<RestackStep>,
    pub return_to: Option<String>,
    pub autostash: bool,
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
        let Some(link) = stacks.below(p[j]) else { continue };
        let top = p[p.len() - 1];
        steps.push(RestackStep { onto: p[j - 1].to_owned(), upstream: link.base, top: top.to_owned() });
        moved.extend(&p[j..]);
    }
    let last_top = steps.last().map(|s: &RestackStep| s.top.as_str());
    let return_to = if steps.is_empty() { None } else { head_branch.filter(|h| Some(*h) != last_top).map(str::to_owned) };
    RestackPlan { steps, return_to, autostash }
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
