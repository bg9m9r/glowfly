//! CI status of branch tips: asking the host about the tips on screen, a few at a time, and
//! recording the answers (state/build_status.rs).

use crate::effect::{Effect, ForgeCall};
use crate::state::build_status::{TIPS_PER_REQUEST, tip_queries};
use crate::state::{AppState, ForgeState, Status};
use git2::Oid;
use std::collections::HashMap;
use std::time::Instant;
use trellis_core::forge::{Checks, TipQuery};

/// The tips drawn in the last frame.
pub(super) fn shown(st: &mut AppState, oids: Vec<Oid>) {
    if let Some(fs) = st.forge.as_mut() {
        fs.tips.visible = oids;
    }
}

/// Asks about the visible tips that are due, when the host is reachable and the last request
/// is far enough behind.
pub(super) fn tick(st: &mut AppState, now: Instant, fx: &mut Vec<Effect>) {
    let (Some(fs), Some(snap)) = (st.forge.as_mut(), st.snap.as_ref()) else { return };
    let Some(host) = fs.host.clone() else { return };
    if fs.host_status != Status::Ok || !fs.tips.may_ask(now) {
        return;
    }
    let due = fs.tips.due(now);
    // Tips that can't be asked about (other remotes, local-only) are dropped before the cap,
    // so they never crowd out ones that can.
    let queries: Vec<(Oid, TipQuery)> = tip_queries(snap, &fs.remote, &due).into_iter().take(TIPS_PER_REQUEST).collect();
    if queries.is_empty() {
        return;
    }
    fs.tips.in_flight.extend(queries.iter().map(|(o, _)| *o));
    fs.tips.last_request = Some(now);
    let call = ForgeCall::TipChecks(queries.into_iter().map(|(_, q)| q).collect());
    let loading = fs.loading;
    super::forge::request(st, host, false, call, fx);
    // Held back (an untrusted repository, a host that may not be contacted): nothing is in flight.
    if let Some(fs) = st.forge.as_mut().filter(|f| f.loading == loading) {
        fs.tips.in_flight.clear();
    }
}

/// The host's answer about `asked`; it never changes the host's status.
pub(super) fn replied(fs: &mut ForgeState, asked: &[TipQuery], result: Result<HashMap<String, Checks>, ()>, now: Instant) {
    fs.tips.record(asked, result, now);
}

/// Pull requests' checks become their head commits' tip status.
pub(super) fn seed_from_prs(st: &mut AppState, now: Instant) {
    let Some(fs) = st.forge.as_mut() else { return };
    for pr in &fs.prs {
        if let (Some(c), Some(o)) = (&pr.checks, fs.pr_heads.get(&pr.id)) {
            fs.tips.seed(*o, c.clone(), now);
        }
    }
}
