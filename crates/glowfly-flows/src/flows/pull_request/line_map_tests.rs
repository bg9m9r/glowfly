//! Headless tests for placing the pull request panel's threads on the local version's lines:
//! the request once the files and the page are loaded, its answer, a failure, a head that
//! isn't here, a synced panel that asks nothing, and stale answers (a new key, a closed panel,
//! a parked tab).

use super::calls_tests::{detail, loaded, reply_msg};
use super::tests::{cleanup, github, open, pr, rev, setup};
use crate::effect::{Effect, ForgeReply, ForgeResult};
use crate::flows::{finish_keeping, git};
use crate::msg::{ForgeMsg, Msg, TabsMsg};
use crate::pull_request::{LineMapJob, PrEffect, PrReply, PullRequestMsg};
use crate::state::AppState;
use crate::state::pull_request::PrView;
use crate::state::pull_request_line_map::{LineMaps, current};
use crate::state::pull_request_threads::{Placement, go_to, marks, placement};
use crate::update::update;
use git2::Oid;
use glowfly_core::diff_rows::LineSide;
use glowfly_core::forge::{PrPage, ReviewThread, ThreadComment};
use glowfly_core::repo::PrLineMaps;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn thread(id: &str, path: &str, line: u32) -> ReviewThread {
    ReviewThread {
        id: id.into(),
        path: path.into(),
        side: LineSide::New,
        line: Some(line),
        start_line: None,
        original_line: None,
        outdated: false,
        resolved: false,
        resolved_by: None,
        can_reply: true,
        can_resolve: true,
        can_unresolve: false,
        context: String::new(),
        comments: vec![ThreadComment { id: Some(5), author: "ada".into(), time: 1, body: "Hm".into(), url: String::new(), pending: false }],
        more: 0,
    }
}

fn jobs(fx: &[Effect]) -> Vec<&LineMapJob> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::PullRequest(PrEffect::LineMaps(j)) => Some(j),
            _ => None,
        })
        .collect()
}

fn view(st: &AppState) -> &PrView {
    st.session.pull_request.view.as_ref().expect("the panel is open")
}

/// Commits f.txt as `text` on top of the local feature branch (not pushed) and returns it.
fn local_commit(work: &Path, text: &str) -> Oid {
    git(work, &["checkout", "-q", "feature"]);
    std::fs::write(work.join("f.txt"), text).unwrap();
    git(work, &["commit", "-q", "-am", "local"]);
    let oid = rev(work, "HEAD");
    git(work, &["checkout", "-q", "main"]);
    oid
}

/// The page with `head` and `threads`, loaded without running what it asks for.
fn page_loaded(st: &mut AppState, head: &str, threads: Vec<ReviewThread>) -> Vec<Effect> {
    let page = PrPage { detail: detail(head), entries: Vec::new(), threads, threads_note: None };
    let msg = reply_msg(st, PrReply::Loaded(Ok(page)));
    update(st, Msg::PullRequest(msg), Instant::now())
}

fn answer(st: &mut AppState, generation: u64, result: Result<PrLineMaps, String>) {
    update(st, Msg::PullRequest(PullRequestMsg::LinesMapped { id: 7, generation, result }), Instant::now());
}

/// `setup` with a local commit Y on feature that adds two lines above f.txt's one, the panel
/// open (its files from the listed head X), and the page naming Y with threads on f.txt lines
/// 3 and 1, not yet answered. Returns (state, effects, Y, X, merge base, dirs).
fn newer_here(name: &str) -> (AppState, Vec<Effect>, Oid, Oid, Oid, [PathBuf; 2]) {
    let (mut st, work, bare, base) = setup(name, &[]);
    let y = local_commit(&work, "new1\nnew2\nfeature\n");
    let x = rev(&work, "origin/feature");
    open(&mut st);
    let fx = page_loaded(&mut st, &y.to_string(), vec![thread("T3", "f.txt", 3), thread("T1", "f.txt", 1)]);
    (st, fx, y, x, base, [work, bare])
}

