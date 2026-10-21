//! CI status of branch tips: which tips are on screen, what the hosting service said about each
//! commit, and when to ask again (update/build_status.rs).

use super::FORGE_REFRESH_INTERVAL;
use git2::Oid;
use glowfly_core::forge::{CheckState, Checks, TipQuery};
use glowfly_core::repo::{BranchInfo, Snapshot};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

/// The least time between two tip requests.
pub const TIP_REQUEST_GAP: Duration = Duration::from_secs(15);
/// How soon a pending tip is asked about again while it is on screen.
pub const TIP_PENDING_RECHECK: Duration = Duration::from_secs(60);
/// How long a finished (or "no status") answer stays fresh; also the back-off after an error.
pub const TIP_RECHECK: Duration = FORGE_REFRESH_INTERVAL;
/// The most tips asked about in one request.
pub const TIPS_PER_REQUEST: usize = 10;
/// The most tips one frame reports as drawn.
pub const MAX_SEEN: usize = 64;

/// What is known about one commit.
#[derive(Clone, Debug)]
pub struct TipEntry {
    /// None: the service reported no status for it.
    pub checks: Option<Checks>,
    pub at: Instant,
    /// Kept on screen, but asked about again (after a manual refresh).
    pub stale: bool,
}

#[derive(Default)]
pub struct TipChecks {
    pub by_oid: HashMap<Oid, TipEntry>,
    /// The tips drawn in the last frame, in drawing order.
    pub visible: Vec<Oid>,
    pub in_flight: HashSet<Oid>,
    pub last_request: Option<Instant>,
    pub backoff_until: Option<Instant>,
}

impl TipChecks {
    pub fn checks(&self, oid: Oid) -> Option<&Checks> {
        self.by_oid.get(&oid).and_then(|e| e.checks.as_ref())
    }

    /// Whether any tip has a status to show (rows reserve the marker column then).
    pub fn any(&self) -> bool {
        self.by_oid.values().any(|e| e.checks.is_some())
    }

    fn max_age(e: &TipEntry) -> Duration {
        if e.checks.as_ref().is_some_and(|c| c.state == CheckState::Pending) { TIP_PENDING_RECHECK } else { TIP_RECHECK }
    }

    /// When this entry needs asking again.
    fn due_at(e: &TipEntry) -> Option<Instant> {
        (!e.stale).then(|| e.at + Self::max_age(e))
    }

    pub fn fresh(&self, oid: Oid, now: Instant) -> bool {
        self.by_oid.get(&oid).and_then(Self::due_at).is_some_and(|t| now < t)
    }

    /// Visible tips to ask about, in drawing order.
    pub fn due(&self, now: Instant) -> Vec<Oid> {
        self.visible.iter().copied().filter(|o| !self.in_flight.contains(o) && !self.fresh(*o, now)).collect()
    }

    pub fn may_ask(&self, now: Instant) -> bool {
        self.in_flight.is_empty()
            && self.last_request.is_none_or(|t| now.duration_since(t) >= TIP_REQUEST_GAP)
            && self.backoff_until.is_none_or(|t| now >= t)
    }

    /// How long until a visible tip may be asked about, for waking the window; at least a second.
    pub fn next_wake(&self, now: Instant) -> Option<Duration> {
        if self.visible.is_empty() {
            return None;
        }
        let gate = [self.last_request.map(|t| t + TIP_REQUEST_GAP), self.backoff_until].into_iter().flatten().max();
        let soonest = self
            .visible
            .iter()
            .filter(|o| !self.in_flight.contains(o))
            .map(|o| self.by_oid.get(o).and_then(Self::due_at).unwrap_or(now))
            .min()?;
        let at = gate.map_or(soonest, |g| g.max(soonest));
        Some(at.saturating_duration_since(now).max(Duration::from_secs(1)))
    }

