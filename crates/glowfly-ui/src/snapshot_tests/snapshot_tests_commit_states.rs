//! The commit box's states (design pass D11): ready, amending, and the German "none staged"
//! caption wrapping whole in a 360 px panel; the caption as the button's description; and the
//! button menu's Stage all and commit, Commit and push and Amend last commit.

use super::snapshot_tests::{git, harness, reload, remove_fixture, with_repo};
use crate::msg::CommitMsg;
use crate::msg::Msg;
use crate::state::AppState;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::{By, Queryable};
use glowfly_core::repo::LoadOptions;
use glowfly_core::repo_identity::{IdentitySource, RepoIdentity};
use glowfly_core::settings::ThemeChoice;
use std::path::PathBuf;

/// The fixture with README.md and NOTES.md staged.
fn two_staged(theme: ThemeChoice, name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    std::fs::write(dir.join("NOTES.md"), "notes\n").unwrap();
    git(&dir, &["add", "README.md", "NOTES.md"], "2026-01-11T10:00:00+00:00");
    reload(&mut st, &dir, &LoadOptions::all(500));
    (st, dir)
}

#[test]
fn commit_box_ready() {
    let (mut st, dir) = two_staged(ThemeChoice::Dark, "commit-ready");
    st.session.draft.summary = "Add login form".into();
    // The identity reads on from the title.
    let who = RepoIdentity {
        name: "Ada Lovelace".into(),
        email: "ada@analytical.example".into(),
        source: IdentitySource::Elsewhere,
        ..RepoIdentity::default()
    };
    st.identities.insert(st.snap.as_ref().unwrap().git_dir.clone(), who);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("commit_box_ready");
    let title = h.get_by_label("Commit").bounding_box().unwrap();
    let line = h.get_by_label("as Ada Lovelace <ada@analytical.example>").bounding_box().unwrap();
    assert!(line.x0 > title.x1 && line.y0 < title.y1 && line.y1 > title.y0, "after the title, on its row: {title:?} {line:?}");
    let button = h.get(By::new().role(Role::Button).label("Commit 2 files"));
    assert!(!button.is_disabled());
    assert_eq!(button.description(), None, "no caption once it can commit");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn commit_box_amend() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "commit-amend");
    st.session.draft.amend = true;
    st.session.draft.summary = "Add login form".into();
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("commit_box_amend");
    assert!(!h.get(By::new().role(Role::Button).label("Amend last commit")).is_disabled(), "an amend needs nothing staged");
    drop(h);
    remove_fixture(&dir);
}

/// German, nothing staged, a 360 px panel: the longest caption wraps whole (staging first, though
/// a summary is written), and it is the button's description.
#[test]
fn commit_box_de() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "commit-de");
    st.settings.language = "de".into();
    st.settings.panel_sizes.details = Some(360.0);
    // With a summary, staging is still the first missing step.
    st.session.draft.summary = "Anmeldeformular".into();
    let why = crate::state::commit::commit_blocked(&st, false).expect("nothing staged");
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("commit_box_de");
    let r = h.get_by_label(&why).bounding_box().expect("the whole caption is shown");
    assert!(r.height() >= 2.0 * f64::from(crate::ui::widgets::reason::LINE_H), "wrapped, not cut: {r:?}");
    let button = h.get(By::new().role(Role::Button).label("Committen"));
    assert!(button.is_disabled());
    assert_eq!(button.description().as_deref(), Some(why.as_str()));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn commit_box_menu_actions() {
    let (mut st, dir) = two_staged(ThemeChoice::Dark, "commit-menu-actions");
    st.session.draft.summary = "Add login form".into();
    let mut h = harness(st, true);
    h.run();
    h.get_by_label("More commit options").click();
    h.run();
    assert!(h.query_by_label("Stage all and commit").is_some());
    h.get_by_label("Commit and push").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Commit(CommitMsg::CommitAndPush))));
    assert!(h.query_by_label("Commit and push").is_none(), "picking closes the menu");

    h.get_by_label("More commit options").click();
    h.run();
    let menu_amend = h.get_all_by_label("Amend last commit").count();
    assert_eq!(menu_amend, 2, "the option and the menu item");
    h.get_all_by_label("Amend last commit").find(|n| n.role() != Role::CheckBox).expect("the menu item").click();
    h.run();
    assert!(h.state().0.session.draft.amend, "the menu item ticks Amend");
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Commit(CommitMsg::PrefillAmend))));
    drop(h);
    remove_fixture(&dir);
}
