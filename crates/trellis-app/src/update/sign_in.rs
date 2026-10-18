//! Auto-fetch and remotes that need a sign-in: an auto-fetch that failed because a remote needs
//! the user to sign in (or to confirm its host key) pauses that remote, so auto-fetch skips it
//! instead of failing again and again (and locking the account); the banner's "Fetch <remote>"
//! runs a fetch that can prompt, and any fetch of the remote that works ends the pause.

use super::jobs::{JobSpec, start_job};
use super::notify::AUTO_FETCH;
use crate::effect::Effect;
use crate::state::AppState;
use crate::state::sign_in::{FetchRun, SignInNeeded, log_line};
use std::path::Path;
use trellis_core::auth_failure::{fetch_failures, url_host};
use trellis_core::gitcli::args;
use trellis_core::paths::same_spelling;
use trellis_core::repo::RemoteInfo;

/// Remotes as (name, fetch URL).
type Remotes = Vec<(String, String)>;

/// The auto-fetch command for the active repository and the remotes it covers: `fetch --all`
/// while no remote is paused, else only the active remotes (neither paused nor
/// `skipFetchAll`). None when every remote is paused.
pub(super) fn auto_fetch_cmd(st: &AppState) -> Option<(Vec<String>, Remotes)> {
    let snap = st.snap.as_ref()?;
    let paused = |r: &RemoteInfo| st.sign_in.is_paused(&snap.workdir, &r.name);
    let pair = |r: &RemoteInfo| (r.name.clone(), r.fetch_url().to_owned());
    let prune = st.settings.auto_prune.then(|| "--prune".to_owned());
    if !snap.remote_info.iter().any(paused) {
        let mut cmd = args(&["fetch", "--all"]);
        cmd.extend(prune);
        return Some((cmd, snap.remote_info.iter().filter(|r| !r.skip_fetch_all).map(pair).collect()));
    }
    let active: Remotes = snap.remote_info.iter().filter(|r| !paused(r) && !r.skip_fetch_all).map(pair).collect();
    let mut cmd = args(&["fetch"]);
    match active.len() {
        0 => return None,
        1 => {}
        _ => cmd.push("--multiple".into()),
    }
    cmd.extend(prune);
    cmd.push("--".into());
    cmd.extend(active.iter().map(|(n, _)| n.clone()));
    Some((cmd, active))
}

/// The configured remotes a `fetch` command covers: every one but `skipFetchAll` ones for
/// `--all`, the names given for `--multiple`, else the remote named first.
fn covered(cmd: &[String], remotes: &[RemoteInfo]) -> Remotes {
    let rest = &cmd[1..];
    let pair = |r: &RemoteInfo| (r.name.clone(), r.fetch_url().to_owned());
    if rest.iter().any(|a| a == "--all") {
        return remotes.iter().filter(|r| !r.skip_fetch_all).map(pair).collect();
    }
    let positional: Vec<&String> = match rest.iter().position(|a| a == "--") {
        Some(i) => rest[i + 1..].iter().collect(),
        None => rest.iter().filter(|a| !a.starts_with('-')).collect(),
    };
    let known = |name: &&String| remotes.iter().find(|r| r.name == **name);
    if rest.iter().any(|a| a == "--multiple") {
        positional.iter().filter_map(known).map(pair).collect()
    } else {
        positional.iter().find_map(known).map(pair).into_iter().collect()
    }
}

/// A job started: remembers a fetch in the active repository, so its result can pause or
/// resume the remotes it covered.
pub(super) fn job_started(st: &mut AppState, quiet: bool, label: &str, cmds: &[Vec<String>]) {
    st.sign_in.fetching = None;
    let ([cmd], Some(snap)) = (cmds, st.snap.as_ref()) else { return };
    if cmd.first().map(String::as_str) != Some("fetch") {
        return;
    }
    let remotes = covered(cmd, &snap.remote_info);
    let run = FetchRun { workdir: snap.workdir.clone(), remotes, auto: quiet && label == AUTO_FETCH };
    st.sign_in.fetching = Some(run);
}

/// The job that ran in `workdir` ended. A fetch that worked ends the pauses of the remotes it
/// covered; an auto-fetch that failed for want of a sign-in pauses those remotes, logging each
/// new pause.
pub(super) fn job_finished(st: &mut AppState, workdir: Option<&Path>, ok: bool, output: &str) {
    let Some(run) = st.sign_in.fetching.take() else { return };
    if !workdir.is_some_and(|w| same_spelling(w, &run.workdir)) {
        return;
    }
    if ok {
        for name in st.sign_in.resume(&run.workdir, run.remotes.iter().map(|(n, _)| n.as_str())) {
            st.log_quiet(true, format!("Auto-fetch includes {name} again."));
        }
        return;
    }
    if !run.auto {
        return;
    }
    for f in fetch_failures(output, &run.remotes) {
        let host = run.remotes.iter().find(|(n, _)| *n == f.remote).and_then(|(_, url)| url_host(url));
        let n = SignInNeeded { workdir: run.workdir.clone(), remote: f.remote, host, need: f.need, dismissed: false };
        let line = log_line(&n);
        if st.sign_in.pause(n) {
            st.log(false, line);
        }
    }
}

/// The banner's "Fetch <remote>": a fetch of that remote that can prompt for a sign-in. Ignored
/// unless `workdir` is the active repository and the remote is still configured there.
pub(super) fn fetch(st: &mut AppState, workdir: &Path, remote: &str, fx: &mut Vec<Effect>) {
    let Some(snap) = st.snap.as_ref() else { return };
    if !same_spelling(workdir, &snap.workdir) || !snap.remote_info.iter().any(|r| r.name == remote) {
        return;
    }
    let mut cmd = args(&["fetch"]);
    if st.settings.auto_prune {
        cmd.push("--prune".into());
    }
    cmd.extend(["--".to_owned(), remote.to_owned()]);
    start_job(st, JobSpec::new(format!("Fetch {remote}"), vec![cmd]), fx);
}
