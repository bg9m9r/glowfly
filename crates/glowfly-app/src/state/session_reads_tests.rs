//! Parking a tab's repository reads keeps the results and drops the requests in flight;
//! clearing them (another repository opened) forgets everything.

use super::*;
use crate::state::session::RepoSession;
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

    let left = &active.reads;
    assert!(left.stats.is_empty() && left.stats_pending.is_empty(), "nothing is left for the next tab");
    assert!(left.line_counts.commit(oid(1)).is_none());
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
