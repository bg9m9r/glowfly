//! Headless tests for moving in and between blames: Blame before this commit and Back, skipping
//! commits, the header toggles, the jump to file history, find, go to line and the run keys.

use super::HELD;
use crate::effect::{BlameRequest, Effect};
use crate::msg::{BlameMsg, Msg, RepoMsg};
use crate::state::AppState;
use crate::state::blame::{BACK_LIMIT, BlameData, BlameLanding, BlameView};
use crate::update::tests::{drive, git, scratch_repo, state};
use crate::update::update;
use git2::Oid;
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::repo::{Blame, BlameCommit, BlameOrigin, BlameRun};
use trellis_core::settings::BlameMoves;

fn oid(n: u8) -> Oid {
    Oid::from_bytes(&[n; 20]).unwrap()
}

/// A blame of `lines` with one commit per run (`oid(k + 1)` for run k), each run's lines from
/// a.txt with `previous`, and line i at line `i + 4` of the origin.
fn literal(lines: &[&str], runs: &[(usize, usize)], previous: Option<(Oid, &str)>) -> BlameData {
    let commit = |k: usize| BlameCommit {
        oid: oid(k as u8 + 1),
        author: "Ada".into(),
        email: "ada@example.com".into(),
        time: k as i64,
        summary: format!("commit {k}"),
        body: String::new(),
    };
    let blame = Blame {
        path: "a.txt".into(),
        at: oid(1),
        commits: (0..runs.len()).map(commit).collect(),
        lines: lines.iter().map(|l| (*l).to_owned()).collect(),
        runs: runs.iter().enumerate().map(|(k, &(start, len))| BlameRun { commit: k, origin: k, start, len }).collect(),
        origins: (0..runs.len())
            .map(|k| BlameOrigin { commit: k, path: "a.txt".into(), previous: previous.map(|(o, p)| (o, p.to_owned())) })
            .collect(),
        orig_lines: (0..lines.len()).map(|i| i + 4).collect(),
        language: None,
    };
    BlameData::new(blame, None)
}

fn ten_lines() -> Vec<&'static str> {
    vec!["l0", "l1", "l2", "l3", "l4", "l5", "l6", "l7", "l8", "l9"]
}

/// A repository with blame open on a.txt at HEAD, showing `data`.
fn shown(name: &str, data: BlameData) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    show(&mut st, data);
    (st, dir)
}

fn show(st: &mut AppState, data: BlameData) {
    let view = st.blame.get_or_insert_with(|| BlameView::loading("a.txt".into(), None));
    view.loading = false;
    view.data = Some(data);
}

fn send(st: &mut AppState, m: BlameMsg) -> Vec<Effect> {
    update(st, Msg::Blame(m), Instant::now())
}

fn view(st: &AppState) -> &BlameView {
    st.blame.as_ref().expect("blame open")
}

fn requests(fx: &[Effect]) -> Vec<&BlameRequest> {
    fx.iter().filter_map(|e| if let Effect::LoadBlame(r) = e { Some(r) } else { None }).collect()
}

fn loaded(st: &mut AppState, data: BlameData) {
    let (path, generation) = (view(st).path.clone(), st.blame_gen);
    send(st, BlameMsg::Loaded { path, generation, result: Ok(Box::new(data)) });
}

