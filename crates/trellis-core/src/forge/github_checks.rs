//! A GitHub pull request's head-commit status (check runs and commit statuses), read in the same
//! GraphQL call as its reviews.

use super::{CheckState, Checks};
use serde::Deserialize;

/// GraphQL selection for a pull request's head-commit status, added beside `reviews` in each
/// `pN: pullRequest(number: N)` field.
pub(super) const ROLLUP_FIELDS: &str = "commits(last: 1) { nodes { commit { statusCheckRollup { state contexts(first: 100) { totalCount nodes { __typename ... on CheckRun { status conclusion } ... on StatusContext { state } } } } } } }";

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
    },
    /// A commit status posted through the statuses API.
    #[serde(rename = "StatusContext")]
    Status { state: String },
    #[serde(other)]
    Other,
}

/// The PR answer's rollup (at `commits.nodes[0].commit.statusCheckRollup`), if any.
pub(super) fn from_pr(pr: &serde_json::Value) -> Option<Checks> {
    let rollup = &pr["commits"]["nodes"][0]["commit"]["statusCheckRollup"];
    if rollup.is_null() {
        return None;
    }
    serde_json::from_value::<Rollup>(rollup.clone()).ok().map(to_checks)
}

fn to_checks(r: Rollup) -> Checks {
    let (mut passed, mut failed, mut pending) = (0, 0, 0);
    let mut complete = r.contexts.nodes.len() == r.contexts.total;
    for c in &r.contexts.nodes {
        match c {
            Context::CheckRun { conclusion: None } => pending += 1,
            Context::CheckRun { conclusion: Some(c) } => match c.as_str() {
                "SUCCESS" | "NEUTRAL" | "SKIPPED" => passed += 1,
                _ => failed += 1,
            },
            Context::Status { state } => match state.as_str() {
                "SUCCESS" => passed += 1,
                "FAILURE" | "ERROR" => failed += 1,
                "PENDING" | "EXPECTED" => pending += 1,
                _ => complete = false,
            },
            Context::Other => complete = false,
        }
    }
    let counted = Checks::from_counts(passed, failed, pending, complete);
    let state = match r.state.as_str() {
        "SUCCESS" => CheckState::Passed,
        "FAILURE" | "ERROR" => CheckState::Failed,
        "PENDING" | "EXPECTED" => CheckState::Pending,
        _ => counted.state,
    };
    Checks { state, ..counted }
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
        let got = from_pr(&pr(rollup("FAILURE", 10, nodes)));
        assert_eq!(got, Some(Checks { state: CheckState::Failed, passed: 4, failed: 4, pending: 2, complete: true }));
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
