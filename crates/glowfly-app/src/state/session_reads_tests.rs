//! Parking a tab's repository reads keeps the results and drops the requests in flight, and
//! leaves the counters behind past those requests; clearing them (another repository opened)
//! forgets everything.

use super::*;
use crate::conflict_check::CheckRequest;
use crate::state::conflict_check::ConflictReport;
use crate::state::session::RepoSession;
use glowfly_core::conflict_check::{MergeTarget, Prediction, TargetKind};
use std::sync::Arc;

fn oid(n: u8) -> Oid {
    Oid::from_bytes(&[n; 20]).unwrap()
}

/// Reads with a stats entry and one pending, line counts stored for `oid(1)` and on their way for
/// `oid(2)`, and a signature check on its way for `oid(3)`.
fn some_reads() -> RepoReads {
    let mut r = RepoReads::default();
    r.stats.insert(oid(1), (2, 10, 4));
    r.stats_pending.insert(oid(4));
    r.line_counts.finish_commit(oid(1), Arc::default());
    r.line_counts.start_commit(oid(2));
    r.signatures.start(oid(3));
    r
}

#[test]
fn parking_keeps_reads_and_drops_requests_in_flight() {
    let mut active = RepoSession { reads: some_reads(), ..Default::default() };
    let parked = active.park();
    let r = &parked.reads;
    assert_eq!(r.stats.get(&oid(1)), Some(&(2, 10, 4)), "the stats go with the tab");
    assert!(r.stats_pending.is_empty(), "stats on their way are asked for again on return");
    assert!(r.line_counts.commit(oid(1)).is_some(), "stored counts are kept");
    assert!(r.line_counts.wants_commit(oid(2)), "counts on their way are asked for again");
    assert!(r.signatures.wanted(oid(3)), "a check on its way is asked for again");

    let left = &mut active.reads;
    assert!(left.stats.is_empty() && left.stats_pending.is_empty(), "nothing is left for the next tab");
    assert!(left.line_counts.commit(oid(1)).is_none());
    assert!(left.counters().into_iter().all(|c| *c > 0), "the counters stay behind, not reset to 0");
}

/// A conflict check's request (the inputs don't matter here).
fn a_request() -> CheckRequest {
    CheckRequest {
        branch: "main".into(),
        ours: oid(7),
        target: MergeTarget { name: "origin/main".into(), oid: oid(8), kind: TargetKind::Upstream },
    }
}

fn a_report() -> ConflictReport {
    ConflictReport { request: a_request(), result: Ok(Prediction::Clean) }
}

/// The reads' counters, as values.
fn counts(r: &mut RepoReads) -> Vec<u64> {
    r.counters().into_iter().map(|c| *c).collect()
}

#[test]
fn parking_leaves_the_counters_behind() {
    let mut active = RepoSession::default();
    let r = &mut active.reads;
    let sig = r.signatures.start(oid(3));
    let check = r.conflict_checks.start().expect("nothing runs yet");
    let wip = r.line_counts.start_wip().expect("nothing runs yet");
    let before = counts(r);

    let mut parked = active.park();
    let left = &mut active.reads;
    assert!(
        left.signatures.wanted(oid(3)) && left.conflict_checks.is_empty() && left.line_counts.wip().is_none(),
        "no results stay behind"
    );
    let after = counts(left);
    assert_eq!(after, counts(&mut parked.reads), "both sides carry the same counters");
    assert_eq!(after, before.iter().map(|c| c + 1).collect::<Vec<_>>(), "each one past its request in flight");

    let ignores_the_replies = |r: &mut RepoReads| {
        r.signatures.finish(oid(3), sig, Err("old".into()));
        r.signatures.get(oid(3)).is_none() && !r.conflict_checks.finish(check, a_report()).accepted && !r.line_counts.finish_wip(wip, None)
    };
    assert!(ignores_the_replies(&mut active.reads), "the next tab ignores the replies from before the park");
    parked.resume_after(&mut active);
    assert!(ignores_the_replies(&mut parked.reads), "and so does the tab once it is back");
}

#[test]
fn clear_forgets_every_read() {
    let mut r = some_reads();
    r.signatures.finish(oid(5), 0, Err("unsigned".into()));
    r.clear();
    assert!(r.stats.is_empty() && r.stats_pending.is_empty());
    assert!(r.line_counts.commit(oid(1)).is_none() && r.line_counts.wants_commit(oid(2)), "line counts are forgotten");
    assert!(r.signatures.wanted(oid(3)) && r.signatures.get(oid(5)).is_none(), "signatures are forgotten");
    assert!(r.conflict_checks.is_empty());
}
