//! A GitHub commit's status (check runs and commit statuses): a pull request's head, read in the
//! same GraphQL call as its reviews, and branch tips, read by commit id in one call.

use super::checks::{Outcome, Tally, TipQuery};
use super::{Auth, CheckState, Checks, Target, github, post};
use crate::error::{Error, Result};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;

/// The selection of a commit's status rollup, with each check's link.
macro_rules! rollup {
    () => {
        "statusCheckRollup { state contexts(first: 100) { totalCount nodes { __typename ... on CheckRun { status conclusion detailsUrl url } ... on StatusContext { state targetUrl } } } }"
    };
}

/// GraphQL selection for a pull request's head-commit status, added beside `reviews` in each
/// `pN: pullRequest(number: N)` field.
pub(super) const ROLLUP_FIELDS: &str = concat!("commits(last: 1) { nodes { commit { ", rollup!(), " } } }");

#[derive(Deserialize)]
struct Rollup {
    state: String,
    contexts: Contexts,
}

#[derive(Deserialize)]
struct Contexts {
    #[serde(rename = "totalCount")]
    total: usize,
    #[serde(default)]
    nodes: Vec<Context>,
}

#[derive(Deserialize)]
#[serde(tag = "__typename")]
enum Context {
    /// A GitHub Actions or app check; it has a conclusion once it has finished.
    CheckRun {
        #[serde(default)]
        conclusion: Option<String>,
        #[serde(rename = "detailsUrl", default)]
        details_url: Option<String>,
        #[serde(default)]
        url: Option<String>,
    },
    /// A commit status posted through the statuses API.
    #[serde(rename = "StatusContext")]
    Status {
        state: String,
        #[serde(rename = "targetUrl", default)]
        target_url: Option<String>,
    },
    #[serde(other)]
    Other,
}

/// The PR answer's rollup (at `commits.nodes[0].commit.statusCheckRollup`), if any.
pub(super) fn from_pr(pr: &serde_json::Value) -> Option<Checks> {
    from_rollup(&pr["commits"]["nodes"][0]["commit"]["statusCheckRollup"])
}

fn from_rollup(rollup: &serde_json::Value) -> Option<Checks> {
    if rollup.is_null() {
        return None;
    }
    serde_json::from_value::<Rollup>(rollup.clone()).ok().map(to_checks)
}

fn to_checks(r: Rollup) -> Checks {
    let mut tally = Tally::new();
    if r.contexts.nodes.len() != r.contexts.total {
        tally.incomplete();
    }
    for c in r.contexts.nodes {
        match c {
            Context::CheckRun { conclusion, details_url, url } => {
                let o = match conclusion.as_deref() {
                    None => Outcome::Pending,
                    Some("SUCCESS" | "NEUTRAL" | "SKIPPED") => Outcome::Passed,
                    Some(_) => Outcome::Failed,
                };
                tally.add(o, details_url.or(url).as_deref());
            }
            Context::Status { state, target_url } => match state.as_str() {
                "SUCCESS" => tally.add(Outcome::Passed, target_url.as_deref()),
                "FAILURE" | "ERROR" => tally.add(Outcome::Failed, target_url.as_deref()),
                "PENDING" | "EXPECTED" => tally.add(Outcome::Pending, target_url.as_deref()),
                _ => tally.incomplete(),
            },
            Context::Other => tally.incomplete(),
        }
    }
    // No contexts at all: the rollup state alone decides.
    let counted = tally.finish().unwrap_or_else(|| Checks::from_counts(0, 0, 0, true));
    let state = match r.state.as_str() {
        "SUCCESS" => CheckState::Passed,
        "FAILURE" | "ERROR" => CheckState::Failed,
        "PENDING" | "EXPECTED" => CheckState::Pending,
        _ => counted.state,
    };
    Checks { state, ..counted }
}

