//! The rebase panel's rows that recreate merges (label, reset and merge steps): their texts,
//! the actions a row offers and whether it can move, and the subtitle and drop count, which
//! count merges too.

use super::rebase::{PanelSource, RebasePanel};
use git2::Oid;
use glowfly_core::rebase::{RebaseAction, ResetTo, TodoStep, Upstream};
use glowfly_core::repo::Snapshot;

fn short(oid: Oid) -> String {
    oid.to_string()[..7].to_owned()
}

fn count(n: usize, one: &str, many: &str) -> String {
    if n == 1 { format!("1 {one}") } else { format!("{n} {many}") }
}

impl RebasePanel {
    /// Commits and merges the list drops; for the remaining steps, only those not dropped
    /// already.
    pub fn drops(&self) -> usize {
        let dropped = |s: &&TodoStep| s.item().is_some_and(|i| i.action == RebaseAction::Drop);
        let before: Vec<_> = self.original.iter().filter(dropped).filter_map(TodoStep::oid).collect();
        let new = |s: &&TodoStep| self.remaining.is_none() || s.oid().is_some_and(|o| !before.contains(&o));
        self.items.iter().filter(dropped).filter(new).count()
    }

    /// "Rebasing 4 commits of feature onto main" (with "and 1 merge" when the list recreates
    /// merges), or "… from the first commit" for a rebase of the root; for the remaining steps,
    /// see `rebase_remaining::subtitle`.
    pub fn subtitle(&self) -> String {
        if self.source == PanelSource::Remaining {
            return super::rebase_remaining::subtitle(self);
        }
        let Some(plan) = &self.plan else { return "Loading commits…".into() };
        let n = self.items.iter().filter(|s| s.commit().is_some()).count();
        let merges = self.items.iter().filter(|s| s.merge().is_some()).count();
        let mut what = count(n, "commit", "commits");
        if merges > 0 {
            what = format!("{what} and {}", count(merges, "merge", "merges"));
        }
        match plan.upstream {
            Upstream::Root => format!("Rebasing {what} of {} from the first commit", plan.branch),
            Upstream::Commit(_) => format!("Rebasing {what} of {} onto {}", plan.branch, plan.onto_label),
        }
    }

    /// "1a2b3c4 Summary" of a commit of the list, or of the loaded history (`snap`); the short
    /// id alone when neither has it.
    fn described(&self, oid: Oid, snap: Option<&Snapshot>) -> String {
        let listed = self.items.iter().chain(&self.original).filter_map(TodoStep::item).find(|i| i.oid == oid).map(|i| i.summary.clone());
        let loaded = || snap.and_then(|s| s.row_of.get(&oid).and_then(|&r| s.commits.get(r))).map(|c| c.summary.to_string());
        match listed.or_else(loaded) {
            Some(summary) => format!("{} {summary}", short(oid)).trim_end().to_owned(),
            None => short(oid),
        }
    }

    /// The text of a label or reset row ("at-1a2b3c4: marks 1a2b3c4 Summary for a later step",
    /// "Continue from the new base (main)"); empty for other rows.
    pub fn step_text(&self, i: usize, snap: Option<&Snapshot>) -> String {
        let labelled = |name: &str| {
            self.items.iter().find_map(|s| match s {
                TodoStep::Label { name: n, at } if n == name => Some(*at),
                _ => None,
            })
        };
        match self.items.get(i) {
            Some(TodoStep::Label { name, at }) => format!("{name}: marks {} for a later step", self.described(*at, snap)),
            Some(TodoStep::Reset(to)) => match to {
                ResetTo::Onto => match self.plan.as_ref().map(|p| p.onto_label.as_str()).filter(|l| !l.is_empty()) {
                    Some(onto) => format!("Continue from the new base ({onto})"),
                    None => "Continue from the new base".into(),
                },
                ResetTo::Label(name) => match labelled(name) {
                    Some(at) => format!("Continue from {}", self.described(at, snap)),
                    None => format!("Continue from {name}"),
                },
                ResetTo::Commit(o) => format!("Continue from {}, kept where it is", self.described(*o, snap)),
                ResetTo::NewRoot => "Start a new first commit".into(),
            },
            _ => String::new(),
        }
    }

    /// The actions row `i`'s combo box offers: Pick, Reword and Drop for a merge.
    pub fn actions(&self, i: usize) -> &'static [RebaseAction] {
        match self.items.get(i) {
            Some(TodoStep::Merge(_)) => &RebaseAction::MERGE_ACTIONS,
            _ => &RebaseAction::ALL,
        }
    }

    /// Whether row `i` can be dragged or moved: label, reset and merge rows are fixed.
    pub fn movable(&self, i: usize) -> bool {
        self.items.get(i).is_some_and(|s| !s.is_structure())
    }
}