#[test]
fn maps_threads_when_the_page_head_is_newer_here() {
    let (mut st, fx, y, x, base, dirs) = newer_here("prl-map");
    let asked = jobs(&fx);
    let [job] = &asked[..] else { panic!("one line map job: {asked:?}") };
    assert_eq!((job.head.as_str(), job.local, job.base, job.id), (y.to_string().as_str(), x, base, 7));
    assert_eq!(job.paths, ["f.txt"]);
    assert!(matches!(view(&st).lines, LineMaps::Loading(_)));
    assert_eq!(placement(view(&st)), Placement::Elsewhere("Placing these comments on the local version…".into()));
    finish_keeping(&mut st, fx);
    let v = view(&st);
    assert!(matches!(placement(v), Placement::Mapped(_)), "{:?}", placement(v));
    let m = marks(v, "f.txt").unwrap();
    assert_eq!(m.keys().copied().collect::<Vec<_>>(), [(LineSide::New, 1)]);
    assert_eq!(m[&(LineSide::New, 1)].thread, "T3");
    let page = v.loaded().unwrap();
    assert_eq!(go_to(v, &page.threads[0]).unwrap().2, 1, "line 3 at the head is line 1 here");
    assert_eq!(go_to(v, &page.threads[1]).unwrap_err(), "The commented line was changed or removed in the local version");
    cleanup(&[&dirs[0], &dirs[1]]);
}

#[test]
fn synced_page_asks_for_no_maps() {
    let (mut st, work, bare, _) = setup("prl-synced", &[]);
    open(&mut st);
    let head = st.session.forge.as_ref().unwrap().prs[0].head.clone().unwrap();
    let fx = page_loaded(&mut st, &head, vec![thread("T1", "f.txt", 1)]);
    assert!(jobs(&fx).is_empty());
    assert_eq!(view(&st).lines, LineMaps::None);
    assert_eq!(placement(view(&st)), Placement::OnLines);
    loaded(&mut st, |_| {});
    assert_eq!(view(&st).lines, LineMaps::None);
    cleanup(&[&work, &bare]);
}

#[test]
fn head_not_here_keeps_the_fetch_hint() {
    let (mut st, work, bare, _) = setup("prl-missing", &[]);
    open(&mut st);
    let fx = page_loaded(&mut st, &"abc1234".repeat(6)[..40], vec![thread("T1", "f.txt", 1)]);
    assert_eq!(jobs(&fx).len(), 1);
    finish_keeping(&mut st, fx);
    let v = view(&st);
    assert!(matches!(&v.lines, LineMaps::Loaded { maps: PrLineMaps::HeadMissing, .. }), "{:?}", v.lines);
    let Placement::Elsewhere(text) = placement(v) else { panic!("not placed") };
    assert!(text.contains("fetch origin"), "{text}");
    assert_eq!(marks(v, "f.txt"), None);
    cleanup(&[&work, &bare]);
}

#[test]
fn a_stale_reply_is_dropped() {
    let (mut st, fx, y, _, _, dirs) = newer_here("prl-stale");
    let first = jobs(&fx)[0].generation;
    let fx = page_loaded(&mut st, &y.to_string(), vec![thread("T3", "f.txt", 3), thread("G", "g.txt", 1)]);
    let second = jobs(&fx);
    let [job] = &second[..] else { panic!("asked again: {second:?}") };
    assert!(job.generation > first);
    assert_eq!(job.paths, ["f.txt", "g.txt"]);
    answer(&mut st, first, Ok(PrLineMaps::HeadMissing));
    let LineMaps::Loading(key) = &view(&st).lines else { panic!("{:?}", view(&st).lines) };
    assert_eq!(key.paths, ["f.txt", "g.txt"], "still waiting for the new key");
    finish_keeping(&mut st, fx);
    assert!(matches!(view(&st).lines, LineMaps::Loaded { maps: PrLineMaps::Mapped { .. }, .. }));
    let last = st.session.pull_request.lines_gen;
    update(&mut st, Msg::PullRequest(PullRequestMsg::Close), Instant::now());
    assert!(st.session.pull_request.lines_gen > last, "closing drops what is in flight");
    answer(&mut st, last, Ok(PrLineMaps::HeadMissing));
    assert!(st.session.pull_request.view.is_none());
    cleanup(&[&dirs[0], &dirs[1]]);
}

