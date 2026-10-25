//! Merging a Bitbucket pull request with the repository's strategies: the strategy names each
//! service uses, the head check before merging, Cloud's background merge, and Data Center's
//! version.

use super::super::bitbucket_dc::dc_repo;
use super::super::http::post_answer;
use super::super::links::https_host;
use super::super::{Auth, Target, get, post};
use super::bitbucket::pr_api;
use super::{MOVED, MergeMethod, MergeRequest, Merged, NOT_OPEN_MERGE, message_of, same_commit};
use crate::error::{Error, Result};
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;

/// How many times a background merge's status is asked for.
const POLLS: usize = 10;
/// The wait before each status poll (none in tests).
const PAUSE: Duration = if cfg!(test) { Duration::ZERO } else { Duration::from_secs(1) };

const PENDING: &str = "Bitbucket is still merging it. Reload in a moment to see whether it merged.";
const NOT_MERGED: &str = "Bitbucket didn't merge it. Reload to see its state.";

/// Cloud's strategy name as a merge method, for the strategies Glowfly can send.
fn cloud_method(name: &str) -> Option<MergeMethod> {
    match name {
        "merge_commit" => Some(MergeMethod::Merge),
        "squash" => Some(MergeMethod::Squash),
        "fast_forward" => Some(MergeMethod::FastForward),
        "rebase_fast_forward" => Some(MergeMethod::Rebase),
        _ => None,
    }
}

/// `methods` in order without repeats, `default` moved to the front when it is one of them.
fn default_first(methods: impl IntoIterator<Item = MergeMethod>, default: Option<MergeMethod>) -> Vec<MergeMethod> {
    let mut out: Vec<MergeMethod> = Vec::new();
    for m in methods {
        if !out.contains(&m) {
            out.push(m);
        }
    }
    if let Some(i) = default.and_then(|d| out.iter().position(|m| *m == d)) {
        let d = out.remove(i);
        out.insert(0, d);
    }
    out
}

/// The destination branch's merge strategies that Glowfly can send, the default first. Without
/// a list, Cloud's long-standing three.
pub(super) fn cloud_methods(list: Option<&[String]>, default: Option<&str>) -> Vec<MergeMethod> {
    let default = default.and_then(cloud_method);
    match list.filter(|l| !l.is_empty()) {
        Some(names) => default_first(names.iter().filter_map(|n| cloud_method(n)), default),
        None => default_first([MergeMethod::Merge, MergeMethod::Squash, MergeMethod::FastForward], default),
    }
}

/// The strategy name Cloud takes for `m`.
pub(super) fn cloud_strategy(m: MergeMethod) -> &'static str {
    match m {
        MergeMethod::Merge => "merge_commit",
        MergeMethod::Squash => "squash",
        MergeMethod::FastForward => "fast_forward",
        MergeMethod::Rebase => "rebase_fast_forward",
    }
}

/// Data Center's strategy id as a merge method, for the strategies Glowfly can send.
fn dc_method(id: &str) -> Option<MergeMethod> {
    match id {
        "no-ff" => Some(MergeMethod::Merge),
        "squash" => Some(MergeMethod::Squash),
        "ff-only" => Some(MergeMethod::FastForward),
        "rebase-ff-only" => Some(MergeMethod::Rebase),
        _ => None,
    }
}

