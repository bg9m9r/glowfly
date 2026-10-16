//! CI checks on a pull request's head commit or a branch tip, in a form common to every service.

use std::collections::HashMap;

/// Pull requests (the first in each list) that get per-PR check calls on services that need
/// one call per pull request.
pub const CHECKED_PRS: usize = 25;
/// The most commits asked about in one `commit_checks` call.
pub const TIPS_PER_CALL: usize = 10;

/// The overall build result of a pull request's head commit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckState {
    Passed,
    Failed,
    Pending,
}

/// CI checks on a pull request's head commit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checks {
    pub state: CheckState,
    pub passed: usize,
    pub failed: usize,
    pub pending: usize,
    /// False when not every check could be read (too many, or of a kind not understood);
    /// the counts then cover only those read and are not shown.
    pub complete: bool,
    /// The build page to open (the first failed check's, else the first pending one's, else
    /// the first check's); empty when the service gave no usable link.
    pub url: String,
}

impl Checks {
    pub fn total(&self) -> usize {
        self.passed + self.failed + self.pending
    }

    /// Overall state from counts: any failed check fails the run, else any pending one keeps it
    /// pending, else it passed.
    pub fn from_counts(passed: usize, failed: usize, pending: usize, complete: bool) -> Checks {
        let state = if failed > 0 {
            CheckState::Failed
        } else if pending > 0 {
            CheckState::Pending
        } else {
            CheckState::Passed
        };
        Checks { state, passed, failed, pending, complete, url: String::new() }
    }

    /// Only an overall state (a service that reports one pipeline, not its checks): the
    /// summary names the state alone.
    pub fn state_only(state: CheckState, url: String) -> Checks {
        Checks { state, passed: 0, failed: 0, pending: 0, complete: false, url }
    }

    /// One tooltip line, e.g. "Checks: 3 of 5 passed, 2 failed". Counts are shown only when
    /// every check was read; otherwise the overall state alone.
    pub fn summary(&self) -> String {
        if !self.complete || self.total() == 0 {
            let word = match self.state {
                CheckState::Passed => "passed",
                CheckState::Failed => "some failed",
                CheckState::Pending => "pending",
            };
            return format!("Checks: {word}");
        }
        let mut s = format!("Checks: {} of {} passed", self.passed, self.total());
        if self.failed > 0 {
            s.push_str(&format!(", {} failed", self.failed));
        }
        if self.pending > 0 {
            s.push_str(&format!(", {} pending", self.pending));
        }
        s
    }
}

/// One check's result, as counted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Outcome {
    Passed,
    Failed,
    Pending,
}

/// Counts checks one at a time and remembers which build page to offer.
#[derive(Default)]
pub(super) struct Tally {
    passed: usize,
    failed: usize,
    pending: usize,
    complete: bool,
    first_failed: Option<String>,
    first_pending: Option<String>,
    first: Option<String>,
}

impl Tally {
    pub(super) fn new() -> Tally {
        Tally { complete: true, ..Tally::default() }
    }

    pub(super) fn add(&mut self, o: Outcome, url: Option<&str>) {
        let slot = match o {
            Outcome::Passed => {
                self.passed += 1;
                None
            }
            Outcome::Failed => {
                self.failed += 1;
                Some(&mut self.first_failed)
            }
            Outcome::Pending => {
                self.pending += 1;
                Some(&mut self.first_pending)
            }
        };
        let Some(url) = url.filter(|u| !u.is_empty()) else { return };
        if let Some(slot) = slot {
            slot.get_or_insert_with(|| url.to_owned());
        }
        self.first.get_or_insert_with(|| url.to_owned());
    }

    /// A check of a kind not understood, or more than one page of them: counts aren't shown.
    pub(super) fn incomplete(&mut self) {
        self.complete = false;
    }

    /// None when nothing was counted and nothing was left out.
    pub(super) fn finish(self) -> Option<Checks> {
        if self.passed + self.failed + self.pending == 0 && self.complete {
            return None;
        }
        let url = self.first_failed.or(self.first_pending).or(self.first).unwrap_or_default();
        Some(Checks { url, ..Checks::from_counts(self.passed, self.failed, self.pending, self.complete) })
    }
}

/// A branch tip to ask about.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TipQuery {
    /// The full 40-hex commit id.
    pub sha: String,
    /// The branch on the host remote, without `refs/heads/`.
    pub branch: String,
}

/// Checks for each of `items` (pull requests), read with `one` for the first `CHECKED_PRS`,
/// for services asked once per pull request. After a failure other than a missing pull request
/// the rest aren't asked either (a token without access fails every time).
pub(super) fn per_pr<T>(items: &[T], mut one: impl FnMut(&T) -> crate::error::Result<Option<Checks>>) -> Vec<Option<Checks>> {
    let mut asking = true;
    items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            if !asking || i >= CHECKED_PRS {
                return None;
            }
            one(item).unwrap_or_else(|e| {
                asking = e.is_not_found();
                None
            })
        })
        .collect()
}

