//! Review votes and CI checks read one pull request at a time, after the list: services that
//! need a call per pull request read them with the list only for the first `CHECKED_PRS`, and
//! the rest in a second pass of `pr_details` calls.

use super::{Checks, PullRequest, Reviewer};
use crate::error::Result;

/// Pull requests asked about per `pr_details` call (the second pass after the list).
pub const DETAILS_PER_CALL: usize = 25;

/// What one pull request's own calls add to the list's entry.
#[derive(Clone, Debug, PartialEq)]
pub struct PrDetails {
    pub id: u64,
    /// The head the details were read for (copied from the pull request asked about).
    pub head: Option<String>,
    pub checks: Option<Checks>,
    /// Explicit votes only (GitLab: approvers as "approved"); None keeps the list's reviewers.
    pub votes: Option<Vec<Reviewer>>,
    /// The ids of the approvers among `votes`, so the pull request's `facets` stop awaiting them.
    pub approved_ids: Vec<String>,
}

impl PrDetails {
    /// Nothing read for `pr` (yet): no checks, and the list's reviewers stay.
    pub fn empty(pr: &PullRequest) -> PrDetails {
        PrDetails { id: pr.id, head: pr.head.clone(), checks: None, votes: None, approved_ids: Vec::new() }
    }
}

impl PullRequest {
    /// Merges `d` in. Checks apply only when they were read for this head. Votes come first;
    /// every other reviewer stays, an approval missing from the votes becoming "no vote" (it
    /// was withdrawn). Applying the same details twice changes nothing more.
    pub fn apply(&mut self, d: &PrDetails) {
        if d.head == self.head {
            self.checks = d.checks.clone();
        }
        let Some(votes) = &d.votes else { return };
        // Votes are approvals: every reviewer who hasn't approved is still awaited.
        let f = &mut self.facets;
        f.awaiting_ids = f.reviewer_ids.iter().filter(|id| !d.approved_ids.contains(id)).cloned().collect();
        let mut reviewers = votes.clone();
        for r in std::mem::take(&mut self.reviewers) {
            if votes.iter().any(|v| v.name == r.name) {
                continue;
            }
            let state = if r.state.starts_with("approved") { "no vote".to_owned() } else { r.state };
            reviewers.push(Reviewer { state, ..r });
        }
        self.reviewers = reviewers;
    }
}

/// Reads each pull request with `one`, in order. A pull request the service no longer knows
/// (404) is recorded as read with nothing; any other error stops the pass, returning what was
/// read, or the error when nothing was.
pub(super) fn each(prs: &[PullRequest], mut one: impl FnMut(&PullRequest) -> Result<PrDetails>) -> Result<Vec<PrDetails>> {
    let mut out = Vec::new();
    for p in prs {
        match one(p) {
            Ok(d) => out.push(d),
            Err(e) if e.is_not_found() => out.push(PrDetails::empty(p)),
            Err(e) if out.is_empty() => return Err(e),
            Err(_) => break,
        }
    }
    Ok(out)
}

/// Checks only, for services whose list already carries every reviewer.
pub(super) fn checks_of(prs: &[PullRequest], mut one: impl FnMut(&PullRequest) -> Result<Option<Checks>>) -> Result<Vec<PrDetails>> {
    each(prs, |p| Ok(PrDetails { checks: one(p)?, ..PrDetails::empty(p) }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Error;
    use crate::forge::CheckState;

    fn reviewer(name: &str, state: &str) -> Reviewer {
        Reviewer { name: name.into(), state: state.into(), required: false }
    }

    fn pr(id: u64, head: &str) -> PullRequest {
        PullRequest {
            id,
            title: String::new(),
            description: String::new(),
            source: String::new(),
            target: String::new(),
            author: String::new(),
            draft: false,
            reviewers: vec![reviewer("Grace", "no vote"), reviewer("Linus", "no vote")],
            head: Some(head.into()),
            head_repo: Default::default(),
            merge_status: None,
            checks: None,
            url: String::new(),
            facets: Default::default(),
        }
    }

    fn states(p: &PullRequest) -> Vec<(&str, &str)> {
        p.reviewers.iter().map(|r| (r.name.as_str(), r.state.as_str())).collect()
    }

    #[test]
    fn votes_come_first_and_withdrawn_approvals_reset() {
        let mut p = pr(1, "abc");
        let d = PrDetails { votes: Some(vec![reviewer("Grace", "approved")]), ..PrDetails::empty(&p) };
        p.apply(&d);
        assert_eq!(states(&p), [("Grace", "approved"), ("Linus", "no vote")]);
        p.apply(&d);
        assert_eq!(states(&p), [("Grace", "approved"), ("Linus", "no vote")], "idempotent");
        p.apply(&PrDetails { votes: Some(Vec::new()), ..PrDetails::empty(&p) });
        assert_eq!(states(&p), [("Grace", "no vote"), ("Linus", "no vote")]);
        // No votes read: the reviewers stay as they are.
        p.reviewers[1].state = "changes requested".into();
        p.apply(&PrDetails::empty(&p));
        assert_eq!(states(&p), [("Grace", "no vote"), ("Linus", "changes requested")]);
    }

    #[test]
    fn approvals_stop_awaiting_their_reviewers() {
        let mut p = pr(1, "abc");
        p.facets.reviewer_ids = vec!["g".into(), "l".into()];
        p.facets.awaiting_ids = p.facets.reviewer_ids.clone();
        let approved =
            PrDetails { votes: Some(vec![reviewer("Grace", "approved")]), approved_ids: vec!["g".into()], ..PrDetails::empty(&p) };
        p.apply(&approved);
        assert_eq!(p.facets.awaiting_ids, ["l"]);
        p.apply(&PrDetails::empty(&p));
        assert_eq!(p.facets.awaiting_ids, ["l"], "no votes read: unchanged");
        p.apply(&PrDetails { votes: Some(Vec::new()), ..PrDetails::empty(&p) });
        assert_eq!(p.facets.awaiting_ids, ["g", "l"], "a withdrawn approval is awaited again");
    }

    #[test]
    fn checks_apply_only_to_their_head() {
        let mut p = pr(1, "abc");
        let passed = Some(Checks::state_only(CheckState::Passed, String::new()));
        p.apply(&PrDetails { head: Some("old".into()), checks: passed.clone(), ..PrDetails::empty(&p) });
        assert_eq!(p.checks, None);
        p.apply(&PrDetails { checks: passed.clone(), ..PrDetails::empty(&p) });
        assert_eq!(p.checks, passed);
        p.apply(&PrDetails::empty(&p));
        assert_eq!(p.checks, None, "a matching head replaces them, with none too");
    }

    #[test]
    fn each_skips_missing_pull_requests_and_stops_on_other_errors() {
        let prs: Vec<PullRequest> = (1..=4).map(|i| pr(i, "h")).collect();
        let read = |fail: u64, e: fn() -> Error| {
            each(&prs, |p| {
                if p.id == fail {
                    return Err(e());
                }
                Ok(PrDetails { checks: Some(Checks::state_only(CheckState::Passed, String::new())), ..PrDetails::empty(p) })
            })
        };
        let got = read(2, || Error::NotFound("gone".into())).unwrap();
        assert_eq!(got.len(), 4);
        assert_eq!(got[1], PrDetails::empty(&prs[1]));
        assert!(got[2].checks.is_some());
        assert!(matches!(read(1, || Error::Forbidden("no".into())), Err(Error::Forbidden(_))));
        assert_eq!(read(3, || Error::Forbidden("no".into())).unwrap().len(), 2);
    }
}
