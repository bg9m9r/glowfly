//! The palette's Git Flow entries: Configure, then once the repository is set up Open Git Flow
//! panel, a Start entry per branch kind, a Start from issue entry per kind (while issues are
//! assigned), a Finish entry per open flow branch, and Continue finishing a Finish stopped at a
//! conflict once its merge is committed.

use super::{Entry, PaletteCommand, entry, with_detail};
use crate::state::AppState;
use crate::state::gitflow::issues_ready;
use crate::state::gitflow_resume::{offered, palette_label, steps_left};
use glowfly_core::gitflow::{Branches, FlowKind, flow_branches, short_name, targets};
use glowfly_core::repo::Snapshot;

/// Configure Git Flow, then (once set up) Open Git Flow panel, Start per kind, Start from issue
/// per kind, Finish per open flow branch (the checked-out one first), and Continue finishing
/// while a stopped Finish's merge is committed and no job runs.
pub(super) fn gitflow_entries(st: &AppState, snap: &Snapshot, out: &mut Vec<Entry>) {
    out.push(entry(
        PaletteCommand::ConfigureGitFlow,
        "Configure Git Flow…",
        "gitflow git-flow flow branching model develop feature release hotfix prefixes initialize setup preferences settings",
    ));
    let Some(s) = snap.gitflow.settings() else { return };
    out.push(entry(
        PaletteCommand::OpenGitFlowPanel,
        "Open Git Flow panel",
        "gitflow git-flow panel view overview branches feature release hotfix list",
    ));
    for (k, keywords) in [
        (FlowKind::Feature, "gitflow git-flow new branch feature"),
        (FlowKind::Release, "gitflow git-flow new branch release version"),
        (FlowKind::Hotfix, "gitflow git-flow new branch hotfix fix patch production"),
    ] {
        let e = entry(PaletteCommand::StartGitFlow(k), format!("Start Git Flow {}…", k.noun()), keywords);
        out.push(with_detail(e, Some(format!("from {}", s.base(k)))));
    }
    if let Some(f) = st.session.forge.as_ref().filter(|_| issues_ready(st)) {
        let assigned = format!("{} assigned", f.my_issues.len());
        for k in FlowKind::ALL {
            let label = format!("Start Git Flow {} from {}…", k.noun(), f.issue_noun());
            let keywords = "gitflow git-flow new branch issue ticket work item task bug story";
            out.push(with_detail(entry(PaletteCommand::ChooseGitFlowIssue(k), label, keywords), Some(assigned.clone())));
        }
    }
    for (kind, b) in flow_branches(&s, &Branches::of(snap)) {
        let label = format!("Finish Git Flow {} {}…", kind.noun(), short_name(&s, kind, b));
        let e = entry(PaletteCommand::FinishGitFlow(b.to_owned()), label, "gitflow git-flow finish complete done merge close end");
        out.push(with_detail(e, Some(format!("into {}", targets(&s, kind)))));
    }
    if let Some(r) = offered(st).filter(|_| st.job.is_none()) {
        let keywords = "gitflow git-flow finish continue resume remaining steps merge committed conflict";
        let t = st.settings.lang();
        let e = entry(PaletteCommand::ContinueGitFlowFinish, palette_label(t, &r.resume), keywords);
        out.push(with_detail(e, Some(steps_left(t, r.resume.plan.steps.len()))));
    }
}
