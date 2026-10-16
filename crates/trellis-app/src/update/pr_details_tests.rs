//! The second pass over pull requests past the first `CHECKED_PRS`, on services that read
//! checks and votes one pull request at a time.

use super::tests::state;
use super::update;
use crate::effect::{Effect, ForgeCall, ForgeError, ForgeReply, ForgeResult};
use crate::msg::{ForgeMsg, Msg};
use crate::state::{AppState, ForgeState, Status};
use std::time::{Duration, Instant};
use trellis_core::forge::{CheckState, Checks, PrDetails, Provider, PullRequest, Reviewer, Target};

fn host(provider: Provider, host: &str) -> AppState {
    let t = Target { provider, host: host.into(), owner: "g".into(), project: String::new(), repo: "app".into() };
    let mut st = state();
    st.forge = Some(ForgeState::new(Some(t.clone()), "origin".into(), Some(t)));
    st
}

fn head(id: u64) -> String {
    format!("{id:040x}")
}

fn pr(id: u64, head: &str) -> PullRequest {
    PullRequest {
        id,
        title: format!("P{id}"),
        description: String::new(),
        source: format!("b{id}"),
        target: "main".into(),
        author: "ada".into(),
        draft: false,
        reviewers: vec![Reviewer { name: "Grace".into(), state: "no vote".into(), required: false }],
        head: Some(head.into()),
        merge_status: None,
        checks: None,
        url: String::new(),
    }
}

fn prs(count: u64) -> Vec<PullRequest> {
    (1..=count).map(|i| pr(i, &head(i))).collect()
}

fn passed() -> Option<Checks> {
    Some(Checks::state_only(CheckState::Passed, String::new()))
}

/// Read details for `ids`: passed checks, and Grace's approval.
fn answers(ids: &[u64]) -> Vec<PrDetails> {
    let grace = Reviewer { name: "Grace".into(), state: "approved".into(), required: false };
    ids.iter().map(|&id| PrDetails { id, head: Some(head(id)), checks: passed(), votes: Some(vec![grace.clone()]) }).collect()
}

/// The ids of each details chunk the effects ask for.
fn chunks(fx: Vec<Effect>) -> Vec<Vec<u64>> {
    fx.into_iter()
        .filter_map(|e| match e {
            Effect::Forge { call: ForgeCall::PrDetails(c), .. } => Some(c.iter().map(|p| p.id).collect()),
            _ => None,
        })
        .collect()
}

fn reply(st: &mut AppState, result: ForgeResult, now: Instant) -> Vec<Vec<u64>> {
    let target = st.forge.as_ref().unwrap().host.clone().unwrap();
    let reply = ForgeReply { target, auth: None, results: vec![result] };
    chunks(update(st, Msg::Forge(ForgeMsg::ForgeReplied(reply)), now))
}

fn list(st: &mut AppState, prs: Vec<PullRequest>, now: Instant) -> Vec<Vec<u64>> {
    reply(st, ForgeResult::Prs(Ok(prs)), now)
}

fn details(st: &mut AppState, ids: &[u64], now: Instant) -> Vec<Vec<u64>> {
    reply(st, ForgeResult::PrDetails(Ok(answers(ids))), now)
}

fn ids(r: std::ops::RangeInclusive<u64>) -> Vec<u64> {
    r.collect()
}

fn fs(st: &AppState) -> &ForgeState {
    st.forge.as_ref().unwrap()
}

/// A GitLab host with 60 pull requests whose details have all been read at `now`.
fn read_all(now: Instant) -> AppState {
    let mut st = host(Provider::GitLab, "gitlab.com");
    assert_eq!(list(&mut st, prs(60), now), [ids(26..=50)]);
    assert_eq!(details(&mut st, &ids(26..=50), now), [ids(51..=60)]);
    assert!(details(&mut st, &ids(51..=60), now).is_empty());
    st
}