#[test]
fn blame_parent_pushes_back_and_requests_the_parent() {
    let (mut st, dir) = shown("blame-nav-parent", literal(&["x", "y"], &[(0, 2)], Some((oid(2), "old.txt"))));
    st.blame.as_mut().unwrap().skipped = vec![oid(7)];
    st.blame.as_mut().unwrap().top_line = 1;
    let fx = send(&mut st, BlameMsg::BlameParent { line: 0 });
    let v = view(&st);
    assert_eq!((v.path.as_str(), v.at, v.back.len()), ("old.txt", Some(oid(2)), 1));
    assert!(v.loading && v.data.is_none());
    let r = requests(&fx);
    assert!(matches!(&r[..], [r] if r.path == "old.txt" && r.at == Some(oid(2)) && r.opts.ignore_revs == [oid(7)]), "{fx:?}");

    loaded(&mut st, literal(&ten_lines(), &[(0, 10)], None));
    let v = view(&st);
    assert_eq!(v.cursor, Some(4), "where line 0 was in the parent");
    assert_eq!(v.scroll_to, Some(BlameLanding { line: 4, center: true }));

    let fx = send(&mut st, BlameMsg::Back);
    let v = view(&st);
    assert_eq!((v.path.as_str(), v.at, &v.skipped[..]), ("a.txt", None, &[oid(7)][..]));
    assert!(v.back.is_empty() && v.loading);
    assert_eq!(v.land, Some(BlameLanding { line: 1, center: false }), "the old top line");
    assert!(matches!(&requests(&fx)[..], [r] if r.path == "a.txt" && r.at.is_none()), "{fx:?}");
    assert!(send(&mut st, BlameMsg::Back).is_empty(), "nothing further back");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn blame_parent_without_previous_toasts() {
    let (mut st, dir) = shown("blame-nav-first", literal(&["x"], &[(0, 1)], None));
    let fx = send(&mut st, BlameMsg::BlameParent { line: 0 });
    assert!(fx.is_empty(), "{fx:?}");
    assert!(view(&st).back.is_empty() && view(&st).data.is_some());
    let toast = st.toasts.last().expect("a toast");
    assert_eq!(toast.text, format!("{} added these lines with the file; there is nothing before it", &oid(1).to_string()[..7]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn back_stack_is_capped() {
    let (mut st, dir) = shown("blame-nav-cap", literal(&["x"], &[(0, 1)], Some((oid(2), "a.txt"))));
    for n in 0..=BACK_LIMIT {
        st.blame.as_mut().unwrap().top_line = n;
        show(&mut st, literal(&["x"], &[(0, 1)], Some((oid(2), "a.txt"))));
        send(&mut st, BlameMsg::BlameParent { line: 0 });
    }
    let v = view(&st);
    assert_eq!(v.back.len(), BACK_LIMIT);
    assert_eq!(v.back[0].top_line, 1, "the oldest step was dropped");
    assert_eq!(v.back[BACK_LIMIT - 1].top_line, BACK_LIMIT);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn skip_and_clear() {
    let (mut st, dir) = shown("blame-nav-skip", literal(&["x"], &[(0, 1)], None));
    let fx = send(&mut st, BlameMsg::SkipCommit(oid(3)));
    assert!(matches!(&requests(&fx)[..], [r] if r.opts.ignore_revs == [oid(3)]), "{fx:?}");
    show(&mut st, literal(&["x"], &[(0, 1)], None));
    assert!(send(&mut st, BlameMsg::SkipCommit(oid(3))).is_empty(), "already skipped");
    assert!(send(&mut st, BlameMsg::SkipCommit(Oid::zero())).is_empty(), "not a commit");
    let fx = send(&mut st, BlameMsg::ClearSkipped);
    assert!(matches!(&requests(&fx)[..], [r] if r.opts.ignore_revs.is_empty()), "{fx:?}");
    assert!(send(&mut st, BlameMsg::ClearSkipped).is_empty(), "nothing to clear");

    let v = st.blame.as_mut().unwrap();
    v.skipped = vec![oid(3)];
    v.back.push(crate::state::blame::BlameStep { path: "a.txt".into(), at: None, skipped: Vec::new(), top_line: 0, cursor: None });
    let fx = send(&mut st, BlameMsg::Open { path: "a.txt".into(), at: None });
    assert!(view(&st).skipped.is_empty() && view(&st).back.is_empty());
    assert!(matches!(&requests(&fx)[..], [r] if r.opts.ignore_revs.is_empty()), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn toggles_save_and_reblame() {
    let (mut st, dir) = shown("blame-nav-toggles", literal(&ten_lines(), &[(0, 10)], None));
    st.blame.as_mut().unwrap().top_line = 7;
    let fx = send(&mut st, BlameMsg::SetIgnoreWhitespace(true));
    assert!(st.settings.blame.ignore_whitespace);
    assert!(matches!(fx.first(), Some(Effect::SaveSettings(s)) if s.blame.ignore_whitespace), "{fx:?}");
    assert!(matches!(&requests(&fx)[..], [r] if r.opts.ignore_whitespace), "{fx:?}");
    assert_eq!(view(&st).land, Some(BlameLanding { line: 7, center: false }));
    assert!(send(&mut st, BlameMsg::SetIgnoreWhitespace(true)).is_empty(), "unchanged");

    let fx = send(&mut st, BlameMsg::SetMoves(BlameMoves::AcrossFiles));
    assert_eq!(st.settings.blame.moves, BlameMoves::AcrossFiles);
    assert!(matches!(&requests(&fx)[..], [r] if r.opts.moves == BlameMoves::AcrossFiles && r.opts.ignore_whitespace), "{fx:?}");

    st.blame = None;
    let fx = send(&mut st, BlameMsg::SetMoves(BlameMoves::Off));
    assert!(matches!(&fx[..], [Effect::SaveSettings(_)]), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn set_wrap_lines_saves_and_keeps_the_top_line() {
    let (mut st, dir) = shown("blame-nav-wrap", literal(&ten_lines(), &[(0, 10)], None));
    st.blame.as_mut().unwrap().top_line = 6;
    let fx = send(&mut st, BlameMsg::SetWrapLines(true));
    assert!(st.settings.blame.wrap_lines);
    assert!(matches!(&fx[..], [Effect::SaveSettings(s)] if s.blame.wrap_lines), "no reload: {fx:?}");
    let v = view(&st);
    assert_eq!(v.scroll_to, Some(BlameLanding { line: 6, center: false }));
    assert!(!v.loading && v.data.is_some());
    assert!(send(&mut st, BlameMsg::SetWrapLines(true)).is_empty(), "unchanged");
    let _ = std::fs::remove_dir_all(&dir);
}

fn commit_file(dir: &Path, text: &str, message: &str) -> Oid {
    std::fs::write(dir.join("a.txt"), text).unwrap();
    git(dir, &["commit", "-q", "-am", message]);
    git2::Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

#[test]
fn open_history_from_blame_selects_the_commit() {
    let dir = scratch_repo("blame-nav-history");
    let c1 = git2::Repository::open(&dir).unwrap().head().unwrap().target().unwrap();
    let c2 = commit_file(&dir, "one\ntwo\n", "second");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Blame(BlameMsg::Open { path: "a.txt".into(), at: None }));
    drive(&mut st, Msg::Blame(BlameMsg::OpenHistory { select: Some(c1) }));
    assert!(st.blame.is_none());
    let h = st.history.as_ref().expect("history open");
    assert_eq!(h.entries.len(), 2);
    assert_eq!(h.selected_entry().map(|e| e.oid), Some(c1));
    assert!(h.select.is_none(), "used by the first batch");

    // No commit asked for: the blamed one (HEAD resolved).
    drive(&mut st, Msg::Blame(BlameMsg::Open { path: "a.txt".into(), at: None }));
    drive(&mut st, Msg::Blame(BlameMsg::OpenHistory { select: None }));
    assert_eq!(st.history.as_ref().unwrap().selected_entry().map(|e| e.oid), Some(c2));
    // A commit the history doesn't list: the first entry.
    drive(&mut st, Msg::Blame(BlameMsg::Open { path: "a.txt".into(), at: None }));
    drive(&mut st, Msg::Blame(BlameMsg::OpenHistory { select: Some(oid(9)) }));
    assert_eq!(st.history.as_ref().unwrap().selected, Some(0));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn find_steps_through_matches() {
    let (mut st, dir) = shown("blame-nav-find", literal(&["foo", "bar", "Foo bar", "baz"], &[(0, 4)], None));
    st.blame.as_mut().unwrap().find.query = "foo".into();
    send(&mut st, BlameMsg::FindChanged);
    let v = view(&st);
    assert_eq!((&v.find.hits[..], v.find.current, v.cursor), (&[0, 2][..], Some(0), Some(0)));
    assert_eq!(v.scroll_to, Some(BlameLanding { line: 0, center: true }));
    let cursor = |st: &AppState| view(st).cursor;
    send(&mut st, BlameMsg::FindStep(1));
    assert_eq!(cursor(&st), Some(2));
    send(&mut st, BlameMsg::FindStep(1));
    assert_eq!(cursor(&st), Some(0), "wraps round");
    send(&mut st, BlameMsg::FindStep(-1));
    assert_eq!(cursor(&st), Some(2));

    st.blame.as_mut().unwrap().find.query = "zzz".into();
    send(&mut st, BlameMsg::FindChanged);
    let v = view(&st);
    assert!(v.find.hits.is_empty() && v.find.current.is_none());
    assert_eq!(v.cursor, Some(2), "no match leaves the cursor");
    st.blame.as_mut().unwrap().find.query.clear();
    send(&mut st, BlameMsg::FindChanged);
    assert!(view(&st).find.hits.is_empty() && view(&st).find.current.is_none());

    // From the top line on.
    st.blame.as_mut().unwrap().find.query = "BA".into();
    st.blame.as_mut().unwrap().top_line = 2;
    send(&mut st, BlameMsg::FindChanged);
    assert_eq!((&view(&st).find.hits[..], view(&st).find.current), (&[1, 2, 3][..], Some(1)));

    // A new blame of the file is searched again.
    st.blame.as_mut().unwrap().loading = true;
    loaded(&mut st, literal(&["bar", "x"], &[(0, 2)], None));
    assert_eq!((&view(&st).find.hits[..], view(&st).find.current), (&[0][..], None));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn go_to_line_clamps() {
    let (mut st, dir) = shown("blame-nav-goto", literal(&["a", "b", "c", "d", "e"], &[(0, 5)], None));
    let go = |st: &mut AppState, text: &str| {
        st.blame.as_mut().unwrap().goto = text.into();
        send(st, BlameMsg::GoToLine);
        view(st).cursor
    };
    assert_eq!(go(&mut st, "3"), Some(2));
    assert_eq!(view(&st).scroll_to, Some(BlameLanding { line: 2, center: true }));
    assert_eq!(go(&mut st, " 999 "), Some(4));
    assert_eq!(go(&mut st, "0"), Some(0));
    assert_eq!(go(&mut st, "abc"), Some(0), "not a number: nothing changes");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn step_run_moves_between_runs() {
    let (mut st, dir) = shown("blame-nav-runs", literal(&ten_lines()[..8], &[(0, 2), (2, 3), (5, 3)], None));
    let step = |st: &mut AppState, d: isize| {
        send(st, BlameMsg::StepRun(d));
        view(st).cursor
    };
    assert_eq!(step(&mut st, 1), Some(2), "from the top line");
    assert_eq!(step(&mut st, 1), Some(5));
    assert_eq!(step(&mut st, 1), Some(5), "stops at the last run");
    st.blame.as_mut().unwrap().cursor = Some(6);
    assert_eq!(step(&mut st, -1), Some(5), "to the start of its own run first");
    assert_eq!(step(&mut st, -1), Some(2));
    assert_eq!(step(&mut st, -1), Some(0));
    assert_eq!(step(&mut st, -1), Some(0), "stops at the first run");
    assert_eq!(view(&st).scroll_to, Some(BlameLanding { line: 0, center: true }));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reblame_in_a_held_repository_shows_held() {
    let (mut st, dir) = crate::update::test_mods::trust_held_tests::held_repo("blame-nav-held");
    show(&mut st, literal(&["x"], &[(0, 1)], None));
    let fx = send(&mut st, BlameMsg::SkipCommit(oid(3)));
    assert!(requests(&fx).is_empty(), "{fx:?}");
    let v = view(&st);
    assert_eq!(v.error.as_deref(), Some(HELD));
    assert!(!v.loading && v.data.is_none());
    assert_eq!(v.skipped, [oid(3)]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn blame_parent_end_to_end() {
    let dir = scratch_repo("blame-nav-e2e");
    let c1 = commit_file(&dir, "1\n2\n3\n", "three lines");
    let c2 = commit_file(&dir, "1\ntwo\n3\n", "edit two");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Blame(BlameMsg::Open { path: "a.txt".into(), at: None }));
    assert_eq!(view(&st).data.as_ref().map(|d| d.blame.at), Some(c2));
    drive(&mut st, Msg::Blame(BlameMsg::BlameParent { line: 1 }));
    let v = view(&st);
    assert!(v.error.is_none(), "{:?}", v.error);
    assert_eq!(v.at, Some(c1));
    assert_eq!(v.data.as_ref().map(|d| d.blame.at), Some(c1));
    assert_eq!(v.cursor, Some(1));
    drive(&mut st, Msg::Blame(BlameMsg::Back));
    let v = view(&st);
    assert_eq!(v.at, None);
    assert_eq!(v.data.as_ref().map(|d| d.blame.at), Some(c2));
    let _ = std::fs::remove_dir_all(&dir);
}
