//! The second pass over the pull request list on services that read checks and votes one pull
//! request at a time: what was read, when, and what is still to ask (update/pr_details.rs).

use super::FORGE_REFRESH_INTERVAL;
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};
use trellis_core::forge::{CheckState, PrDetails, PullRequest};

/// Details older than this are read again on the next list refresh.
pub const DETAILS_MAX_AGE: Duration = Duration::from_secs(15 * 60);

#[derive(Default)]
pub struct PrDetailPass {
    /// What was read for each pull request, and when (None: marked stale by a manual refresh).
    pub cache: HashMap<u64, (PrDetails, Option<Instant>)>,
    /// Pull requests still to ask about, in list order.
    pub queue: VecDeque<u64>,
    /// A chunk is being read.
    pub in_flight: bool,
    /// How many pull requests the last chunk asked about (an answer with fewer was cut short).
    pub asked: usize,
}

impl PrDetailPass {
    /// Whether `pr`'s details should be read again: never read, read for another head, too
    /// old, or pending checks past the refresh interval.
    pub fn stale(&self, pr: &PullRequest, now: Instant) -> bool {
        let Some((d, at)) = self.cache.get(&pr.id) else { return true };
        let Some(at) = at else { return true };
        let age = now.saturating_duration_since(*at);
        let pending = d.checks.as_ref().is_some_and(|c| c.state == CheckState::Pending);
        d.head != pr.head || age >= DETAILS_MAX_AGE || (pending && age >= FORGE_REFRESH_INTERVAL)
    }

    /// A manual refresh: every entry is read again, kept on screen until then.
    pub fn expire(&mut self) {
        for (_, at) in self.cache.values_mut() {
            *at = None;
        }
    }

    pub fn record(&mut self, d: PrDetails, now: Instant) {
        self.cache.insert(d.id, (d, Some(now)));
    }

    pub fn clear(&mut self) {
        *self = PrDetailPass::default();
    }
}
