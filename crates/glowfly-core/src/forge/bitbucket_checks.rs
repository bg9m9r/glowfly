//! Bitbucket build statuses, posted on commits by CI: for Cloud pull requests and commits, and
//! for Data Center commits (a pull request's latest commit, or a branch tip); counted for the
//! lists, or each one listed for the pull request panel (`cloud_list`, `dc_list`).

use super::bitbucket::cloud_repo;
use super::check_detail::{CheckList, CheckRun, MAX_CHECKS, RunState, time_of};
use super::checks::{Outcome, Tally, TipQuery, per_tip};
use super::page::{self, Page};
use super::{Auth, Checks, Target, get};
use crate::error::Result;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
struct BuildStatus {
    state: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    key: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    /// Cloud's times.
    #[serde(default)]
    created_on: Option<String>,
    #[serde(default)]
    updated_on: Option<String>,
    /// Data Center's time, in epoch milliseconds.
    #[serde(rename = "dateAdded", default)]
    date_added: Option<i64>,
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
    #[serde(default)]
    next_page_start: Option<u64>,
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

/// A build status's state, for the pull request panel.
fn run_state(state: &str) -> RunState {
    match state {
        "SUCCESSFUL" => RunState::Passed,
        "FAILED" => RunState::Failed,
        "INPROGRESS" => RunState::Running,
        "STOPPED" | "CANCELLED" => RunState::Cancelled,
        _ => RunState::Neutral,
    }
}

/// A status as a check: named by its name, else its key.
fn to_run(s: BuildStatus, started: Option<i64>, updated: Option<i64>) -> CheckRun {
    let state = run_state(&s.state);
    let (started, finished) = if state.is_active() { (started.or(updated), None) } else { (started, updated) };
    CheckRun {
        name: s.name.filter(|n| !n.trim().is_empty()).unwrap_or(s.key),
        group: None,
        state,
        required: None,
        started,
        finished,
        url: s.url,
        description: s.description.unwrap_or_default(),
    }
}

/// Every build status on a Cloud pull request (its latest commit's).
pub(super) fn cloud_list(t: &Target, auth: &Auth, id: u64) -> Result<CheckList> {
    let first = format!("{}/pullrequests/{id}/statuses?pagelen=100", cloud_repo(t));
    let all = page::collect(first, |url| {
        let p: Statuses = get(auth, url)?;
        Ok(Page { items: p.values, next: p.next })
    })?;
    let complete = all.len() <= MAX_CHECKS;
    let runs = all
        .into_iter()
        .map(|s| {
            let (started, updated) = (time_of(&s.created_on), time_of(&s.updated_on));
            to_run(s, started, updated)
        })
        .collect();
    Ok(CheckList { sha: String::new(), runs, complete })
}

/// Every build status on a Data Center commit (`sha`, a full commit id).
pub(super) fn dc_list(auth: &Auth, host: &str, sha: &str) -> Result<CheckList> {
    let first = format!("https://{host}/rest/build-status/1.0/commits/{sha}?limit=100");
    let all = page::collect(first.clone(), |url| {
        let p: Statuses = get(auth, url)?;
        let next = match (p.is_last_page, p.next_page_start) {
            (Some(false), Some(start)) => Some(page::with_param(&first, "start", &start.to_string())),
            _ => None,
        };
        Ok(Page { items: p.values, next })
    })?;
    let complete = all.len() <= MAX_CHECKS;
    let runs = all
        .into_iter()
        .map(|s| {
            let added = s.date_added.filter(|&ms| ms > 0).map(|ms| ms / 1000);
            if run_state(&s.state).is_active() { to_run(s, added, None) } else { to_run(s, None, added) }
        })
        .collect();
    Ok(CheckList { sha: sha.to_owned(), runs, complete })
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

    #[test]
    fn run_states() {
        use RunState::*;
        let cases = [
            ("SUCCESSFUL", Passed),
            ("FAILED", Failed),
            ("INPROGRESS", Running),
            ("STOPPED", Cancelled),
            ("CANCELLED", Cancelled),
            ("UNKNOWN", Neutral),
        ];
        for (state, want) in cases {
            assert_eq!(run_state(state), want, "{state}");
        }
    }
}