/// The strategy id Data Center takes for `m`.
pub(super) fn dc_strategy(m: MergeMethod) -> &'static str {
    match m {
        MergeMethod::Merge => "no-ff",
        MergeMethod::Squash => "squash",
        MergeMethod::FastForward => "ff-only",
        MergeMethod::Rebase => "rebase-ff-only",
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(super) struct StrategyId {
    id: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(super) struct Strategy {
    id: String,
    /// Missing: listed, so enabled.
    enabled: Option<bool>,
}

/// A Data Center repository's merge settings.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct MergeConfig {
    default_strategy: Option<StrategyId>,
    strategies: Vec<Strategy>,
}

/// The repository's enabled strategies that Glowfly can send, the default first; a merge commit
/// alone when the settings couldn't be read.
pub(super) fn dc_methods(config: Option<&MergeConfig>) -> Vec<MergeMethod> {
    let Some(c) = config.filter(|c| !c.strategies.is_empty()) else { return vec![MergeMethod::Merge] };
    let enabled = c.strategies.iter().filter(|s| s.enabled.unwrap_or(true)).filter_map(|s| dc_method(&s.id));
    default_first(enabled, c.default_strategy.as_ref().and_then(|d| dc_method(&d.id)))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Commit {
    hash: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Source {
    commit: Option<Commit>,
}

/// The pull request as it is just before merging.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Head {
    state: String,
    source: Source,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Done {
    state: String,
    merge_commit: Option<Commit>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Task {
    task_status: String,
    merge_result: Option<Done>,
}

/// Fails unless the pull request is still open at the head the page showed.
fn still_open_at(state: &str, head: &str, req: &MergeRequest) -> Result<()> {
    if state != "OPEN" {
        Err(Error::Other(NOT_OPEN_MERGE.into()))
    } else if !same_commit(head, &req.head_sha) {
        Err(Error::Other(format!("{MOVED} merge.")))
    } else {
        Ok(())
    }
}

/// The commit message sent with the merge: only a merge commit or a squash makes one.
fn message_for(req: &MergeRequest) -> Option<String> {
    let message = message_of(req);
    (matches!(req.method, MergeMethod::Merge | MergeMethod::Squash) && !message.is_empty()).then_some(message)
}

fn cloud_refusal(e: Error) -> Error {
    match e {
        Error::Http { status: 555, .. } => Error::Other("Bitbucket took too long to merge it. Reload to see whether it merged.".into()),
        e => e,
    }
}

pub(super) fn cloud(t: &Target, auth: &Auth, id: u64, req: &MergeRequest) -> Result<Merged> {
    let api = pr_api(t, id);
    let head: Head = get(auth, &api)?;
    let hash = head.source.commit.map(|c| c.hash).unwrap_or_default();
    still_open_at(&head.state, &hash, req)?;
    let mut body = json!({ "type": "pullrequest", "merge_strategy": cloud_strategy(req.method) });
    if let Some(message) = message_for(req) {
        body["message"] = json!(message);
    }
    let answer = post_answer(auth, &format!("{api}/merge"), body).map_err(cloud_refusal)?;
    let done = if answer.status == 202 {
        wait(auth, answer.location.as_deref())?
    } else {
        serde_json::from_value(answer.body.unwrap_or_default())?
    };
    if done.state != "MERGED" {
        return Err(Error::Other(NOT_MERGED.into()));
    }
    Ok(Merged { sha: done.merge_commit.map(|c| c.hash).unwrap_or_default() })
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct DcFrom {
    latest_commit: String,
}

/// The Data Center pull request as it is just before merging.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct DcHead {
    version: u64,
    state: String,
    from_ref: DcFrom,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct DcProperties {
    merge_commit: Option<StrategyId>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct DcDone {
    state: String,
    properties: DcProperties,
}

/// A Data Center merge's failure in plain words: a 409 is a moved pull request or a veto, and
/// a 401 is a missing permission (the credential is fine: it just read the pull request).
fn dc_refusal(e: Error) -> Error {
    match e {
        Error::Http { status: 409, message } if message.to_lowercase().contains("out-of-date") => Error::Other(format!("{MOVED} merge.")),
        Error::Http { status: 409, message } if message.is_empty() => {
            Error::Other("Bitbucket can't merge it yet. Reload to see what blocks it.".into())
        }
        Error::Http { status: 409, message } => {
            Error::Other(format!("Bitbucket can't merge it yet ({message}). Reload to see what blocks it."))
        }
        Error::Auth(_) => {
            Error::Forbidden("Bitbucket says you can't merge this pull request; you need write access to the repository.".into())
        }
        e => e,
    }
}

pub(super) fn data_center(t: &Target, auth: &Auth, id: u64, req: &MergeRequest) -> Result<Merged> {
    let api = format!("{}/pull-requests/{id}", dc_repo(t));
    let head: DcHead = get(auth, &api)?;
    still_open_at(&head.state, &head.from_ref.latest_commit, req)?;
    let v = head.version;
    let mut body = json!({ "version": v, "strategyId": dc_strategy(req.method) });
    if let Some(message) = message_for(req) {
        body["message"] = json!(message);
    }
    let done: DcDone = post(auth, &format!("{api}/merge?version={v}"), body).map_err(dc_refusal)?;
    if done.state != "MERGED" {
        return Err(Error::Other(NOT_MERGED.into()));
    }
    Ok(Merged { sha: done.properties.merge_commit.map(|c| c.id).unwrap_or_default() })
}

/// Polls a background merge's task status until it has finished, a few times at most.
fn wait(auth: &Auth, location: Option<&str>) -> Result<Done> {
    let Some(url) = location.filter(|l| https_host(l).as_deref() == Some("api.bitbucket.org")) else {
        return Err(Error::Other(PENDING.into()));
    };
    for _ in 0..POLLS {
        std::thread::sleep(PAUSE);
        let task: Task = get(auth, url)?;
        match task.task_status.as_str() {
            "SUCCESS" => return Ok(task.merge_result.unwrap_or_else(|| Done { state: "MERGED".into(), merge_commit: None })),
            "PENDING" => {}
            _ => return Err(Error::Other(NOT_MERGED.into())),
        }
    }
    Err(Error::Other(PENDING.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use MergeMethod::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn cloud_methods_are_the_repositorys_default_first() {
        let cases: [(&[&str], Option<&str>, &[MergeMethod]); 7] = [
            (&["merge_commit", "squash", "fast_forward"], Some("squash"), &[Squash, Merge, FastForward]),
            (&["fast_forward"], None, &[FastForward]),
            (&["rebase_fast_forward", "merge_commit"], Some("rebase_fast_forward"), &[Rebase, Merge]),
            (&["squash_fast_forward", "rebase_merge"], Some("squash_fast_forward"), &[]),
            (&["merge_commit", "squash"], Some("fast_forward"), &[Merge, Squash]),
            (&["squash", "merge_commit", "squash"], None, &[Squash, Merge]),
            (&[], Some("fast_forward"), &[FastForward, Merge, Squash]),
        ];
        for (list, default, want) in cases {
            assert_eq!(cloud_methods(Some(&names(list)), default), want, "{list:?} {default:?}");
        }
        assert_eq!(cloud_methods(None, None), [Merge, Squash, FastForward]);
        assert_eq!(cloud_methods(None, Some("squash")), [Squash, Merge, FastForward]);
    }

    #[test]
    fn cloud_strategies_round_trip() {
        for m in [Merge, Squash, FastForward, Rebase] {
            assert_eq!(cloud_methods(Some(&names(&[cloud_strategy(m)])), None), [m], "{m:?}");
        }
    }

    fn config(json: &str) -> MergeConfig {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn dc_methods_are_the_enabled_strategies_default_first() {
        let all = config(
            r#"{"defaultStrategy":{"id":"squash"},"strategies":[{"id":"no-ff","enabled":true},{"id":"ff","enabled":true},
            {"id":"ff-only","enabled":false},{"id":"rebase-no-ff","enabled":true},{"id":"rebase-ff-only","enabled":true},
            {"id":"squash","enabled":true},{"id":"squash-ff-only","enabled":true}]}"#,
        );
        assert_eq!(dc_methods(Some(&all)), [Squash, Merge, Rebase], "only enabled ones Glowfly can send");
        let ff = config(r#"{"defaultStrategy":{"id":"ff"},"strategies":[{"id":"ff","enabled":true},{"id":"ff-only","enabled":true}]}"#);
        assert_eq!(dc_methods(Some(&ff)), [FastForward], "a default that can't be sent is left out");
        let none = config(r#"{"defaultStrategy":{"id":"ff"},"strategies":[{"id":"ff","enabled":true}]}"#);
        assert_eq!(dc_methods(Some(&none)), []);
        assert_eq!(dc_methods(None), [Merge]);
        assert_eq!(dc_methods(Some(&config("{}"))), [Merge], "no strategies listed");
    }

    #[test]
    fn dc_strategy_ids() {
        let ids: Vec<&str> = [Merge, Squash, FastForward, Rebase].into_iter().map(dc_strategy).collect();
        assert_eq!(ids, ["no-ff", "squash", "ff-only", "rebase-ff-only"]);
        for m in [Merge, Squash, FastForward, Rebase] {
            assert_eq!(dc_method(dc_strategy(m)), Some(m));
        }
    }

    #[test]
    fn commits_match_by_prefix() {
        let full = "0123456789abcdef0123456789abcdef01234567";
        assert!(same_commit("0123456789ab", full));
        assert!(same_commit(full, "0123456789AB"), "without case");
        assert!(!same_commit("012345", full), "too short to tell");
        assert!(same_commit("", full) && same_commit(full, ""), "nothing to compare");
        assert!(!same_commit("fedcba987654", full));
        assert!(same_commit(full, full));
    }

    #[test]
    fn only_a_merge_commit_or_squash_takes_a_message() {
        let req = |method| MergeRequest { method, head_sha: "h".into(), title: "Title".into(), message: "Body".into() };
        assert_eq!(message_for(&req(Merge)).as_deref(), Some("Title\n\nBody"));
        assert_eq!(message_for(&req(Squash)).as_deref(), Some("Title\n\nBody"));
        assert_eq!(message_for(&req(FastForward)), None);
        assert_eq!(message_for(&req(Rebase)), None);
    }
}
