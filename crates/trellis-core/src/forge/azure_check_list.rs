//! Every check on an Azure DevOps pull request, for the pull request panel: its build policies
//! (build validation, required when blocking) with the build each one queued, and the statuses
//! posted on it (required when a blocking status policy names them).

use super::azure::{V, project_base, repo_api};
use super::azure_checks::{BUILD_POLICY, List, enabled, no_access};
use super::check_detail::{CheckList, CheckQuery, CheckRun, RunState, time_of};
use super::{Auth, Target, enc, get};
use crate::error::Result;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
struct RepoInfo {
    #[serde(default)]
    project: Option<Project>,
}

#[derive(Deserialize)]
struct Project {
    #[serde(default)]
    id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Evaluation {
    #[serde(default)]
    status: String,
    configuration: Configuration,
    #[serde(default)]
    context: Option<EvalContext>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Configuration {
    #[serde(default = "enabled")]
    is_enabled: bool,
    #[serde(default)]
    is_blocking: bool,
    #[serde(rename = "type")]
    kind: PolicyType,
    #[serde(default)]
    settings: Settings,
}

#[derive(Deserialize)]
struct PolicyType {
    #[serde(default)]
    id: String,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Settings {
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    manual_queue_only: bool,
    /// Set on status policies only.
    #[serde(default)]
    status_name: Option<String>,
    #[serde(default)]
    status_genre: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EvalContext {
    #[serde(default)]
    build_id: Option<u64>,
    #[serde(default)]
    is_expired: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Build {
    #[serde(default)]
    result: Option<String>,
    #[serde(default)]
    start_time: Option<String>,
    #[serde(default)]
    finish_time: Option<String>,
    #[serde(default)]
    definition: Option<Definition>,
    #[serde(rename = "_links", default)]
    links: Option<Links>,
}

#[derive(Deserialize)]
struct Definition {
    #[serde(default)]
    name: String,
}

#[derive(Deserialize)]
struct Links {
    #[serde(default)]
    web: Option<Href>,
}

#[derive(Deserialize)]
struct Href {
    #[serde(default)]
    href: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Status {
    #[serde(default)]
    id: u64,
    #[serde(default)]
    state: String,
    #[serde(default)]
    context: StatusContext,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    target_url: Option<String>,
    #[serde(default)]
    updated_date: Option<String>,
}

#[derive(Deserialize, Default)]
struct StatusContext {
    #[serde(default)]
    genre: String,
    #[serde(default)]
    name: String,
}

/// A status policy's key: (genre, name), lowercased.
fn status_key(genre: &str, name: &str) -> (String, String) {
    (genre.to_lowercase(), name.to_lowercase())
}

/// A build policy as a check, before its build is read.
fn policy_run(t: &Target, e: &Evaluation) -> CheckRun {
    let ctx = e.context.as_ref();
    let build = ctx.and_then(|c| c.build_id);
    let s = &e.configuration.settings;
    let (state, mut description) = match e.status.as_str() {
        "approved" => (RunState::Passed, ""),
        "rejected" => (RunState::Failed, ""),
        "broken" => (RunState::Failed, "The policy is broken"),
        "running" => (RunState::Running, ""),
        "notApplicable" => (RunState::Skipped, ""),
        "queued" if build.is_none() && s.manual_queue_only => (RunState::ActionRequired, "Queue the build to run it"),
        "queued" => (RunState::Pending, ""),
        _ => (RunState::Neutral, ""),
    };
    if ctx.is_some_and(|c| c.is_expired) {
        description = "Expired: ran on an older commit";
    }
    CheckRun {
        name: s.display_name.clone().filter(|n| !n.trim().is_empty()).unwrap_or_default(),
        group: None,
        state,
        required: Some(e.configuration.is_blocking),
        started: None,
        finished: None,
        url: build.map(|id| format!("{}/_build/results?buildId={id}", project_base(t))).unwrap_or_default(),
        description: description.to_owned(),
    }
}

/// `run` with what its build says: the pipeline's name, times, page, and a cancelled run.
fn with_build(mut run: CheckRun, b: Build) -> CheckRun {
    if run.name.is_empty() {
        run.name = b.definition.map(|d| d.name).unwrap_or_default();
    }
    if run.state == RunState::Failed && b.result.as_deref() == Some("canceled") {
        run.state = RunState::Cancelled;
    }
    run.started = time_of(&b.start_time);
    run.finished = time_of(&b.finish_time);
    if let Some(href) = b.links.and_then(|l| l.web).map(|w| w.href).filter(|h| !h.is_empty()) {
        run.url = href;
    }
    run
}

fn status_state(state: &str) -> RunState {
    match state {
        "succeeded" => RunState::Passed,
        "failed" | "error" => RunState::Failed,
        "pending" => RunState::Pending,
        "notApplicable" => RunState::Skipped,
        _ => RunState::Neutral,
    }
}

/// The latest status of each context; `blocking` is None when the policies couldn't be read.
fn status_runs(mut statuses: Vec<Status>, blocking: Option<&HashMap<(String, String), bool>>) -> Vec<CheckRun> {
    statuses.sort_by_key(|s| std::cmp::Reverse(s.id));
    let mut seen = Vec::new();
    let mut out = Vec::new();
    for s in statuses {
        let key = status_key(&s.context.genre, &s.context.name);
        if seen.contains(&key) {
            continue;
        }
        let state = status_state(&s.state);
        let name = if s.context.genre.is_empty() { s.context.name } else { format!("{}/{}", s.context.genre, s.context.name) };
        out.push(CheckRun {
            name,
            group: None,
            state,
            required: blocking.map(|b| b.get(&key).copied().unwrap_or(false)),
            started: None,
            finished: if state.is_active() { None } else { time_of(&s.updated_date) },
            url: s.target_url.unwrap_or_default(),
            description: s.description.unwrap_or_default(),
        });
        seen.push(key);
    }
    // The service's order, oldest first.
    out.reverse();
    out
}

pub(super) fn check_list(t: &Target, auth: &Auth, q: &CheckQuery) -> Result<CheckList> {
    let info: RepoInfo = get(auth, &format!("{}?api-version={V}", repo_api(t)))?;
    let project = info.project.map(|p| p.id).unwrap_or_default();
    if project.is_empty() {
        return Ok(CheckList::empty(""));
    }
    let artifact = format!("vstfs:///CodeReview/CodeReviewId/{project}/{}", q.id);
    let url = format!("{}/_apis/policy/evaluations?artifactId={}&api-version=7.1-preview.1", project_base(t), enc(&artifact));
    let mut runs = Vec::new();
    let blocking = match get::<List<Evaluation>>(auth, &url) {
        Ok(list) => {
            let mut blocking = HashMap::new();
            for e in list.value.into_iter().filter(|e| e.configuration.is_enabled) {
                let s = &e.configuration.settings;
                if e.configuration.kind.id == BUILD_POLICY {
                    let run = policy_run(t, &e);
                    let build = e.context.as_ref().and_then(|c| c.build_id);
                    let run =
                        match build.map(|id| get::<Build>(auth, &format!("{}/_apis/build/builds/{id}?api-version={V}", project_base(t)))) {
                            Some(Ok(b)) => with_build(run, b),
                            Some(Err(err)) if !no_access(&err) && !err.is_not_found() => return Err(err),
                            _ => run,
                        };
                    runs.push(CheckRun { name: if run.name.is_empty() { "Build".into() } else { run.name.clone() }, ..run });
                } else if let Some(name) = &s.status_name {
                    blocking.insert(status_key(s.status_genre.as_deref().unwrap_or_default(), name), e.configuration.is_blocking);
                }
            }
            Some(blocking)
        }
        Err(e) if no_access(&e) => None,
        Err(e) => return Err(e),
    };
    let statuses = get::<List<Status>>(auth, &format!("{}/pullRequests/{}/statuses?api-version={V}", repo_api(t), q.id))?;
    runs.extend(status_runs(statuses.value, blocking.as_ref()));
    Ok(CheckList { sha: String::new(), runs, complete: true })
}