#[test]
fn details_past_the_first_25_are_read_a_chunk_at_a_time() {
    let now = Instant::now();
    let mut st = host(Provider::GitLab, "gitlab.com");
    assert_eq!(list(&mut st, prs(60), now), [ids(26..=50)]);
    assert!(fs(&st).prs[30].checks.is_none());
    assert_eq!(details(&mut st, &ids(26..=50), now), [ids(51..=60)]);
    let p = &fs(&st).prs[30];
    assert_eq!((p.checks.clone(), p.approvals()), (passed(), 1));
    assert!(fs(&st).prs[55].checks.is_none());
    assert!(details(&mut st, &ids(51..=60), now).is_empty());
    assert_eq!((fs(&st).prs[55].approvals(), fs(&st).loading), (1, 0));
}

#[test]
fn services_that_list_details_need_no_second_pass() {
    let mut st = host(Provider::GitHub, "github.com");
    assert!(list(&mut st, prs(60), Instant::now()).is_empty());
    assert!(fs(&st).details.cache.is_empty());
}

#[test]
fn a_refreshed_list_reuses_fresh_details_for_the_same_head() {
    let now = Instant::now();
    let mut st = read_all(now);
    // A minute later the list comes back without details past the first 25.
    assert!(list(&mut st, prs(60), now + Duration::from_secs(60)).is_empty());
    let p = &fs(&st).prs[30];
    assert_eq!((p.checks.clone(), p.approvals()), (passed(), 1), "shown from the cache");
    // After 16 minutes, every one is read again.
    assert_eq!(list(&mut st, prs(60), now + Duration::from_secs(16 * 60)), [ids(26..=50)]);
    assert_eq!(fs(&st).details.queue.len(), 10);

    // A new head: the old checks aren't shown for it, and only it is read again.
    let mut st = read_all(now);
    let mut moved = prs(60);
    moved[39].head = Some("f".repeat(40));
    assert_eq!(list(&mut st, moved, now + Duration::from_secs(60)), [vec![40]]);
    assert_eq!(fs(&st).prs[39].checks, None);
    assert_eq!(fs(&st).prs[38].checks, passed());
}

#[test]
fn a_failed_chunk_stops_the_pass_and_keeps_what_is_shown() {
    let now = Instant::now();
    let mut st = host(Provider::GitLab, "gitlab.com");
    list(&mut st, prs(60), now);
    assert_eq!(details(&mut st, &ids(26..=50), now), [ids(51..=60)]);
    let err = ForgeError { message: "rate limited".into(), auth: false };
    assert!(reply(&mut st, ForgeResult::PrDetails(Err(err)), now).is_empty());
    let fs = fs(&st);
    assert_eq!(fs.host_status, Status::Ok);
    assert_eq!(fs.prs[30].checks, passed());
    assert!(fs.details.queue.is_empty() && !fs.details.in_flight);
}

#[test]
fn a_manual_refresh_reads_every_detail_again() {
    let now = Instant::now();
    let mut st = read_all(now);
    let later = now + Duration::from_secs(60);
    update(&mut st, Msg::Forge(ForgeMsg::ForgeRefresh { interactive: true }), later);
    assert_eq!(list(&mut st, prs(60), later), [ids(26..=50)]);
}

#[test]
fn disconnecting_or_parking_drops_the_pass() {
    let now = Instant::now();
    let mut st = host(Provider::GitLab, "gitlab.com");
    assert_eq!(list(&mut st, prs(60), now), [ids(26..=50)]);
    st.forge.as_mut().unwrap().replies_dropped();
    assert!(!fs(&st).details.in_flight);
    assert_eq!(list(&mut st, prs(60), now), [ids(26..=50)], "the next list starts the pass again");

    let account = fs(&st).host.as_ref().unwrap().account();
    update(&mut st, Msg::Forge(ForgeMsg::Disconnect(account)), now);
    let d = &fs(&st).details;
    assert!(d.cache.is_empty() && d.queue.is_empty() && !d.in_flight);
}
