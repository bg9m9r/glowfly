//! Checks and votes for every pull request on services that read them one pull request at a
//! time: the list carries them for the first `CHECKED_PRS`, then the rest are asked for in
//! chunks, one request at a time, each filled in as it returns (state/pr_details.rs).

use crate::effect::{Effect, ForgeCall, ForgeError};
use crate::state::{AppState, ForgeState};
use glowfly_core::forge::{CHECKED_PRS, DETAILS_PER_CALL, PrDetails, Provider};
use std::collections::HashSet;
use std::time::Instant;

/// A new pull request list: show what was read before as placeholders and queue the pull
/// requests whose details are missing, out of date or for another head.
pub(super) fn list_arrived(fs: &mut ForgeState, now: Instant) {
    if !fs.host_provider().is_some_and(Provider::details_per_pr) {
        fs.details.clear();
        return;
    }
    let split = fs.prs.len().min(CHECKED_PRS);
    let (first, rest) = fs.prs.split_at_mut(split);
    for pr in first.iter() {
        // The list read these with their votes merged in; kept as checks placeholders, and read
        // again should the pull request move past the first CHECKED_PRS.
        fs.details.cache.insert(pr.id, (PrDetails { checks: pr.checks.clone(), ..PrDetails::empty(pr) }, None));
    }
    for pr in rest.iter_mut() {
        if let Some((d, _)) = fs.details.cache.get(&pr.id) {
            pr.apply(d);
        }
    }
    // A chunk in flight is checked again when it returns.
    let d = &fs.details;
    let queue = rest.iter().filter(|p| !d.asking.contains(&p.id) && d.stale(p, now)).map(|p| p.id).collect();
    fs.details.queue = queue;
    let listed: HashSet<u64> = fs.prs.iter().map(|p| p.id).collect();
    fs.details.cache.retain(|id, _| listed.contains(id));
}

/// Asks about the next chunk, unless one is in flight or nothing is left.
pub(super) fn send_next(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(fs) = st.forge.as_mut() else { return };
    let Some(host) = fs.host.clone() else { return };
    if fs.details.in_flight || fs.details.queue.is_empty() {
        return;
    }
    let mut chunk = Vec::new();
    while chunk.len() < DETAILS_PER_CALL {
        let Some(id) = fs.details.queue.pop_front() else { break };
        if let Some(pr) = fs.prs.iter().find(|p| p.id == id) {
            chunk.push(pr.clone());
        }
    }
    if chunk.is_empty() {
        return;
    }
    let (loading, asking) = (fs.loading, chunk.iter().map(|p| p.id).collect());
    super::forge::request(st, host, false, ForgeCall::PrDetails(chunk), fx);
    let Some(fs) = st.forge.as_mut() else { return };
    if fs.loading > loading {
        fs.details.in_flight = true;
        fs.details.asking = asking;
    } else {
        // Held back (an untrusted repository, a host that may not be contacted).
        fs.details.queue.clear();
    }
}

/// A chunk's answer, filled in as it comes; true when a listed pull request changed. A failure
/// (or an answer cut short by one) stops the pass until the next refresh, and never changes the
/// host's status or what is shown.
pub(super) fn replied(fs: &mut ForgeState, result: Result<Vec<PrDetails>, ForgeError>, key: &str, now: Instant) -> bool {
    fs.details.in_flight = false;
    let asking = std::mem::take(&mut fs.details.asking);
    let list = match result {
        Ok(list) => list,
        Err(e) => {
            tracing::debug!("pull request details: {}", e.message);
            fs.details.queue.clear();
            if e.auth {
                fs.auth.remove(key);
            }
            return false;
        }
    };
    let cut_short = list.len() < asking.len();
    if cut_short {
        tracing::debug!("pull request details: {} of {} read", list.len(), asking.len());
        fs.details.queue.clear();
    }
    let mut changed = false;
    for d in list {
        // One that left the list while it was read is dropped.
        if let Some(pr) = fs.prs.iter_mut().find(|p| p.id == d.id) {
            pr.apply(&d);
            changed = true;
            fs.details.record(d, now);
        }
    }
    if !cut_short {
        // A list that arrived meanwhile may have moved one of them to a new head.
        let moved: Vec<u64> = fs.prs.iter().filter(|p| asking.contains(&p.id) && fs.details.stale(p, now)).map(|p| p.id).collect();
        fs.details.queue.extend(moved);
    }
    changed
}
