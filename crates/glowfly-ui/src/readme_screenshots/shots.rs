//! The README's screenshots of the main window, a commit's split diff and the conflict resolver,
//! each in Graphite (`-dark`) and Porcelain (`-light`). The pull request's is in
//! `pull_request.rs`.

use super::demo::{self, Demo};
use super::{harness, shoot};
use crate::snapshot_fixtures::{NOW, scenes::fill_lists, settings};
use eframe::egui;
use glowfly_core::repo::{self, Choice, DiffTarget, LoadOptions};
use glowfly_core::settings::{DiffLayout, ThemeChoice};
use glowfly_core::{highlight, intraline};
use glowfly_model::state::conflict::ConflictView;
use glowfly_model::state::{AppState, Selection, Tab};
use std::sync::Arc;
use std::time::Instant;

/// The window most screenshots draw.
pub(super) const WINDOW: egui::Vec2 = egui::vec2(1400.0, 800.0);

/// The part of the window right of the sidebar and below the toolbar.
fn below_toolbar() -> Option<egui::Rect> {
    Some(egui::Rect::from_min_max(egui::pos2(260.0, 94.0), egui::pos2(WINDOW.x, WINDOW.y)))
}

fn theme(dark: bool) -> ThemeChoice {
    if dark { ThemeChoice::Graphite } else { ThemeChoice::Porcelain }
}

fn name(base: &str, dark: bool) -> String {
    format!("{base}-{}", if dark { "dark" } else { "light" })
}

/// `demo` loaded into a fresh state, with a second, unloaded tab and Tags and Stashes folded.
pub(super) fn state(d: &Demo, dark: bool) -> AppState {
    let mut st = AppState::new(settings(theme(dark)), Vec::new(), Instant::now());
    st.snap = Some(repo::load(&d.dir, &LoadOptions::all(500)).unwrap());
    st.session.selection = Selection::Wip;
    let other = d.dir.parent().unwrap().join("buoy-feed");
    st.tabs = vec![Tab::unloaded(d.dir.clone()), Tab::unloaded(other)];
    st.view.sections.tags = false;
    st.view.sections.stashes = false;
    st.cues.now = NOW;
    st
}

/// The commit whose summary is `summary`.
fn commit(st: &AppState, summary: &str) -> git2::Oid {
    st.snap.as_ref().unwrap().commits.iter().find(|c| c.summary == summary).unwrap_or_else(|| panic!("no commit {summary:?}")).oid
}

#[test]
fn readme_screenshots_overview() {
    for dark in [true, false] {
        let d = demo::build(&name("overview", dark));
        demo::leave_changes(&d);
        let mut st = state(&d, dark);
        st.session.draft.summary = "Show high and low water in local time".into();
        let snap = st.snap.as_ref().unwrap();
        assert_eq!((snap.staged.len(), snap.unstaged.len()), (1, 2), "one staged file, two unstaged");
        let mut h = harness(st, dark, WINDOW);
        h.run();
        shoot(&mut h, &name("overview", dark), None);
        drop(h);
        d.remove();
    }
}

#[test]
fn readme_screenshots_diff() {
    for dark in [true, false] {
        let d = demo::build(&name("diff", dark));
        let mut st = state(&d, dark);
        let oid = commit(&st, "Colour rising and falling tides");
        st.session.selection = Selection::Commit(oid);
        st.session.detail = Some(repo::commit_detail(&d.dir, oid).unwrap());
        st.session.reads.line_counts.start_commit(oid);
        st.session.reads.line_counts.finish_commit(oid, Arc::new(repo::commit_line_counts(&d.dir, oid).unwrap()));
        let target = DiffTarget::Commit(oid, "src/render.rs".into());
        let mut diff = repo::load_diff(&d.dir, &target).unwrap();
        assert_eq!(diff.hunks.len(), 2, "two hunks");
        diff.highlight = highlight::highlight_diff(target.path(), &diff, None);
        diff.intraline = intraline::intraline(&diff);
        st.session.diff.view = Some((target, diff.into()));
        st.settings.diff_layout = DiffLayout::Split;
        let mut h = harness(st, dark, WINDOW);
        h.run();
        shoot(&mut h, &name("diff", dark), below_toolbar());
        drop(h);
        d.remove();
    }
}

#[test]
fn readme_screenshots_conflicts() {
    for dark in [true, false] {
        let d = demo::build(&name("conflicts", dark));
        assert!(!d.try_run(&["merge", "moon-phase"]), "the merge stops on conflicts");
        let mut st = state(&d, dark);
        let file = "src/summary.rs";
        let mut view = ConflictView::loading(file.into());
        view.set_file(repo::load_conflict(&d.dir, file).unwrap());
        assert_eq!(view.chunk_count(), 2, "two conflicts");
        view.picks[0] = Choice::Theirs.into();
        st.session.conflict.view = Some(view);
        fill_lists(&mut st, &d.dir);
        let mut h = harness(st, dark, WINDOW);
        h.run();
        // The whole width: the merge banner starts under the sidebar.
        let below = egui::Rect::from_min_max(egui::pos2(0.0, 94.0), egui::pos2(WINDOW.x, WINDOW.y));
        shoot(&mut h, &name("conflicts", dark), Some(below));
        drop(h);
        d.remove();
    }
}
