//! Range-diff snapshots: the pairs with a changed pair's interdiff, the
//! stacked layout of a narrow window, an unchanged pair's pane, and the loading, failed and
//! empty views; and what its rows, Close and Esc send. The pairs are built by hand on the
//! fixture's commits, with the interdiff read from a captured sample, so the image never depends
//! on the fixture's reflog.

use super::snapshot_tests::{NOW, harness, harness_sized, remove_fixture, with_repo};
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::journal::JournalView;
use crate::state::range_diff::{Origin, Phase, RangeDiffView, widest};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{self, Key};
use egui_kittest::kittest::Queryable;
use git2::Oid;
use glowfly_core::range_diff::{Compared, Ends, Outcome, Pair, Range, Rewrite, Side, Status};
use glowfly_core::settings::ThemeChoice;
use glowfly_model::range_diff::RangeDiffMsg;
use std::path::PathBuf;

/// A changed pair's interdiff as git prints it (blank lines are five spaces).
const SAMPLE: &str = "\
1:  6f32963 ! 1:  71ef454 Add login page
    @@ Metadata
     Author: Test <test@example.com>
\x20\x20\x20\x20\x20
      ## Commit message ##
    -    Add login page
    +    Add login page and remember the user
\x20\x20\x20\x20\x20
      ## login.txt (new) ##
     @@
    -+login
    ++login form
    ++remember me
";

fn commit(st: &AppState, summary: &str) -> Oid {
    st.snap.as_ref().unwrap().commits.iter().find(|c| c.summary == summary).unwrap_or_else(|| panic!("{summary}")).oid
}

fn side(index: u32, oid: Oid, summary: &str) -> Option<Side> {
    Some(Side { index, oid, summary: summary.into() })
}

/// login-page compared with its version before the last rebase: one changed commit, one
/// dropped, one new, one the same.
fn compared(st: &AppState) -> Compared {
    let (login, search, fix, skeleton, merge) = (
        commit(st, "Add login page"),
        commit(st, "Search results page"),
        commit(st, "Fix build on Windows"),
        commit(st, "Add app skeleton"),
        commit(st, "Merge branch 'login-page'"),
    );
    let interdiff = glowfly_core::range_diff::parse::pairs(SAMPLE).unwrap().remove(0).interdiff;
    let pairs = vec![
        Pair {
            status: Status::Changed,
            old: side(1, login, "Add login page"),
            new: side(1, search, "Add login page and remember the user"),
            interdiff,
        },
        Pair { status: Status::Removed, old: side(2, fix, "Fix build on Windows"), new: None, interdiff: None },
        Pair { status: Status::Added, old: None, new: side(2, skeleton, "Add app skeleton"), interdiff: None },
        Pair {
            status: Status::Unchanged,
            old: side(3, merge, "Tidy the login form"),
            new: side(3, login, "Tidy the login form"),
            interdiff: None,
        },
    ];
    let rewrite = Rewrite { before: login, at: NOW - 2 * 86_400, message: format!("rebase (finish): refs/heads/login-page onto {merge}") };
    Compared {
        old: Range { base: skeleton, tip: login },
        new: Range { base: merge, tip: search },
        mainline: Some("main".into()),
        rewrite: Some(rewrite),
        pairs,
    }
}

fn origin() -> Origin {
    Origin { ends: Ends::LastRewrite { refname: "refs/heads/login-page".into() }, label: Some("login-page".into()), at: None }
}

fn with_phase(theme: ThemeChoice, name: &str, phase: impl FnOnce(&AppState) -> Phase, selected: Option<usize>) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    let phase = phase(&st);
    let mut view = RangeDiffView::loading(origin(), 1);
    view.phase = phase;
    view.selected = selected;
    view.widest = view.compared().map(|c| widest(c, 4)).unwrap_or_default();
    st.session.range_diff.view = Some(view);
    (st, dir)
}

fn ready(theme: ThemeChoice, name: &str, selected: Option<usize>) -> (AppState, PathBuf) {
    with_phase(theme, name, |st| Phase::Ready(Box::new(Outcome::Compared(compared(st)))), selected)
}

fn sent(h: &egui_kittest::Harness<'_, (AppState, Vec<Msg>)>, want: fn(&RangeDiffMsg) -> bool) -> bool {
    h.state().1.iter().any(|m| matches!(m, Msg::RangeDiff(r) if want(r)))
}

#[test]
fn range_diff_changed() {
    let (st, dir) = ready(ThemeChoice::Dark, "range-diff-changed", Some(0));
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("range_diff_changed");
    h.get_by_label("Dropped: Fix build on Windows").click();
    h.run();
    assert!(sent(&h, |m| matches!(m, RangeDiffMsg::Select(1))), "a row selects its pair");
    h.get_by_label("Close").click();
    h.run();
    assert!(sent(&h, |m| matches!(m, RangeDiffMsg::Close)));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn esc_closes_it_even_over_the_journal() {
    for journal in [false, true] {
        let (mut st, dir) = ready(ThemeChoice::Dark, &format!("range-diff-esc-{journal}"), Some(0));
        if journal {
            st.session.journal.view = Some(JournalView::default());
        }
        let mut h = super::snapshot_tests_esc::app_harness(st);
        h.run();
        h.press_key(Key::Escape);
        h.run();
        assert!(sent(&h, |m| matches!(m, RangeDiffMsg::Close)));
        assert!(!h.state().1.iter().any(|m| matches!(m, Msg::Journal(_))), "the journal stays open underneath");
        drop(h);
        remove_fixture(&dir);
    }
}

#[test]
fn range_diff_narrow() {
    let (st, dir) = ready(ThemeChoice::Dark, "range-diff-narrow", Some(0));
    let mut h = harness_sized(st, true, egui::vec2(640.0, 820.0));
    h.run();
    h.check_snapshot("range_diff_narrow");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn range_diff_same_selected() {
    let (st, dir) = ready(ThemeChoice::Dark, "range-diff-same", Some(3));
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("range_diff_same_selected");
    assert!(h.query_by_label("Both versions of this commit make the same change.").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn range_diff_loading() {
    let (st, dir) = with_phase(ThemeChoice::Dark, "range-diff-loading", |_| Phase::Loading, None);
    let mut h = harness(st, true);
    // The spinner keeps repainting.
    h.run_steps(4);
    h.check_snapshot("range_diff_loading");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn range_diff_failed() {
    let no = |_: &AppState| Phase::Ready(Box::new(Outcome::NoRewrite { refname: "refs/heads/login-page".into() }));
    let (st, dir) = with_phase(ThemeChoice::Dark, "range-diff-failed", no, None);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("range_diff_failed");
    h.get_by_label("Try again").click();
    h.run();
    assert!(sent(&h, |m| matches!(m, RangeDiffMsg::Retry)));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn range_diff_empty() {
    let empty = |st: &AppState| Phase::Ready(Box::new(Outcome::Compared(Compared { pairs: Vec::new(), ..compared(st) })));
    let (st, dir) = with_phase(ThemeChoice::Dark, "range-diff-empty", empty, None);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("range_diff_empty");
    drop(h);
    remove_fixture(&dir);
}
