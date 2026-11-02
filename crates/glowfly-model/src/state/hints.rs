//! Tips in context: one short hint at a time, in the pane its feature lives in. The ids that
//! are saved (`HintId::id`), the pure rules for offering one (`offer`), retiring it for good
//! (`retire`) and hiding it unanswered (`ignore`, `place_gone`), and the trigger tests (WIP
//! summaries, two amends, a hosting service with no account). update/hints.rs calls them and
//! saves; `view` is what the hint row draws. Nothing is collected or sent.

use super::AppState;
use super::tabs::TabId;
use crate::keymap::BindingId;
use git2::{Oid, RepositoryState};
use glowfly_core::forge;
use glowfly_core::repo::Snapshot;

/// A tip, with its stable saved name (`id`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HintId {
    /// After a discard, reset or branch delete: Undo and the operation journal.
    Undo,
    /// WIP commits or two amends in a row: tidy them before pushing.
    TidyCommits,
    /// A repository on a hosting service with no account: connect it.
    Connect,
}

/// Where a hint's row sits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    /// Under the graph's column header.
    Graph,
    /// Under the left panel's Remotes section.
    LeftPanel,
}

impl HintId {
    pub const ALL: [HintId; 3] = [HintId::Undo, HintId::TidyCommits, HintId::Connect];

    /// The name saved in `hints_seen` and `hints_ignored`; never change one.
    pub fn id(self) -> &'static str {
        match self {
            HintId::Undo => "undo",
            HintId::TidyCommits => "tidy-commits",
            HintId::Connect => "connect",
        }
    }

    pub fn place(self) -> Place {
        match self {
            HintId::Undo | HintId::TidyCommits => Place::Graph,
            HintId::Connect => Place::LeftPanel,
        }
    }
}

/// The hint showing, and the tab it shows in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShownHint {
    pub id: HintId,
    pub tab: TabId,
}

/// A tab's commits this session, for the Tidy hint (in `RepoSession`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CommitRun {
    /// Amends of HEAD in a row; a plain commit starts again from 0.
    pub amends_in_row: u8,
    /// A commit finished: check the Tidy hint once the snapshot after it loads.
    pub tidy_check: bool,
}

/// The active tab's id, if there is a tab.
fn active_tab(st: &AppState) -> Option<TabId> {
    st.tabs.get(st.active_tab).map(|t| t.id)
}

/// Whether something keeps a new hint away: a dialog, Preferences, a job, or a conflict (files
/// conflicted, or a merge, rebase, cherry-pick or revert in progress) in the active tab.
pub fn blocked(st: &AppState) -> bool {
    st.modal.is_some() || st.prefs.is_some() || st.job.is_some() || st.snap.as_ref().is_some_and(in_conflict)
}

fn in_conflict(snap: &Snapshot) -> bool {
    use RepositoryState::*;
    let busy =
        matches!(snap.state, Merge | Rebase | RebaseInteractive | RebaseMerge | CherryPick | CherryPickSequence | Revert | RevertSequence);
    busy || super::rebase::has_conflicts(snap)
}

/// Whether `id` may show now (`offer`'s rules, without showing it).
fn may_offer(st: &AppState, id: HintId) -> bool {
    let o = &st.onboarding;
    st.settings.hints
        && !st.settings.hints_seen.contains(id.id())
        && !o.quiet_session
        && o.hints_shown_this_session == 0
        && o.hint.is_none()
        && !blocked(st)
        && active_tab(st).is_some()
        && (id.place() != Place::LeftPanel || st.settings.show_left_panel)
}

/// Shows `id` in the active tab when every rule allows it; otherwise the offer is dropped, not
/// queued. Returns whether it shows.
pub fn offer(st: &mut AppState, id: HintId) -> bool {
    let Some(tab) = active_tab(st).filter(|_| may_offer(st, id)) else { return false };
    st.onboarding.hint = Some(ShownHint { id, tab });
    st.onboarding.hints_shown_this_session += 1;
    true
}

/// Retires `id` for good (acted on, dismissed, its feature used, or ignored twice) and hides
/// it if it shows. Returns whether the records changed, so the settings need saving.
pub fn retire(st: &mut AppState, id: HintId) -> bool {
    if st.onboarding.hint.is_some_and(|h| h.id == id) {
        st.onboarding.hint = None;
    }
    st.settings.hints_seen.insert(id.id().to_owned())
}

/// Hides the hint showing without an answer: the first time it is recorded as ignored, the
/// second time it retires. Returns whether the records changed.
pub fn ignore(st: &mut AppState) -> bool {
    let Some(h) = st.onboarding.hint.take() else { return false };
    if st.settings.hints_ignored.contains(h.id.id()) { retire(st, h.id) } else { st.settings.hints_ignored.insert(h.id.id().to_owned()) }
}

