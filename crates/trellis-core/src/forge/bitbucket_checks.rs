//! Bitbucket build statuses, posted on commits by CI: for Cloud pull requests and commits, and
//! for Data Center commits (a pull request's latest commit, or a branch tip).

use super::bitbucket::cloud_repo;
use super::checks::{Outcome, Tally, TipQuery, per_tip};
use super::{Auth, Checks, Target, get};
use crate::error::Result;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
struct BuildStatus {
    state: String,
    #[serde(default)]
    url: String,
}

/// One page of statuses: Cloud links a next page, Data Center says whether this is the last.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Statuses {
    #[serde(default)]
    values: Vec<BuildStatus>,
    #[serde(default)]
    next: Option<String>,
    #[serde(default)]
    is_last_page: Option<bool>,
}

fn outcome(state: &str) -> Option<Outcome> {
    match state {
        "SUCCESSFUL" => Some(Outcome::Passed),
        "FAILED" | "STOPPED" | "CANCELLED" => Some(Outcome::Failed),
        "INPROGRESS" => Some(Outcome::Pending),
        _ => None,
    }
}

fn tally(page: Statuses) -> Option<Checks> {
    let mut t = Tally::new();
    if page.next.is_some() || page.is_last_page == Some(false) {
        t.incomplete();
    }
    for s in page.values {
        match outcome(&s.state) {
            Some(o) => t.add(o, Some(&s.url)),
            None => t.incomplete(),
        }
    }
    t.finish()
}

/// A Cloud pull request's build statuses.
pub(super) fn cloud_pr(t: &Target, auth: &Auth, id: u64) -> Result<Option<Checks>> {
    Ok(tally(get(auth, &format!("{}/pullrequests/{id}/statuses?pagelen=100", cloud_repo(t)))?))
}

pub(super) fn cloud_commits(t: &Target, auth: &Auth, tips: &[TipQuery]) -> Result<HashMap<String, Checks>> {
    per_tip(tips, |q| Ok(tally(get(auth, &format!("{}/commit/{}/statuses?pagelen=100", cloud_repo(t), q.sha))?)))
}

/// A Data Center commit's build statuses.
pub(super) fn dc_commit(auth: &Auth, host: &str, sha: &str) -> Result<Option<Checks>> {
    Ok(tally(get(auth, &format!("https://{host}/rest/build-status/1.0/commits/{sha}?limit=100"))?))
}

pub(super) fn dc_commits(t: &Target, auth: &Auth, tips: &[TipQuery]) -> Result<HashMap<String, Checks>> {
    per_tip(tips, |q| dc_commit(auth, &t.host, &q.sha))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_states() {
        let cases = [
            ("SUCCESSFUL", Some(Outcome::Passed)),
            ("FAILED", Some(Outcome::Failed)),
            ("STOPPED", Some(Outcome::Failed)),
            ("CANCELLED", Some(Outcome::Failed)),
            ("INPROGRESS", Some(Outcome::Pending)),
            ("UNKNOWN", None),
        ];
        for (state, want) in cases {
            assert_eq!(outcome(state), want, "{state}");
        }
        let page: Statuses = serde_json::from_str(r#"{"values":[{"state":"SUCCESSFUL"},{"state":"UNKNOWN"}]}"#).unwrap();
        assert_eq!(tally(page).map(|c| (c.passed, c.complete)), Some((1, false)));
        let empty: Statuses = serde_json::from_str(r#"{"values":[]}"#).unwrap();
        assert_eq!(tally(empty), None);
    }
}