/// The status of each branch tip, by full commit id, in one GraphQL call. Commits the server
/// doesn't know, or that have no status, are left out.
pub(super) fn commit_rollups(t: &Target, auth: &Auth, tips: &[TipQuery]) -> Result<HashMap<String, Checks>> {
    let params: String = (0..tips.len()).map(|i| format!(", $o{i}: GitObjectID!")).collect();
    let fields: String = (0..tips.len()).map(|i| format!("c{i}: object(oid: $o{i}) {{ ... on Commit {{ {} }} }} ", rollup!())).collect();
    let query = format!("query($owner: String!, $repo: String!{params}) {{ repository(owner: $owner, name: $repo) {{ {fields}}} }}");
    let mut vars = json!({ "owner": t.owner, "repo": t.repo });
    for (i, q) in tips.iter().enumerate() {
        vars[format!("o{i}")] = json!(q.sha);
    }
    let answer: serde_json::Value = post(auth, &github::graphql_url(t), json!({ "query": query, "variables": vars }))?;
    let repo = &answer["data"]["repository"];
    if repo.is_null() && answer["errors"].is_array() {
        return Err(Error::Unsupported("this server can't report commit status".into()));
    }
    Ok(tips
        .iter()
        .enumerate()
        .filter_map(|(i, q)| from_rollup(&repo[format!("c{i}")]["statusCheckRollup"]).map(|c| (q.sha.clone(), c)))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn pr(rollup: Value) -> Value {
        json!({ "commits": { "nodes": [{ "commit": { "statusCheckRollup": rollup } }] } })
    }

    fn run(conclusion: Option<&str>) -> Value {
        json!({ "__typename": "CheckRun", "status": if conclusion.is_some() { "COMPLETED" } else { "IN_PROGRESS" }, "conclusion": conclusion })
    }

    fn status(state: &str) -> Value {
        json!({ "__typename": "StatusContext", "state": state })
    }

    fn rollup(state: &str, total: usize, nodes: Vec<Value>) -> Value {
        json!({ "state": state, "contexts": { "totalCount": total, "nodes": nodes } })
    }

    #[test]
    fn classifies_check_runs_and_status_contexts() {
        let mut nodes: Vec<Value> =
            ["SUCCESS", "NEUTRAL", "SKIPPED", "FAILURE", "CANCELLED", "TIMED_OUT"].into_iter().map(|c| run(Some(c))).collect();
        nodes.push(run(None));
        nodes.extend(["SUCCESS", "PENDING", "ERROR"].map(status));
        let got = from_pr(&pr(rollup("FAILURE", 10, nodes))).unwrap();
        assert_eq!((got.state, got.passed, got.failed, got.pending, got.complete), (CheckState::Failed, 4, 4, 2, true));
        assert_eq!(got.url, "");
    }

    #[test]
    fn links_come_from_the_first_failed_check() {
        let linked = |c: &str, details: Value, url: &str| json!({ "__typename": "CheckRun", "status": "COMPLETED", "conclusion": c, "detailsUrl": details, "url": url });
        let nodes = vec![
            linked("SUCCESS", json!("https://ci.example/ok"), "https://github.com/o/r/runs/1"),
            linked("FAILURE", Value::Null, "https://github.com/o/r/runs/2"),
            linked("FAILURE", json!("https://ci.example/bad"), "https://github.com/o/r/runs/3"),
        ];
        let got = from_pr(&pr(rollup("FAILURE", 3, nodes))).unwrap();
        // detailsUrl is null on the first failure: its own run page is used.
        assert_eq!(got.url, "https://github.com/o/r/runs/2");
        let s = json!({ "__typename": "StatusContext", "state": "PENDING", "targetUrl": "https://ci.example/7" });
        let got = from_pr(&pr(rollup("SUCCESS", 2, vec![run(Some("SUCCESS")), s]))).unwrap();
        // The rollup state still decides the marker; the pending status gives the link.
        assert_eq!((got.state, got.url.as_str()), (CheckState::Passed, "https://ci.example/7"));
    }

    #[test]
    fn rollup_state_wins_and_unknown_contexts_make_counts_incomplete() {
        // More contexts than one page holds: counts are partial, the rollup still decides.
        let many = from_pr(&pr(rollup("PENDING", 130, (0..100).map(|_| run(Some("SUCCESS"))).collect()))).unwrap();
        assert_eq!((many.state, many.passed, many.complete), (CheckState::Pending, 100, false));
        let odd = from_pr(&pr(rollup("SUCCESS", 2, vec![run(Some("SUCCESS")), json!({ "__typename": "Deployment" })]))).unwrap();
        assert_eq!((odd.state, odd.passed, odd.total(), odd.complete), (CheckState::Passed, 1, 1, false));

        let state_of = |s: &str| from_pr(&pr(rollup(s, 2, vec![run(Some("SUCCESS")), run(None)]))).unwrap().state;
        assert_eq!(state_of("SUCCESS"), CheckState::Passed);
        assert_eq!(state_of("ERROR"), CheckState::Failed);
        assert_eq!(state_of("EXPECTED"), CheckState::Pending);
        // An unknown overall state falls back to what the counts say (one still running).
        assert_eq!(state_of("SOMETHING_NEW"), CheckState::Pending);

        assert_eq!(from_pr(&pr(Value::Null)), None);
        assert_eq!(from_pr(&json!({ "commits": { "nodes": [] } })), None);
        assert_eq!(from_pr(&json!({ "reviews": { "nodes": [] } })), None);
    }
}