#[test]
fn a_moved_tip_asks_again() {
    let (mut st, fx, y, _, _, dirs) = newer_here("prl-moved");
    finish_keeping(&mut st, fx);
    assert!(matches!(placement(view(&st)), Placement::Mapped(_)));
    let z = local_commit(&dirs[0], "new1\nnew2\nfeature\nmore\n");
    let reply = ForgeReply { target: github(), auth: None, results: vec![ForgeResult::Prs(Ok(vec![pr(7, Some(z))]))] };
    let fx = update(&mut st, Msg::Forge(ForgeMsg::ForgeReplied(reply)), Instant::now());
    assert_eq!(current(view(&st)), None, "the old maps aren't used while the files load");
    assert!(marks(view(&st), "f.txt").is_none());
    let files = fx.into_iter().find(|e| matches!(e, Effect::PullRequest(PrEffect::Files(_)))).expect("the files load again");
    let msg = crate::runner::testing::execute(files).expect("an answer");
    let fx = update(&mut st, msg, Instant::now());
    let asked = jobs(&fx);
    let [job] = &asked[..] else { panic!("asked again: {asked:?}") };
    assert_eq!((job.head.as_str(), job.local), (y.to_string().as_str(), z));
    assert!(matches!(current(view(&st)), Some(LineMaps::Loading(_))), "not the old maps");
    finish_keeping(&mut st, fx);
    // Z only adds a line below Y's: both threads keep their lines.
    let mut keys: Vec<_> = marks(view(&st), "f.txt").unwrap().keys().map(|(_, n)| *n).collect();
    keys.sort_unstable();
    assert_eq!(keys, [1, 3]);
    cleanup(&[&dirs[0], &dirs[1]]);
}

#[test]
fn a_parked_tab_asks_again() {
    let (mut st, fx, _, _, _, dirs) = newer_here("prl-parked");
    let first = jobs(&fx)[0].generation;
    update(&mut st, Msg::Tabs(TabsMsg::NewTab), Instant::now());
    assert!(st.session.pull_request.view.is_none(), "parked with its tab");
    let fx = update(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)), Instant::now());
    let again = jobs(&fx);
    let [job] = &again[..] else { panic!("asked again: {again:?}") };
    assert!(job.generation > first);
    assert_eq!(job.generation, st.session.pull_request.lines_gen);
    answer(&mut st, first, Ok(PrLineMaps::HeadMissing));
    assert!(matches!(view(&st).lines, LineMaps::Loading(_)), "the old generation's reply is ignored");
    finish_keeping(&mut st, fx);
    assert!(matches!(placement(view(&st)), Placement::Mapped(_)));
    cleanup(&[&dirs[0], &dirs[1]]);
}

#[test]
fn a_failed_map_is_noted_and_logged() {
    let (mut st, fx, _, _, _, dirs) = newer_here("prl-failed");
    let generation = jobs(&fx)[0].generation;
    answer(&mut st, generation, Err("boom".into()));
    let v = view(&st);
    assert!(matches!(&v.lines, LineMaps::Failed { error, .. } if error == "boom"));
    let Placement::Elsewhere(text) = placement(v) else { panic!("not placed") };
    assert!(text.ends_with("their lines couldn't be found in the local version."), "{text}");
    assert!(st.log.iter().any(|l| l.text == "Couldn't place #7's comments on the local lines: boom"), "logged");
    cleanup(&[&dirs[0], &dirs[1]]);
}
