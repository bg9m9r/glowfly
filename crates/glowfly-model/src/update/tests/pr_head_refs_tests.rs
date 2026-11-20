//! Headless tests for the refs that keep a pull request's head while a panel shows it
//! (`refs/glowfly/pr/<id>`): held when the files are compared from a head that was already here
//! or when the head fetch starts or answers, deleted once no panel copy in the repository shows
//! the pull request (closing, another pull request, a view replacing the panel, parked and
//! worktree tabs, a late fetch answer), and swept when a repository opens.

use super::tests::state;
use super::update;
use crate::effect::{Effect, EffectMeta, LoadKind, SandboxRule};
use crate::msg::{HistoryMsg, Msg, RepoMsg, TabsMsg};
use crate::pull_request::{HeadRefJob, PrEffect, PullRequestMsg};
use crate::state::{AppState, ForgeState, Tab};
use git2::Oid;
use glowfly_core::forge::{Provider, PullRequest, Target};
use glowfly_core::repo::{PrCompare, RemoteInfo, SourceFrom, SourceTip};
use std::path::PathBuf;
use std::time::Instant;

const HEAD: &str = "0123456789abcdef0123456789abcdef01234567";

fn target() -> Target {
    Target { provider: Provider::GitHub, host: "github.com".into(), owner: "o".into(), project: String::new(), repo: "r".into() }
}

fn pull_request(id: u64) -> PullRequest {
    PullRequest {
        id,
        title: "Add search".into(),
        description: String::new(),
        source: "feature".into(),
        target: "main".into(),
        author: "ada".into(),
        draft: false,
        reviewers: Vec::new(),
        head: Some(HEAD.into()),
        head_repo: Default::default(),
        merge_status: None,
        checks: None,
        url: String::new(),
        facets: Default::default(),
    }
}

fn forge() -> ForgeState {
    let mut fs = ForgeState::new(Some(target()), "origin".into(), Some(target()));
    fs.prs = vec![pull_request(7), pull_request(8)];
    fs
}

/// A repository on GitHub listing pull requests 7 and 8, in tab 0, with an `origin` remote.
fn setup(name: &str) -> AppState {
    let mut st = state();
    let mut snap = crate::state::op_fixture::snapshot(name);
    snap.remote_info.push(RemoteInfo { name: "origin".into(), urls: vec!["https://example.com/o/r.git".into()], ..Default::default() });
    st.tabs[0] = Tab::unloaded(snap.workdir.clone());
    st.snap = Some(snap);
    st.session.forge = Some(forge());
    st
}

fn send(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn pr(st: &mut AppState, msg: PullRequestMsg) -> Vec<Effect> {
    send(st, Msg::PullRequest(msg))
}

fn head_oid() -> Oid {
    Oid::from_str(HEAD).unwrap()
}

/// What the head ref jobs among `fx` do: ("keep" | "drop" | "sweep", folder, pull requests).
fn jobs(fx: &[Effect]) -> Vec<(&'static str, PathBuf, Vec<u64>)> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::PullRequest(PrEffect::HeadRef(j)) => Some(match j {
                HeadRefJob::Keep { workdir, id, oid } => {
                    assert_eq!(*oid, head_oid());
                    ("keep", workdir.clone(), vec![*id])
                }
                HeadRefJob::Drop { workdir, ids } => ("drop", workdir.clone(), ids.clone()),
                HeadRefJob::Sweep { workdir, keep } => ("sweep", workdir.clone(), keep.clone()),
            }),
            _ => None,
        })
        .collect()
}

/// A message that replaces the panel, made for the state it is sent in.
type Path = fn(&AppState) -> Msg;

fn held(st: &AppState) -> Vec<u64> {
    st.pull_requests.head_refs.held.iter().map(|h| h.id).collect()
}

fn workdir(st: &AppState) -> PathBuf {
    st.workdir().unwrap()
}

fn compared(from: SourceFrom) -> PrCompare {
    PrCompare::Unrelated(SourceTip { oid: head_oid(), from })
}

/// Opens pull request 7 and answers its files as compared from a source of kind `from`.
fn opened_and_compared(st: &mut AppState, from: SourceFrom) -> Vec<Effect> {
    pr(st, PullRequestMsg::Open(7));
    let generation = st.session.pull_request.generation;
    pr(st, PullRequestMsg::FilesLoaded { id: 7, generation, result: Ok(compared(from)) })
}

