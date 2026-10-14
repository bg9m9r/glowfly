//! The palette's Git Flow entries: Configure, a Start entry per branch kind and a Finish entry
//! per open flow branch once the repository is set up.

use super::{Entry, PaletteCommand, entry, with_detail};
use trellis_core::gitflow::{Branches, FlowKind, flow_branches, short_name, targets};
use trellis_core::repo::Snapshot;

/// Configure Git Flow, then (once set up) Start per kind and Finish per open flow branch,
/// the checked-out one first.
pub(super) fn gitflow_entries(snap: &Snapshot, out: &mut Vec<Entry>) {
    out.push(entry(
        PaletteCommand::ConfigureGitFlow,
        "Configure Git Flow…",
        "gitflow git-flow flow branching model develop feature release hotfix prefixes initialize setup preferences settings",
    ));
    let Some(s) = snap.gitflow.settings() else { return };
    for (k, keywords) in [
        (FlowKind::Feature, "gitflow git-flow new branch feature"),
        (FlowKind::Release, "gitflow git-flow new branch release version"),
        (FlowKind::Hotfix, "gitflow git-flow new branch hotfix fix patch production"),
    ] {
        let e = entry(PaletteCommand::StartGitFlow(k), format!("Start Git Flow {}…", k.noun()), keywords);
        out.push(with_detail(e, Some(format!("from {}", s.base(k)))));
    }
    for (kind, b) in flow_branches(&s, &Branches::of(snap)) {
        let label = format!("Finish Git Flow {} {}…", kind.noun(), short_name(&s, kind, b));
        let e = entry(PaletteCommand::FinishGitFlow(b.to_owned()), label, "gitflow git-flow finish complete done merge close end");
        out.push(with_detail(e, Some(format!("into {}", targets(&s, kind)))));
    }
}
