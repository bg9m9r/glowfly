//! Every check on a GitHub pull request's head commit: the commit's status rollup (check runs
//! and commit statuses, the latest attempt of each), read by cursor 100 at a time, with whether
//! each is required to merge. A server that doesn't know the `isRequired` field is asked again
//! without it.

use super::check_detail::{CheckList, CheckQuery, CheckRun, MAX_CHECKS, RunState, time_of};
use super::{Auth, Target, github, post};
use crate::error::{Error, Result};
use serde::Deserialize;
use serde_json::{Value, json};

/// Pages asked for at most, in case a server keeps returning a next page.
const MAX_PAGES: usize = 10;

/// The query for one page of contexts; `with_required` asks whether each one is required.
fn query(with_required: bool) -> String {
    let required = if with_required { " isRequired(pullRequestNumber: $number)" } else { "" };
    format!(
        "query($owner: String!, $repo: String!, $number: Int!, $after: String) {{ repository(owner: $owner, name: $repo) {{ \
         pullRequest(number: $number) {{ commits(last: 1) {{ nodes {{ commit {{ oid statusCheckRollup {{ state \
         contexts(first: 100, after: $after) {{ totalCount pageInfo {{ hasNextPage endCursor }} nodes {{ __typename \
         ... on CheckRun {{ name status conclusion startedAt completedAt detailsUrl url title{required} \
         checkSuite {{ workflowRun {{ workflow {{ name }} }} app {{ name }} }} }} \
         ... on StatusContext {{ context state description targetUrl createdAt{required} }} }} }} }} }} }} }} }} }} }}"
    )
}

#[derive(Deserialize)]
struct Rollup {
    contexts: Contexts,
}

#[derive(Deserialize)]
struct Contexts {
    #[serde(rename = "pageInfo", default)]
    page_info: PageInfo,
    #[serde(default)]
    nodes: Vec<Context>,
}

#[derive(Deserialize, Default)]
struct PageInfo {
    #[serde(rename = "hasNextPage", default)]
    has_next: bool,
    #[serde(rename = "endCursor", default)]
    end: Option<String>,
}

