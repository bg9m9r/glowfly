//! The rebase panel's rows that recreate merges (label, reset and merge steps): their texts,
//! the actions a row offers and whether it can move, the drop count, and the header's texts:
//! the subtitle (which counts merges too), the order line and the tip row after the last step.

use super::rebase::{PanelSource, RebasePanel};
use git2::Oid;
use glowfly_core::i18n::Lang;
use glowfly_core::rebase::{RebaseAction, ResetTo, TodoStep, Upstream};
use glowfly_core::repo::Snapshot;

fn short(oid: Oid) -> String {
    oid.to_string()[..7].to_owned()
}

/// What a rebase goes onto, for the header: a commit (labelled "1a2b3c4 Summary") by its short
/// id, a branch by its name (which has no spaces).
pub(super) fn onto_name(label: &str) -> &str {
    label.split_once(' ').map_or(label, |(sha, _)| sha)
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

    /// The header's second line: "Rebasing 4 commits of feature onto main." (with "and 1 merge"
    /// when the list recreates merges; a commit to rebase onto by its short id alone), or "… from
    /// the first commit." for a rebase of the root; for the remaining steps, see
    /// `rebase_remaining::subtitle`.
    pub fn subtitle(&self, t: Lang) -> String {
        if self.source == PanelSource::Remaining {
            return super::rebase_remaining::subtitle(t, self);
        }
        let Some(plan) = &self.plan else { return "Loading commits…".into() };
        let n = self.items.iter().filter(|s| s.commit().is_some()).count();
        let merges = self.items.iter().filter(|s| s.merge().is_some()).count();
        let mut what = t.trn("rebase.subtitle.commits", n, &[]);
        if merges > 0 {
            let m = t.trn("rebase.subtitle.merges", merges, &[]);
            what = t.trf("rebase.subtitle.and", &[("commits", &what), ("merges", &m)]);
        }
        let branch = &plan.branch;
        match plan.upstream {
            Upstream::Root => t.trf("rebase.subtitle.root", &[("what", &what), ("branch", branch)]),
            Upstream::Commit(_) => {
                let onto = onto_name(&plan.onto_label);
                t.trf("rebase.subtitle.onto", &[("what", &what), ("branch", branch), ("onto", &onto)])
            }
        }
    }

    /// The branch the rebase ends on, or `None` for a detached HEAD.
    pub fn tip_branch(&self) -> Option<&str> {
        match self.source {
            PanelSource::Remaining => self.remaining.as_ref().and_then(|r| r.branch.as_deref()),
            PanelSource::Start(_) => self.plan.as_ref().map(|p| p.branch.as_str()),
        }
    }

    /// The header's third line: the order the list runs in, oldest first, and that its last
    /// step becomes the tip of the branch.
    pub fn order_hint(&self, t: Lang) -> String {
        let remaining = self.source == PanelSource::Remaining;
        match (self.tip_branch(), remaining) {
            (Some(b), false) => t.trf("rebase.order.start", &[("branch", &b)]),
            (Some(b), true) => t.trf("rebase.order.remaining", &[("branch", &b)]),
            (None, _) => t.tr("rebase.order.remaining_detached").to_owned(),
        }
    }

    /// The faint row after the last step: "Tip of main", or "New HEAD" when detached.
    pub fn tip_text(&self, t: Lang) -> String {
        match self.tip_branch() {
            Some(b) => t.trf("rebase.tip.branch", &[("branch", &b)]),
            None => t.tr("rebase.tip.detached").to_owned(),
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

    /// Whether row `i` can be dragged or moved: label, reset and merge rows are fixed, and an
    /// update-ref row only moves with its commit.
    pub fn movable(&self, i: usize) -> bool {
        self.items.get(i).is_some_and(|s| !s.is_structure() && !matches!(s, TodoStep::UpdateRef(_)))
    }
}
