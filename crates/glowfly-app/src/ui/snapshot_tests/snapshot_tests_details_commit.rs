//! The commit details panel, title first: the title, the author's row, the chips (SHA,
//! parents, committer) and what they send, the hover over the author, a long body that scrolls
//! in its block, and a commit with no message.

use super::common::short;
use super::snapshot_tests::{NOW, git, harness, remove_fixture, with_repo};
use super::snapshot_tests_line_counts::{lines, write};
use super::snapshot_tests_select::click;
use crate::msg::{Msg, RepoMsg};
use crate::state::{AppState, Selection};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{Event, PointerButton, Pos2, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use git2::Oid;
use glowfly_core::gitcli::{args, run_with_input};
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{self, LoadOptions};
use glowfly_core::settings::ThemeChoice;
use std::path::{Path, PathBuf};
use std::sync::Arc;

type H = Harness<'static, (AppState, Vec<Msg>)>;

pub(in crate::ui) const MESSAGE: &str =
    "Add the guide and search index\n\nWrites docs/guide.md and an index for search.\nThe index is rebuilt on each start.";
const GRACE: &str = "Grace Hopper <grace@example.com>";

/// `git commit` with Ada as the author on `authored` and Grace as the committer on `committed`,
/// as after someone else applied or rebased Ada's change.
fn commit_as_grace(dir: &Path, a: &[&str], authored: &str, committed: &str) {
    let env = [
        ("GIT_AUTHOR_NAME", "Ada Lovelace"),
        ("GIT_AUTHOR_EMAIL", "ada@example.com"),
        ("GIT_COMMITTER_NAME", "Grace Hopper"),
        ("GIT_COMMITTER_EMAIL", "grace@example.com"),
        ("GIT_AUTHOR_DATE", authored),
        ("GIT_COMMITTER_DATE", committed),
    ];
    let (ok, out) = run_with_input(dir, &args(a), "", &env);
    assert!(ok, "git {a:?} failed: {out}");
}

/// Reloads the history and selects `oid` with its detail and line counts loaded.
fn select(st: &mut AppState, dir: &Path, oid: Oid) {
    st.snap = Some(repo::load(dir, &LoadOptions::all(500)).unwrap());
    st.selection = Selection::Commit(oid);
    st.session.detail = Some(repo::commit_detail(dir, oid).unwrap());
    st.session.reads.line_counts.start_commit(oid);
    st.session.reads.line_counts.finish_commit(oid, Arc::new(repo::commit_line_counts(dir, oid).unwrap()));
}

fn head(dir: &Path) -> Oid {
    git2::Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

/// The fixture with one more commit, by Ada and committed by Grace, that edits, adds, deletes
/// and adds a binary file, selected. `message` is its message.
pub(in crate::ui) fn with_commit(theme: ThemeChoice, name: &str, message: &str) -> (AppState, PathBuf, Oid) {
    let (mut st, dir) = with_repo(theme, name);
    write(&dir, "app.txt", "app\nfixed\nrouting\nconfig\n");
    write(&dir, "docs/guide.md", &lines("Step", 24));
    write(&dir, "src/search/index.rs", &lines("fn part", 9));
    std::fs::remove_file(dir.join("login.txt")).unwrap();
    std::fs::write(dir.join("logo.png"), b"\x89PNG\r\n\x1a\n\0\0\0binary").unwrap();
    git(&dir, &["add", "-A"], "2026-01-12T10:00:00+00:00");
    commit_as_grace(
        &dir,
        &["commit", "-q", "--allow-empty-message", "-m", message],
        "2026-01-12T10:00:00+00:00",
        "2026-01-13T10:00:00+00:00",
    );
    let oid = head(&dir);
    select(&mut st, &dir, oid);
    (st, dir, oid)
}

/// The messages the last frames pushed.
fn pushed(h: &mut H) -> Vec<Msg> {
    std::mem::take(&mut h.state_mut().1)
}

fn center(h: &H, label: &str) -> Pos2 {
    let b = h.get_by_label(label).bounding_box().expect(label);
    pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32)
}

#[test]
fn details_commit_title_first() {
    for (theme, name, dark) in
        [(ThemeChoice::Dark, "details_commit_title_first", true), (ThemeChoice::Light, "details_commit_title_first_light", false)]
    {
        let (st, dir, _) = with_commit(theme, "details-title-first", MESSAGE);
        let mut h = harness(st, dark);
        h.run();
        assert_eq!(h.query_all_by_label("Add the guide and search index").count(), 2, "the graph row and the title");
        assert!(h.query_by_label(&Lang::En.trf("details.commit.committer.label", &[("committer", &"Grace Hopper")])).is_some());
        h.check_snapshot(name);
        drop(h);
        remove_fixture(&dir);
    }
}

#[test]
fn details_commit_merge() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "details-merge");
    let merge = st.snap.as_ref().unwrap().commits.iter().find(|c| c.summary == "Merge branch 'login-page'").unwrap().oid;
    select(&mut st, &dir, merge);
    let parents = st.session.detail.as_ref().unwrap().parents.clone();
    assert_eq!(parents.len(), 2);
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label(Lang::En.tr("details.commit.parents")).is_some());
    for p in &parents {
        assert!(h.query_by_label(&Lang::En.trf("details.commit.parent.name", &[("sha", &short(*p))])).is_some(), "{p}");
    }
    let committer = Lang::En.trf("details.commit.committer.label", &[("committer", &"Ada Lovelace")]);
    assert!(h.query_by_label(&committer).is_none(), "the author committed the merge");
    h.check_snapshot("details_commit_merge");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn details_commit_chips_send_their_messages() {
    let (st, dir, oid) = with_commit(ThemeChoice::Dark, "details-chips", MESSAGE);
    let parent = st.session.detail.as_ref().unwrap().parents[0];
    let mut h = harness(st, true);
    h.run();
    pushed(&mut h);

    h.get_by_label(&Lang::En.trf("details.commit.sha", &[("sha", &short(oid))])).click();
    h.run();
    assert!(pushed(&mut h).iter().any(|m| matches!(m, Msg::Copy(s) if *s == oid.to_string())), "the SHA chip copies the full SHA");

    let parent_name = Lang::En.trf("details.commit.parent.name", &[("sha", &short(parent))]);
    h.get_by_label(&parent_name).click();
    h.run();
    assert!(pushed(&mut h).iter().any(|m| matches!(m, Msg::Repo(RepoMsg::JumpTo(p)) if *p == parent)), "the parent chip selects it");

    let at = center(&h, &parent_name);
    click(&mut h, at, PointerButton::Secondary);
    pushed(&mut h);
    h.get_by_label(Lang::En.tr("common.copy_sha.button")).click();
    h.run();
    assert!(pushed(&mut h).iter().any(|m| matches!(m, Msg::Copy(s) if *s == parent.to_string())), "Copy SHA copies the parent's");

    h.get_by_label(&Lang::En.trf("details.commit.committer.label", &[("committer", &"Grace Hopper")])).click();
    h.run();
    assert!(pushed(&mut h).iter().any(|m| matches!(m, Msg::Copy(s) if s == GRACE)), "the committer chip copies the name and email");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn details_commit_person_tip() {
    let (st, dir, _) = with_commit(ThemeChoice::Dark, "details-person-tip", MESSAGE);
    let authored = st.session.detail.as_ref().unwrap().author_time;
    let verbose = st.settings.format_verbose(authored, 0);
    let when = format!(" · {}", glowfly_core::dates::relative(Lang::En, NOW - authored));
    let mut h = harness(st, true);
    h.run();
    let b = h.get_by_label(&when).bounding_box().expect("the relative date");
    h.input_mut().events.push(Event::PointerMoved(pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32)));
    for _ in 0..4 {
        h.step();
    }
    let tip = format!("Ada Lovelace <ada@example.com>\n{verbose}");
    assert!(h.query_by_label(&tip).is_some(), "the hover names the author and the whole date: {tip:?}");
    h.check_snapshot("details_commit_person_tip");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn details_commit_long_body() {
    let body: String = (1..=40).map(|i| format!("Line {i} of a long description that explains the change.\n")).collect();
    let (st, dir, _) = with_commit(ThemeChoice::Dark, "details-long-body", &format!("Rework the search index\n\n{body}"));
    let mut h = harness(st, true);
    h.run();
    let label = h.get_by_label_contains("Line 1 of a long").bounding_box().expect("the body");
    let files = h.get_by_label_contains("changed files").bounding_box().expect("the files header");
    assert!(label.y1 - label.y0 > 320.0, "the body is longer than any cap");
    // From the body's top (inside the block's margin) to the files header: the capped block,
    // its margin, the gap and the separator.
    let block = files.y0 - label.y0 - 16.0 - 8.0;
    assert!(block <= 320.0 + 12.0, "the block is capped: {block}");
    h.check_snapshot("details_commit_long_body");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn details_commit_no_message() {
    let (st, dir, _) = with_commit(ThemeChoice::Dark, "details-no-message", "");
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label(Lang::En.tr("details.commit.no_message.label")).is_some());
    drop(h);
    remove_fixture(&dir);
}