#[test]
fn a_head_already_here_is_kept_once() {
    let mut st = setup("prh-keep");
    let w = workdir(&st);
    let fx = opened_and_compared(&mut st, SourceFrom::Head);
    assert_eq!(jobs(&fx), [("keep", w, vec![7])]);
    assert_eq!(held(&st), [7]);
    assert_eq!(st.session.pull_request.view.as_ref().unwrap().kept_head, Some(head_oid()));

    let generation = st.session.pull_request.generation;
    let fx = pr(&mut st, PullRequestMsg::FilesLoaded { id: 7, generation, result: Ok(compared(SourceFrom::Head)) });
    assert!(jobs(&fx).is_empty(), "asked once: {fx:?}");
    assert_eq!(held(&st), [7]);

    let mut st = setup("prh-branch");
    let fx = opened_and_compared(&mut st, SourceFrom::Branch);
    assert!(jobs(&fx).is_empty() && held(&st).is_empty(), "a remote-tracking branch is already a ref");
}

#[test]
fn close_drops_the_held_ref() {
    let mut st = setup("prh-close");
    let w = workdir(&st);
    opened_and_compared(&mut st, SourceFrom::Head);
    let fx = pr(&mut st, PullRequestMsg::Close);
    assert_eq!(jobs(&fx), [("drop", w, vec![7])]);
    assert!(held(&st).is_empty());
    assert!(jobs(&pr(&mut st, PullRequestMsg::Close)).is_empty());
}

#[test]
fn opening_another_pr_and_replacing_views_drop_it() {
    let mut st = setup("prh-another");
    let w = workdir(&st);
    opened_and_compared(&mut st, SourceFrom::Head);
    let fx = pr(&mut st, PullRequestMsg::Open(8));
    assert_eq!(jobs(&fx), [("drop", w, vec![7])]);
    assert!(held(&st).is_empty());

    let replacing: [(&str, Path); 2] = [
        ("file history", |_| Msg::History(HistoryMsg::OpenFileHistory { path: "a.txt".into(), at: None })),
        ("commit", |st| Msg::Repo(RepoMsg::SelectCommit(st.snap.as_ref().and_then(|s| s.head_oid).unwrap()))),
    ];
    for (name, msg) in replacing {
        let mut st = setup("prh-replace");
        opened_and_compared(&mut st, SourceFrom::Head);
        let msg = msg(&st);
        let fx = send(&mut st, msg);
        assert!(st.session.pull_request.view.is_none(), "{name}");
        assert_eq!(jobs(&fx), [("drop", workdir(&st), vec![7])], "{name}");
    }
}

#[test]
fn a_parked_copy_keeps_the_ref() {
    let mut st = setup("prh-parked");
    let w = workdir(&st);
    opened_and_compared(&mut st, SourceFrom::Head);
    let fx = send(&mut st, Msg::Tabs(TabsMsg::NewTab));
    assert!(jobs(&fx).is_empty(), "{fx:?}");
    assert_eq!(held(&st), [7]);
    let fx = send(&mut st, Msg::Tabs(TabsMsg::CloseTab(0)));
    assert_eq!(jobs(&fx), [("drop", w, vec![7])]);
    assert!(held(&st).is_empty());
}

#[test]
fn a_worktree_tab_keeps_it() {
    let mut st = setup("prh-worktree-a");
    let first = workdir(&st);
    let common = st.snap.as_ref().unwrap().common.clone();
    opened_and_compared(&mut st, SourceFrom::Head);
    send(&mut st, Msg::Tabs(TabsMsg::NewTab));

    // Another worktree of the same repository, in the new tab, showing the same pull request.
    let mut snap = crate::state::op_fixture::snapshot("prh-worktree-b");
    snap.common = common;
    let second = snap.workdir.clone();
    st.tabs[1] = Tab::unloaded(second.clone());
    st.snap = Some(snap);
    st.session.forge = Some(forge());
    opened_and_compared(&mut st, SourceFrom::Head);
    assert_eq!(held(&st), [7, 7]);

    let fx = pr(&mut st, PullRequestMsg::Close);
    assert!(jobs(&fx).is_empty(), "the first tab still shows it: {fx:?}");
    assert_eq!(held(&st), [7, 7]);
    let fx = send(&mut st, Msg::Tabs(TabsMsg::CloseTab(0)));
    assert_eq!(jobs(&fx), [("drop", first, vec![7]), ("drop", second, vec![7])]);
    assert!(held(&st).is_empty());
}