/// Reads each tip with `one`, for services asked once per commit. An unknown commit (404) has
/// none; any other error stops the lookups, returning what was read, or the error when nothing was.
pub(super) fn per_tip(
    tips: &[TipQuery],
    mut one: impl FnMut(&TipQuery) -> crate::error::Result<Option<Checks>>,
) -> crate::error::Result<HashMap<String, Checks>> {
    let mut out = HashMap::new();
    for q in tips {
        match one(q) {
            Ok(Some(c)) => {
                out.insert(q.sha.clone(), c);
            }
            Ok(None) => {}
            Err(e) if e.is_not_found() => {}
            Err(e) if out.is_empty() => return Err(e),
            Err(_) => break,
        }
    }
    Ok(out)
}

/// A full SHA-1 commit id in hex.
pub(super) fn full_sha(s: &str) -> bool {
    s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tally_offers_the_failed_then_pending_then_first_link() {
        let mut t = Tally::new();
        t.add(Outcome::Passed, Some("https://ci/1"));
        t.add(Outcome::Pending, Some("https://ci/2"));
        t.add(Outcome::Failed, Some(""));
        t.add(Outcome::Failed, Some("https://ci/4"));
        t.add(Outcome::Failed, Some("https://ci/5"));
        let c = t.finish().unwrap();
        assert_eq!((c.state, c.passed, c.failed, c.pending, c.complete), (CheckState::Failed, 1, 3, 1, true));
        assert_eq!(c.url, "https://ci/4");

        let mut t = Tally::new();
        t.add(Outcome::Passed, None);
        t.add(Outcome::Passed, Some("https://ci/1"));
        t.add(Outcome::Pending, Some("https://ci/2"));
        assert_eq!(t.finish().unwrap().url, "https://ci/2");
        let mut t = Tally::new();
        t.add(Outcome::Passed, Some("https://ci/1"));
        t.add(Outcome::Passed, Some("https://ci/2"));
        assert_eq!(t.finish().unwrap().url, "https://ci/1");
        let mut t = Tally::new();
        t.add(Outcome::Failed, None);
        assert_eq!(t.finish().unwrap().url, "");

        assert_eq!(Tally::new().finish(), None);
        let mut t = Tally::new();
        t.add(Outcome::Passed, None);
        t.add(Outcome::Failed, None);
        t.incomplete();
        assert_eq!(t.finish().unwrap().summary(), "Checks: some failed");
        let mut t = Tally::new();
        t.incomplete();
        assert_eq!(t.finish().map(|c| c.summary()), Some("Checks: passed".into()));
    }

    #[test]
    fn state_only_names_the_state() {
        assert_eq!(Checks::state_only(CheckState::Failed, "u".into()).summary(), "Checks: some failed");
        assert_eq!(Checks::state_only(CheckState::Pending, String::new()).summary(), "Checks: pending");
        assert_eq!(Checks::state_only(CheckState::Passed, "u".into()).url, "u");
        assert!(full_sha(&"a1".repeat(20)) && !full_sha("abc1234") && !full_sha(&"g".repeat(40)));
    }

    #[test]
    fn summaries_use_counts_only_when_complete() {
        let cases = [
            ((6, 0, 0), CheckState::Passed, "Checks: 6 of 6 passed"),
            ((3, 2, 0), CheckState::Failed, "Checks: 3 of 5 passed, 2 failed"),
            ((4, 0, 2), CheckState::Pending, "Checks: 4 of 6 passed, 2 pending"),
            ((4, 1, 2), CheckState::Failed, "Checks: 4 of 7 passed, 1 failed, 2 pending"),
        ];
        for ((p, f, n), state, text) in cases {
            let c = Checks::from_counts(p, f, n, true);
            assert_eq!((c.state, c.summary().as_str(), c.total()), (state, text, p + f + n));
        }
        assert_eq!(Checks::from_counts(6, 0, 0, false).summary(), "Checks: passed");
        assert_eq!(Checks::from_counts(3, 2, 0, false).summary(), "Checks: some failed");
        assert_eq!(Checks::from_counts(4, 0, 2, false).summary(), "Checks: pending");
        // No contexts counted, but the service still gave a state.
        let none = |state| Checks { state, passed: 0, failed: 0, pending: 0, complete: true, url: String::new() };
        assert_eq!(none(CheckState::Passed).summary(), "Checks: passed");
        assert_eq!(none(CheckState::Failed).summary(), "Checks: some failed");
        assert_eq!(none(CheckState::Pending).summary(), "Checks: pending");
        assert_eq!(Checks::from_counts(0, 0, 0, true).state, CheckState::Passed);
    }
}
