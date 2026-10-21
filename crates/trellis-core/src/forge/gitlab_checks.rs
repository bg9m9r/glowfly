//! GitLab pipelines: a merge request's head pipeline, and the last pipeline of a branch tip.
//! GitLab reports one overall state per pipeline, so the summary names the state alone; the
//! pull request panel lists the head pipeline's jobs (`check_list`).

use super::check_detail::{CheckList, CheckQuery, CheckRun, RunState, time_of};
use super::checks::{TipQuery, per_tip};
use super::gitlab::project_api;
use super::{Auth, CheckState, Checks, Target, get, page};
use crate::error::Result;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
struct Pipeline {
    #[serde(default)]
    id: u64,
    #[serde(default)]
    sha: String,
    status: String,
    #[serde(default)]
    web_url: String,
    #[serde(default)]
    detailed_status: Option<Detailed>,
}

#[derive(Deserialize)]
struct Detailed {
    #[serde(default)]
    group: String,
}

/// A pipeline's state; None for a status word not known here.
fn pipeline_state(status: &str, group: &str) -> Option<CheckState> {
    if group == "success-with-warnings" {
        return Some(CheckState::Passed);
    }
    match status {
        "success" | "skipped" => Some(CheckState::Passed),
        "failed" | "canceled" => Some(CheckState::Failed),
        "created" | "waiting_for_resource" | "preparing" | "pending" | "running" | "manual" | "scheduled" => Some(CheckState::Pending),
        _ => None,
    }
}

fn to_checks(p: Option<Pipeline>) -> Option<Checks> {
    let p = p?;
    let group = p.detailed_status.map(|d| d.group).unwrap_or_default();
    pipeline_state(&p.status, &group).map(|s| Checks::state_only(s, p.web_url))
}

/// The merge request's head pipeline, if it has one.
pub(super) fn head_pipeline(t: &Target, auth: &Auth, iid: u64) -> Result<Option<Checks>> {
    #[derive(Deserialize)]
    struct Mr {
        #[serde(default)]
        head_pipeline: Option<Pipeline>,
    }
    let mr: Mr = get(auth, &format!("{}/merge_requests/{iid}", project_api(t)))?;
    Ok(to_checks(mr.head_pipeline))
}

/// The last pipeline of each commit.
pub(super) fn commit_checks(t: &Target, auth: &Auth, tips: &[TipQuery]) -> Result<HashMap<String, Checks>> {
    #[derive(Deserialize)]
    struct Commit {
        #[serde(default)]
        last_pipeline: Option<Pipeline>,
    }
    per_tip(tips, |q| {
        let c: Commit = get(auth, &format!("{}/repository/commits/{}", project_api(t), q.sha))?;
        Ok(to_checks(c.last_pipeline))
    })
}

/// One job of a pipeline (the latest attempt: retried ones aren't listed).
#[derive(Deserialize)]
struct Job {
    #[serde(default)]
    id: u64,
    #[serde(default)]
    name: String,
    #[serde(default)]
    stage: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    allow_failure: bool,
    #[serde(default)]
    started_at: Option<String>,
    #[serde(default)]
    finished_at: Option<String>,
    #[serde(default)]
    web_url: String,
    #[serde(default)]
    failure_reason: Option<String>,
}

/// A job's state (manual: waits for someone to start it).
fn job_state(status: &str) -> RunState {
    match status {
        "created" | "waiting_for_resource" | "preparing" | "pending" | "scheduled" | "waiting_for_callback" => RunState::Pending,
        "running" => RunState::Running,
        "success" => RunState::Passed,
        "failed" => RunState::Failed,
        "canceled" | "canceling" => RunState::Cancelled,
        "skipped" => RunState::Skipped,
        "manual" => RunState::ActionRequired,
        _ => RunState::Neutral,
    }
}

fn to_run(j: Job) -> CheckRun {
    let state = job_state(&j.status);
    let mut description = j.failure_reason.unwrap_or_default().replace('_', " ");
    if j.allow_failure && state == RunState::Failed {
        description = if description.is_empty() { "allowed to fail".into() } else { format!("{description} (allowed to fail)") };
    }
    CheckRun {
        name: j.name,
        group: Some(j.stage).filter(|s| !s.is_empty()),
        state,
        required: Some(!j.allow_failure),
        started: time_of(&j.started_at),
        finished: time_of(&j.finished_at),
        url: j.web_url,
        description,
    }
}

/// The jobs of the merge request's head pipeline, in pipeline order; none without one.
pub(super) fn check_list(t: &Target, auth: &Auth, q: &CheckQuery) -> Result<CheckList> {
    #[derive(Deserialize)]
    struct Mr {
        #[serde(default)]
        sha: String,
        #[serde(default)]
        head_pipeline: Option<Pipeline>,
    }
    let mr: Mr = get(auth, &format!("{}/merge_requests/{}", project_api(t), q.id))?;
    let Some(p) = mr.head_pipeline.filter(|p| p.id != 0) else { return Ok(CheckList::empty(mr.sha)) };
    let mut jobs: Vec<Job> = page::by_link(auth, format!("{}/pipelines/{}/jobs?per_page=100", project_api(t), p.id))?;
    // Listed newest first; oldest first keeps the stages in order.
    jobs.sort_by_key(|j| j.id);
    let sha = if p.sha.is_empty() { mr.sha } else { p.sha };
    Ok(CheckList { sha, runs: jobs.into_iter().map(to_run).collect(), complete: true })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipeline_states() {
        use CheckState::*;
        let cases = [
            ("success", "", Some(Passed)),
            ("skipped", "", Some(Passed)),
            ("success", "success-with-warnings", Some(Passed)),
            ("failed", "", Some(Failed)),
            ("canceled", "", Some(Failed)),
            ("manual", "", Some(Pending)),
            ("scheduled", "", Some(Pending)),
            ("running", "", Some(Pending)),
            ("waiting_for_resource", "", Some(Pending)),
            ("something_new", "", None),
        ];
        for (status, group, want) in cases {
            assert_eq!(pipeline_state(status, group), want, "{status} {group}");
        }
    }

    #[test]
    fn job_states() {
        use RunState::*;
        let cases = [
            ("created", Pending),
            ("waiting_for_resource", Pending),
            ("preparing", Pending),
            ("pending", Pending),
            ("scheduled", Pending),
            ("running", Running),
            ("success", Passed),
            ("failed", Failed),
            ("canceled", Cancelled),
            ("canceling", Cancelled),
            ("skipped", Skipped),
            ("manual", ActionRequired),
            ("something_new", Neutral),
        ];
        for (status, want) in cases {
            assert_eq!(job_state(status), want, "{status}");
        }
    }
}