#[test]
fn the_fetch_holds_and_a_late_answer_is_dropped() {
    let mut st = setup("prh-fetch");
    let w = workdir(&st);
    pr(&mut st, PullRequestMsg::Open(7));
    let generation = st.session.pull_request.generation;
    let fx = pr(&mut st, PullRequestMsg::FilesLoaded { id: 7, generation, result: Ok(PrCompare::NoSource) });
    let cmd = fx
        .iter()
        .find_map(|e| match e {
            Effect::PullRequest(PrEffect::FetchHead(j)) => Some(j.cmd.clone()),
            _ => None,
        })
        .expect("the head is fetched");
    assert_eq!(&cmd[cmd.len() - 3..], ["--", "origin", "+refs/pull/7/head:refs/glowfly/pr/7"]);
    assert_eq!(held(&st), [7]);

    let fx = pr(&mut st, PullRequestMsg::Close);
    assert_eq!(jobs(&fx), [("drop", w.clone(), vec![7])]);
    let fx = pr(&mut st, PullRequestMsg::HeadFetched { id: 7, head: HEAD.into(), result: Ok(()) });
    assert_eq!(jobs(&fx), [("drop", w, vec![7])], "the fetch wrote it after the close");

    // A tab that no longer exists: its answer still registers the ref, and it goes.
    let gone = glowfly_core::paths::temp_dir().join("prh-gone-tab");
    let answer = Msg::PullRequest(PullRequestMsg::HeadFetched { id: 9, head: HEAD.into(), result: Ok(()) });
    let fx = send(&mut st, Msg::Scoped { workdir: gone.clone(), msg: Box::new(answer) });
    assert_eq!(jobs(&fx), [("drop", gone, vec![9])]);
    assert!(held(&st).is_empty());
}

#[test]
fn an_open_load_sweeps_all_but_held() {
    let mut st = state();
    let snap = crate::state::op_fixture::snapshot("prh-sweep");
    let w = snap.workdir.clone();
    st.loading = true;
    let msg = Msg::Repo(RepoMsg::RepoLoaded { generation: st.load_gen, kind: LoadKind::Open, result: Ok(Box::new(snap)) });
    assert_eq!(jobs(&send(&mut st, msg)), [("sweep", w, vec![])]);

    // A parked worktree tab of the same repository holds 7: it stays.
    let mut st = setup("prh-sweep-held");
    let common = st.snap.as_ref().unwrap().common.clone();
    opened_and_compared(&mut st, SourceFrom::Head);
    send(&mut st, Msg::Tabs(TabsMsg::NewTab));
    let mut other = crate::state::op_fixture::snapshot("prh-sweep-other");
    other.common = common;
    let w = other.workdir.clone();
    st.loading = true;
    let msg = Msg::Repo(RepoMsg::RepoLoaded { generation: st.load_gen, kind: LoadKind::Open, result: Ok(Box::new(other)) });
    assert_eq!(jobs(&send(&mut st, msg)), [("sweep", w, vec![7])]);
}

#[test]
fn head_ref_jobs_run_locally_without_a_scope_or_a_queue() {
    let w = glowfly_core::paths::temp_dir();
    let all = [
        (HeadRefJob::Keep { workdir: w.clone(), id: 7, oid: head_oid() }, "keep-pr-head"),
        (HeadRefJob::Drop { workdir: w.clone(), ids: vec![7] }, "drop-pr-heads"),
        (HeadRefJob::Sweep { workdir: w, keep: vec![] }, "sweep-pr-heads"),
    ];
    for (job, name) in all {
        let e = PrEffect::HeadRef(job);
        assert_eq!(e.name(), name);
        assert!(e.scope().is_none() && e.queue().is_none() && e.timed());
        assert_eq!(e.sandbox(), SandboxRule::Run);
    }
}