/// Whether the hint showing has lost its place: another tab is active, its tab or repository
/// closed, or (Connect) the left panel is hidden.
pub fn place_gone(st: &AppState) -> bool {
    let Some(h) = st.onboarding.hint else { return false };
    active_tab(st) != Some(h.tab) || st.snap.is_none() || (h.id.place() == Place::LeftPanel && !st.settings.show_left_panel)
}

/// A summary that marks a work-in-progress commit: it starts with "wip" as a word, or with
/// "fixup! ", "squash! " or "amend! " (any case).
fn wip_summary(summary: &str) -> bool {
    let lower = summary.to_lowercase();
    if ["fixup! ", "squash! ", "amend! "].iter().any(|p| lower.starts_with(p)) {
        return true;
    }
    lower.strip_prefix("wip").is_some_and(|rest| !rest.starts_with(|c: char| c.is_alphanumeric() || c == '_'))
}

/// The current branch's commits strictly ahead of its upstream along first parents, newest
/// first, with their summaries (`Snapshot::head_ahead`, which stops at the first pushed commit).
/// None when the branch has no upstream, the answer is unknown, or one of those commits isn't in
/// the loaded history: no tip then.
fn ahead_commits(snap: &Snapshot) -> Option<Vec<(Oid, &str)>> {
    let ahead = snap.head_ahead.as_ref()?;
    ahead.iter().map(|&oid| snap.row_of.get(&oid).and_then(|&i| snap.commits.get(i)).map(|c| (oid, &*c.summary))).collect()
}

/// The oldest first-parent commit strictly ahead of the upstream: the Tidy hint's rebase
/// starts there, so it never reaches a pushed commit.
pub fn tidy_base(snap: &Snapshot) -> Option<Oid> {
    ahead_commits(snap)?.last().map(|(oid, _)| *oid)
}

/// Whether the Tidy hint applies after a commit: the branch has an upstream and either 3 or
/// more commits are ahead with at least 2 WIP summaries, or the last two commits were amends.
pub fn tidy_due(snap: &Snapshot, run: CommitRun) -> bool {
    let Some(ahead) = ahead_commits(snap) else { return false };
    let wips = ahead.iter().filter(|(_, s)| wip_summary(s)).count();
    (ahead.len() >= 3 && wips >= 2) || (run.amends_in_row >= 2 && !ahead.is_empty())
}

/// The hosting service of the active repository when no account is connected for it: its name
/// and whether it says "merge request".
pub fn unconnected_service(st: &AppState) -> Option<(&'static str, bool)> {
    let host = st.session.forge.as_ref()?.host.as_ref()?;
    if forge::is_connected(&host.account(), &st.settings.connections) {
        return None;
    }
    Some((host.provider.name(), host.provider.pr_noun() == "merge request"))
}

/// What the hint row draws.
#[derive(Clone, Debug, PartialEq)]
pub struct HintView {
    pub id: HintId,
    pub place: Place,
    pub text: String,
    pub button: &'static str,
}

/// The hint showing in the active tab, worded.
pub fn view(st: &AppState) -> Option<HintView> {
    let h = st.onboarding.hint.filter(|h| active_tab(st) == Some(h.tab))?;
    let t = st.settings.lang();
    let (text, button) = match h.id {
        HintId::Undo => match st.keymap.hint(BindingId::Undo) {
            Some(key) => (t.trf("hints.undo.text", &[("key", &key)]), t.tr("hints.undo.button")),
            None => (t.tr("hints.undo.text_unbound").to_owned(), t.tr("hints.undo.button")),
        },
        HintId::TidyCommits => (t.tr("hints.tidy.text").to_owned(), t.tr("hints.tidy.button")),
        HintId::Connect => {
            let (service, mr) = unconnected_service(st)?;
            let args: &[(&str, &dyn std::fmt::Display)] = &[("service", &service)];
            let text = if mr { t.trf("hints.connect.text_mr", args) } else { t.trf("hints.connect.text", args) };
            (text, t.tr("hints.connect.button"))
        }
    };
    Some(HintView { id: h.id, place: h.id.place(), text, button })
}

#[cfg(test)]
mod tests {
    use super::wip_summary;

    #[test]
    fn wip_summaries() {
        for s in ["wip", "WIP: x", "wip x", "Wip.", "fixup! a", "squash! a", "amend! a", "FIXUP! a"] {
            assert!(wip_summary(s), "{s}");
        }
        for s in ["wipe", "wip_x", "Fix the wip flag", "fixup!a", "squash a"] {
            assert!(!wip_summary(s), "{s}");
        }
    }
}