    /// The answer to a request for `asked`. A failure keeps what was shown, counts as fresh
    /// and backs off.
    pub fn record(&mut self, asked: &[TipQuery], result: Result<HashMap<String, Checks>, ()>, now: Instant) {
        let asked: Vec<(Oid, &str)> = asked.iter().filter_map(|q| Some((Oid::from_str(&q.sha).ok()?, q.sha.as_str()))).collect();
        for (o, _) in &asked {
            self.in_flight.remove(o);
        }
        match result {
            Ok(mut map) => {
                for (o, sha) in asked {
                    self.by_oid.insert(o, TipEntry { checks: map.remove(sha), at: now, stale: false });
                }
            }
            Err(()) => {
                for (o, _) in asked {
                    let checks = self.by_oid.remove(&o).and_then(|e| e.checks);
                    self.by_oid.insert(o, TipEntry { checks, at: now, stale: false });
                }
                self.backoff_until = Some(now + TIP_RECHECK);
            }
        }
    }

    /// A manual refresh: ask again now, keeping what is shown until the answer arrives.
    pub fn expire(&mut self) {
        self.backoff_until = None;
        for e in self.by_oid.values_mut() {
            e.stale = true;
        }
    }

    /// A pull request's head commit got its checks with the pull request list.
    pub fn seed(&mut self, oid: Oid, checks: Checks, now: Instant) {
        self.by_oid.insert(oid, TipEntry { checks: Some(checks), at: now, stale: false });
    }
}

/// The tips among `oids` the host can be asked about, in input order: remote-tracking branches
/// under `remote`, and local branches whose upstream is on `remote`.
pub fn tip_queries(snap: &Snapshot, remote: &str, oids: &[Oid]) -> Vec<(Oid, TipQuery)> {
    queries_from(&snap.locals, &snap.remotes, remote, oids)
}

