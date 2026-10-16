//! Azure Pipelines and posted statuses: a pull request's build policies and statuses, and a
//! branch tip's builds and commit statuses.

use super::azure::{V, project_base, repo_api};
use super::checks::{Outcome, Tally, TipQuery, per_tip};
use super::{Auth, Checks, Target, enc, get};
use crate::error::{Error, Result};
use serde::Deserialize;
use std::collections::HashMap;

/// The policy type id of "Build" (build validation) policies.
const BUILD_POLICY: &str = "0609b952-1397-4640-95ec-e00a01b2c241";

#[derive(Deserialize)]
struct List<T> {
    #[serde(default = "Vec::new")]
    value: Vec<T>,
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
    #[serde(rename = "type")]
    kind: PolicyType,
}

fn enabled() -> bool {
    true
}

#[derive(Deserialize)]
struct PolicyType {
    id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EvalContext {
    #[serde(default)]
    build_id: Option<u64>,
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
    target_url: Option<String>,
}

#[derive(Deserialize, Default, PartialEq, Eq, Hash, Clone)]
struct StatusContext {
    #[serde(default)]
    genre: String,
    #[serde(default)]
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Build {
    #[serde(default)]
    status: String,
    #[serde(default)]
    result: Option<String>,
    #[serde(default)]
    source_version: String,
    repository: BuildRepo,
    definition: Definition,
    #[serde(rename = "_links", default)]
    links: Option<BuildLinks>,
}

#[derive(Deserialize)]
struct BuildRepo {
    #[serde(default)]
    name: String,
}

#[derive(Deserialize)]
struct Definition {
    id: u64,
}

#[derive(Deserialize)]
struct BuildLinks {
    web: Option<Href>,
}

#[derive(Deserialize)]
struct Href {
    href: String,
}

/// Counts statuses, keeping only the latest (highest id) of each context.
fn count_statuses(tally: &mut Tally, mut statuses: Vec<Status>) {
    statuses.sort_by_key(|s| std::cmp::Reverse(s.id));
    let mut seen = Vec::new();
    for s in statuses {
        if seen.contains(&s.context) {
            continue;
        }
        seen.push(s.context.clone());
        let o = match s.state.as_str() {
            "succeeded" => Outcome::Passed,
            "failed" | "error" => Outcome::Failed,
            "pending" => Outcome::Pending,
            "notSet" | "notApplicable" => continue,
            _ => {
                tally.incomplete();
                continue;
            }
        };
        tally.add(o, s.target_url.as_deref());
    }
}

/// No access to builds (a token without the Build scope): counted as none.
fn no_access(e: &Error) -> bool {
    matches!(e, Error::Forbidden(_) | Error::Auth(_))
}

/// A pull request's build policies (build validation) and the statuses posted on it.
pub(super) fn pr_checks(t: &Target, auth: &Auth, project_id: &str, pr_id: u64) -> Result<Option<Checks>> {
    let mut tally = Tally::new();
    let artifact = format!("vstfs:///CodeReview/CodeReviewId/{project_id}/{pr_id}");
    let url = format!("{}/_apis/policy/evaluations?artifactId={}&api-version=7.1-preview.1", project_base(t), enc(&artifact));
    match get::<List<Evaluation>>(auth, &url) {
        Ok(list) => {
            for e in list.value {
                if e.configuration.kind.id != BUILD_POLICY || !e.configuration.is_enabled {
                    continue;
                }
                let o = match e.status.as_str() {
                    "approved" => Outcome::Passed,
                    "rejected" | "broken" => Outcome::Failed,
                    "running" | "queued" => Outcome::Pending,
                    "notApplicable" => continue,
                    _ => {
                        tally.incomplete();
                        continue;
                    }
                };
                let link = e.context.and_then(|c| c.build_id).map(|id| format!("{}/_build/results?buildId={id}", project_base(t)));
                tally.add(o, link.as_deref());
            }
        }
        Err(e) if no_access(&e) => {}
        Err(e) => return Err(e),
    }
    let statuses = get::<List<Status>>(auth, &format!("{}/pullRequests/{pr_id}/statuses?api-version={V}", repo_api(t)))?;
    count_statuses(&mut tally, statuses.value);
    Ok(tally.finish())
}

/// One tip's builds of this repository on its branch, the latest per pipeline, and its statuses.
fn tip_checks(t: &Target, auth: &Auth, q: &TipQuery) -> Result<Option<Checks>> {
    let mut tally = Tally::new();
    let url = format!(
        "{}/_apis/build/builds?branchName={}&queryOrder=queueTimeDescending&$top=20&api-version={V}",
        project_base(t),
        enc(&format!("refs/heads/{}", q.branch))
    );
    match get::<List<Build>>(auth, &url) {
        Ok(list) => {
            let mut definitions = Vec::new();
            // Newest first, so the first build of each pipeline is its latest run.
            for b in list.value {
                if b.source_version != q.sha || !b.repository.name.eq_ignore_ascii_case(&t.repo) || definitions.contains(&b.definition.id) {
                    continue;
                }
                definitions.push(b.definition.id);
                let o = match (b.status.as_str(), b.result.as_deref()) {
                    (s, _) if s != "completed" => Outcome::Pending,
                    (_, Some("succeeded" | "partiallySucceeded")) => Outcome::Passed,
                    (_, Some("failed" | "canceled")) => Outcome::Failed,
                    _ => {
                        tally.incomplete();
                        continue;
                    }
                };
                tally.add(o, b.links.and_then(|l| l.web).map(|h| h.href).as_deref());
            }
        }
        Err(e) if no_access(&e) => {}
        Err(e) => return Err(e),
    }
    let statuses = get::<List<Status>>(auth, &format!("{}/commits/{}/statuses?latestOnly=true&api-version={V}", repo_api(t), q.sha))?;
    count_statuses(&mut tally, statuses.value);
    Ok(tally.finish())
}

pub(super) fn commit_checks(t: &Target, auth: &Auth, tips: &[TipQuery]) -> Result<HashMap<String, Checks>> {
    per_tip(tips, |q| tip_checks(t, auth, q))
}
