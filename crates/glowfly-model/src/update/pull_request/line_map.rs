//! Placing the pull request panel's code comment threads on the local version of their files:
//! when the files were loaded from a source tip that isn't the service's head, their lines are
//! mapped in the background (`PrEffect::LineMaps`), asked again whenever the key changes, and
//! a reply for an older key, a closed panel or a parked tab is dropped.

use crate::effect::Effect;
use crate::pull_request::{LineMapJob, PrEffect};
use crate::state::AppState;
use crate::state::pull_request_line_map::{LineMaps, wanted};
use glowfly_core::repo::PrLineMaps;

/// Asks for the maps of the key wanted now, unless they were asked for that key already
/// (`again` asks anyway: the reply was dropped while the tab was parked). Without a key (the
/// files are of the head, or not loaded) nothing is kept.
pub(super) fn sync(st: &mut AppState, fx: &mut Vec<Effect>, again: bool) {
    let workdir = st.workdir();
    let Some(v) = st.session.pull_request.view.as_mut() else { return };
    let Some(key) = wanted(v) else {
        v.lines = LineMaps::None;
        return;
    };
    if !again && v.lines.key() == Some(&key) {
        return;
    }
    let Some(workdir) = workdir else { return };
    st.session.pull_request.lines_gen += 1;
    let generation = st.session.pull_request.lines_gen;
    let Some(v) = st.session.pull_request.view.as_mut() else { return };
    v.lines = LineMaps::Loading(key.clone());
    let job = LineMapJob {
        workdir,
        id: v.pr.id,
        head: key.head,
        local: key.local,
        base: key.base,
        target: key.target,
        paths: key.paths,
        generation,
    };
    fx.push(Effect::PullRequest(PrEffect::LineMaps(job)));
}

/// The runner's maps, for the panel still waiting for them with the same generation.
pub(super) fn mapped(st: &mut AppState, id: u64, generation: u64, result: Result<PrLineMaps, String>) {
    if generation != st.session.pull_request.lines_gen {
        return; // superseded: asked again, closed, or parked with its tab
    }
    let Some(v) = st.session.pull_request.view.as_mut().filter(|v| v.pr.id == id) else { return };
    let LineMaps::Loading(key) = &v.lines else { return };
    let key = key.clone();
    match result {
        Ok(maps) => v.lines = LineMaps::Loaded { key, maps },
        Err(error) => {
            let text = format!("Couldn't place {}'s comments on the local lines: {error}", v.label);
            v.lines = LineMaps::Failed { key, error };
            st.log_quiet(false, text);
        }
    }
}