fn queries_from(locals: &[BranchInfo], remotes: &[(String, Vec<(String, Oid)>)], remote: &str, oids: &[Oid]) -> Vec<(Oid, TipQuery)> {
    let on_remote = remotes.iter().find(|(r, _)| r == remote).map(|(_, b)| b.as_slice()).unwrap_or_default();
    oids.iter()
        .filter_map(|o| {
            let branch = on_remote.iter().find(|(_, t)| t == o).map(|(n, _)| n.clone()).or_else(|| {
                locals
                    .iter()
                    .find(|b| b.oid == Some(*o) && b.tracking.as_ref().is_some_and(|t| t.remote == remote))
                    .and_then(|b| b.tracking.as_ref().map(|t| t.merge.clone()))
            })?;
            Some((*o, TipQuery { sha: o.to_string(), branch }))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use glowfly_core::repo::Tracking;

    fn oid(n: u8) -> Oid {
        Oid::from_bytes(&[n; 20]).unwrap()
    }

    fn local(name: &str, o: Oid, tracking: Option<(&str, &str)>) -> BranchInfo {
        BranchInfo {
            name: name.into(),
            oid: Some(o),
            is_head: false,
            upstream: None,
            ahead: 0,
            behind: 0,
            tracking: tracking.map(|(r, m)| Tracking { remote: r.into(), merge: m.into() }),
            upstream_ref: None,
            merged: false,
        }
    }

    fn ask(n: u8) -> TipQuery {
        TipQuery { sha: oid(n).to_string(), branch: "b".into() }
    }

    fn checks(state: CheckState) -> Checks {
        Checks { state, ..Checks::from_counts(1, 0, 0, true) }
    }

    #[test]
    fn tip_queries_ask_only_about_the_host_remote() {
        let locals = [
            local("feature", oid(2), Some(("origin", "feature"))),
            local("solo", oid(3), None),
            local("fork", oid(4), Some(("upstream", "x"))),
        ];
        let remotes =
            vec![("origin".to_owned(), vec![("main".to_owned(), oid(1))]), ("upstream".to_owned(), vec![("dev".to_owned(), oid(5))])];
        let got = queries_from(&locals, &remotes, "origin", &[oid(5), oid(2), oid(3), oid(1), oid(4)]);
        let names: Vec<(Oid, &str)> = got.iter().map(|(o, q)| (*o, q.branch.as_str())).collect();
        assert_eq!(names, [(oid(2), "feature"), (oid(1), "main")]);
        assert_eq!(got[0].1.sha, oid(2).to_string());
    }

    #[test]
    fn due_follows_visibility_flight_and_freshness() {
        let t0 = Instant::now();
        let mut tc = TipChecks { visible: vec![oid(3), oid(1), oid(2)], ..Default::default() };
        assert_eq!(tc.due(t0), [oid(3), oid(1), oid(2)]);
        tc.in_flight.insert(oid(1));
        assert_eq!(tc.due(t0), [oid(3), oid(2)]);
        tc.in_flight.clear();
        tc.seed(oid(1), checks(CheckState::Pending), t0);
        tc.seed(oid(2), checks(CheckState::Passed), t0);
        tc.by_oid.insert(oid(3), TipEntry { checks: None, at: t0, stale: false });
        assert!(tc.due(t0 + Duration::from_secs(59)).is_empty());
        assert_eq!(tc.due(t0 + Duration::from_secs(61)), [oid(1)]);
        assert_eq!(tc.due(t0 + Duration::from_secs(301)), [oid(3), oid(1), oid(2)]);
        tc.expire();
        assert_eq!(tc.due(t0), [oid(3), oid(1), oid(2)]);
        // Stale entries stay on screen.
        assert_eq!(tc.checks(oid(2)).map(|c| c.state), Some(CheckState::Passed));
        assert!(tc.any());
    }

    #[test]
    fn may_ask_waits_for_the_gap_and_the_back_off() {
        let t0 = Instant::now();
        let mut tc = TipChecks { visible: vec![oid(1)], ..Default::default() };
        assert!(tc.may_ask(t0));
        tc.last_request = Some(t0);
        tc.in_flight.insert(oid(1));
        assert!(!tc.may_ask(t0 + Duration::from_secs(20)));
        tc.record(&[ask(1)], Ok(HashMap::from([(oid(1).to_string(), checks(CheckState::Failed))])), t0);
        assert!(!tc.may_ask(t0 + Duration::from_secs(14)));
        assert!(tc.may_ask(t0 + Duration::from_secs(15)));
        assert_eq!(tc.checks(oid(1)).map(|c| c.state), Some(CheckState::Failed));

        // An error keeps the old answer and backs off for five minutes.
        let t1 = t0 + Duration::from_secs(400);
        tc.expire();
        tc.record(&[ask(1)], Err(()), t1);
        assert_eq!(tc.checks(oid(1)).map(|c| c.state), Some(CheckState::Failed));
        assert!(!tc.may_ask(t1 + Duration::from_secs(299)));
        assert!(tc.may_ask(t1 + Duration::from_secs(300)));
        tc.expire();
        assert!(tc.may_ask(t1 + Duration::from_secs(16)));
    }

    #[test]
    fn next_wake_is_the_soonest_due_tip() {
        let t0 = Instant::now();
        let mut tc = TipChecks::default();
        assert_eq!(tc.next_wake(t0), None);
        tc.visible = vec![oid(1), oid(2)];
        // Never asked: due now, but at least a second away.
        assert_eq!(tc.next_wake(t0), Some(Duration::from_secs(1)));
        tc.seed(oid(1), checks(CheckState::Pending), t0);
        tc.seed(oid(2), checks(CheckState::Passed), t0);
        tc.last_request = Some(t0);
        assert_eq!(tc.next_wake(t0), Some(TIP_PENDING_RECHECK));
        tc.backoff_until = Some(t0 + Duration::from_secs(120));
        assert_eq!(tc.next_wake(t0), Some(Duration::from_secs(120)));
    }
}
