//! Linking branches into stacks from their first-parent histories (pure). Each branch's line is
//! read newest first from its tip's parent; the nearest commit on it that is another branch's
//! tip (current) or one of its former tips (behind) names the branch it is stacked on.

use super::{StackLink, Stacks};
use git2::Oid;
use std::collections::HashMap;

/// A local branch: its tip and its former tips (newest first, the current one left out).
#[derive(Clone, Debug, PartialEq)]
pub struct BranchTip {
    pub name: String,
    pub tip: Oid,
    pub former: Vec<Oid>,
}

/// What recognition reads: the candidate branches, and the first parent of every walked commit
/// that isn't in the base (the base's commits are not keys, so a line ends where it meets it).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StackInput {
    pub branches: Vec<BranchTip>,
    pub first_parent: HashMap<Oid, Option<Oid>>,
}

/// A link found for one branch, with how far down its line it was.
struct Hit<'a> {
    branch: &'a str,
    on: &'a str,
    base: Oid,
    behind: bool,
    distance: usize,
}

/// The stacks `input` describes. Only branches whose tip was walked (not merged into the base)
/// take part, as upper or lower layers.
pub fn link(input: &StackInput) -> Stacks {
    let fp = &input.first_parent;
    let candidates: Vec<&BranchTip> = input.branches.iter().filter(|b| fp.contains_key(&b.tip)).collect();
    let mut tip_of: HashMap<Oid, Vec<&str>> = HashMap::new();
    let mut former_of: HashMap<Oid, Vec<&str>> = HashMap::new();
    for b in &candidates {
        tip_of.entry(b.tip).or_default().push(&b.name);
        for f in b.former.iter().filter(|f| **f != b.tip) {
            former_of.entry(*f).or_default().push(&b.name);
        }
    }
    for names in tip_of.values_mut().chain(former_of.values_mut()) {
        names.sort_unstable();
        names.dedup();
    }

    let mut hits: Vec<Hit> = Vec::new();
    for x in &candidates {
        let mut at = fp.get(&x.tip).copied().flatten();
        let mut distance = 0;
        // A guard against a malformed map that loops.
        while let Some(c) = at.filter(|c| fp.contains_key(c) && distance <= fp.len()) {
            distance += 1;
            if let Some(on) = other(tip_of.get(&c), &x.name) {
                hits.push(Hit { branch: &x.name, on, base: c, behind: false, distance });
                break;
            }
            if let Some(on) = other(former_of.get(&c), &x.name) {
                hits.push(Hit { branch: &x.name, on, base: c, behind: true, distance });
                break;
            }
            at = fp.get(&c).copied().flatten();
        }
    }

    // Current links can't close a cycle (each lower tip is a strict ancestor); behind links,
    // nearest first, are skipped when they would.
    hits.sort_by(|a, b| (a.behind, a.distance, a.branch).cmp(&(b.behind, b.distance, b.branch)));
    let mut on_of: HashMap<&str, &str> = HashMap::new();
    let mut links: Vec<StackLink> = Vec::new();
    for h in hits {
        if h.behind && reaches(&on_of, h.on, h.branch) {
            continue;
        }
        on_of.insert(h.branch, h.on);
        links.push(StackLink { branch: h.branch.to_owned(), on: h.on.to_owned(), base: h.base, behind: h.behind });
    }
    links.sort_by(|a, b| a.branch.cmp(&b.branch));
    Stacks { links }
}

/// The first of `names` that isn't `x`.
fn other<'a>(names: Option<&Vec<&'a str>>, x: &str) -> Option<&'a str> {
    names?.iter().find(|n| **n != x).copied()
}

/// Whether following the links down from `from` reaches `target`.
fn reaches(on_of: &HashMap<&str, &str>, from: &str, target: &str) -> bool {
    let mut at = from;
    for _ in 0..=on_of.len() {
        if at == target {
            return true;
        }
        match on_of.get(at) {
            Some(next) => at = next,
            None => return false,
        }
    }
    false
}
