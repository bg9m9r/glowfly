//! GitLab pipelines: a merge request's head pipeline, and the last pipeline of a branch tip.
//! GitLab reports one overall state per pipeline, so the summary names the state alone.

use super::checks::{TipQuery, per_tip};
use super::gitlab::project_api;
use super::{Auth, CheckState, Checks, Target, get};
use crate::error::Result;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
struct Pipeline {
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
}