#[derive(Deserialize)]
#[serde(tag = "__typename")]
enum Context {
    CheckRun {
        #[serde(default)]
        name: String,
        #[serde(default)]
        status: String,
        #[serde(default)]
        conclusion: Option<String>,
        #[serde(rename = "startedAt", default)]
        started: Option<String>,
        #[serde(rename = "completedAt", default)]
        completed: Option<String>,
        #[serde(rename = "detailsUrl", default)]
        details_url: Option<String>,
        #[serde(default)]
        url: Option<String>,
        #[serde(default)]
        title: Option<String>,
        #[serde(rename = "isRequired", default)]
        required: Option<bool>,
        #[serde(rename = "checkSuite", default)]
        suite: Option<Suite>,
    },
    #[serde(rename = "StatusContext")]
    Status {
        #[serde(default)]
        context: String,
        #[serde(default)]
        state: String,
        #[serde(default)]
        description: Option<String>,
        #[serde(rename = "targetUrl", default)]
        target_url: Option<String>,
        #[serde(rename = "createdAt", default)]
        created: Option<String>,
        #[serde(rename = "isRequired", default)]
        required: Option<bool>,
    },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct Suite {
    #[serde(rename = "workflowRun", default)]
    workflow_run: Option<WorkflowRun>,
    #[serde(default)]
    app: Option<Named>,
}

#[derive(Deserialize)]
struct WorkflowRun {
    #[serde(default)]
    workflow: Option<Named>,
}

#[derive(Deserialize)]
struct Named {
    #[serde(default)]
    name: String,
}

/// A check run's state from its status and, once completed, its conclusion.
fn run_state(status: &str, conclusion: Option<&str>) -> RunState {
    let completed = status == "COMPLETED" || (status.is_empty() && conclusion.is_some());
    if !completed {
        return if status == "IN_PROGRESS" { RunState::Running } else { RunState::Pending };
    }
    match conclusion.unwrap_or_default() {
        "SUCCESS" => RunState::Passed,
        "FAILURE" | "TIMED_OUT" | "STARTUP_FAILURE" => RunState::Failed,
        "CANCELLED" => RunState::Cancelled,
        "SKIPPED" => RunState::Skipped,
        "ACTION_REQUIRED" => RunState::ActionRequired,
        _ => RunState::Neutral,
    }
}

/// A commit status's state (EXPECTED: a required status that hasn't reported yet).
fn status_state(state: &str) -> RunState {
    match state {
        "SUCCESS" => RunState::Passed,
        "FAILURE" | "ERROR" => RunState::Failed,
        "PENDING" | "EXPECTED" => RunState::Pending,
        _ => RunState::Neutral,
    }
}

fn to_run(c: Context, with_required: bool) -> Option<CheckRun> {
    let required = |r: Option<bool>| if with_required { r } else { None };
    match c {
        Context::CheckRun { name, status, conclusion, started, completed, details_url, url, title, required: r, suite } => {
            let group = suite
                .and_then(|s| s.workflow_run.and_then(|w| w.workflow).map(|w| w.name).filter(|n| !n.is_empty()).or(s.app.map(|a| a.name)));
            Some(CheckRun {
                name,
                group: group.filter(|g| !g.is_empty()),
                state: run_state(&status, conclusion.as_deref()),
                required: required(r),
                started: time_of(&started),
                finished: time_of(&completed),
                url: details_url.filter(|u| !u.is_empty()).or(url).unwrap_or_default(),
                description: title.unwrap_or_default(),
            })
        }
        Context::Status { context, state, description, target_url, created, required: r } => {
            let state = status_state(&state);
            Some(CheckRun {
                name: context,
                group: None,
                state,
                required: required(r),
                started: None,
                // When this state was posted: the end only once it is final.
                finished: if state.is_active() { None } else { time_of(&created) },
                url: target_url.unwrap_or_default(),
                description: description.unwrap_or_default(),
            })
        }
        Context::Other => None,
    }
}

/// The error a GraphQL answer without data carries.
fn answer_error(answer: &Value) -> Error {
    let message = answer["errors"][0]["message"].as_str().unwrap_or("the server sent no checks").to_owned();
    if answer["errors"][0]["type"] == "NOT_FOUND" { Error::NotFound(message) } else { Error::Other(message) }
}

pub(super) fn check_list(t: &Target, auth: &Auth, q: &CheckQuery) -> Result<CheckList> {
    let ask = |after: Option<&str>, with_required: bool| {
        let vars = json!({ "owner": t.owner, "repo": t.repo, "number": q.id, "after": after });
        post::<Value>(auth, &github::graphql_url(t), json!({ "query": query(with_required), "variables": vars }))
    };
    let mut list = CheckList::empty(String::new());
    let mut with_required = true;
    let mut after: Option<String> = None;
    for _ in 0..MAX_PAGES {
        let mut answer = ask(after.as_deref(), with_required)?;
        if with_required && answer["data"]["repository"].is_null() && answer["errors"].is_array() {
            // An older server that doesn't know the field: ask without it.
            with_required = false;
            answer = ask(after.as_deref(), false)?;
        }
        let repo = &answer["data"]["repository"];
        if repo.is_null() {
            return Err(answer_error(&answer));
        }
        let commit = &repo["pullRequest"]["commits"]["nodes"][0]["commit"];
        if list.sha.is_empty() {
            list.sha = commit["oid"].as_str().unwrap_or_default().to_owned();
        }
        let rollup = &commit["statusCheckRollup"];
        if rollup.is_null() {
            return Ok(list);
        }
        let r: Rollup = serde_json::from_value(rollup.clone())?;
        list.runs.extend(r.contexts.nodes.into_iter().filter_map(|c| to_run(c, with_required)));
        match r.contexts.page_info {
            PageInfo { has_next: false, .. } => return Ok(list),
            PageInfo { end: Some(end), .. } if list.runs.len() < MAX_CHECKS && after.as_deref() != Some(end.as_str()) => after = Some(end),
            _ => break,
        }
    }
    list.complete = false;
    Ok(list)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_run_states() {
        use RunState::*;
        let cases = [
            ("IN_PROGRESS", None, Running),
            ("QUEUED", None, Pending),
            ("PENDING", None, Pending),
            ("REQUESTED", None, Pending),
            ("WAITING", None, Pending),
            ("COMPLETED", Some("SUCCESS"), Passed),
            ("COMPLETED", Some("FAILURE"), Failed),
            ("COMPLETED", Some("TIMED_OUT"), Failed),
            ("COMPLETED", Some("STARTUP_FAILURE"), Failed),
            ("COMPLETED", Some("CANCELLED"), Cancelled),
            ("COMPLETED", Some("SKIPPED"), Skipped),
            ("COMPLETED", Some("NEUTRAL"), Neutral),
            ("COMPLETED", Some("STALE"), Neutral),
            ("COMPLETED", Some("ACTION_REQUIRED"), ActionRequired),
            ("COMPLETED", Some("SOMETHING_NEW"), Neutral),
            ("", Some("SUCCESS"), Passed),
        ];
        for (status, conclusion, want) in cases {
            assert_eq!(run_state(status, conclusion), want, "{status} {conclusion:?}");
        }
    }

    #[test]
    fn status_context_states() {
        use RunState::*;
        for (state, want) in [("SUCCESS", Passed), ("FAILURE", Failed), ("ERROR", Failed), ("PENDING", Pending), ("EXPECTED", Pending)] {
            assert_eq!(status_state(state), want, "{state}");
        }
        assert_eq!(status_state("NEW"), Neutral);
    }

    #[test]
    fn the_query_asks_for_required_only_when_told() {
        assert!(query(true).contains("isRequired(pullRequestNumber: $number)"));
        assert_eq!(query(true).matches("isRequired").count(), 2);
        assert!(!query(false).contains("isRequired"));
        assert!(query(false).contains("after: $after"));
    }
}
