//! The WORKTREES section (rows for the main, linked, detached, locked and missing worktrees; the
//! filter; the row menu with its disabled items) and the Create worktree dialog (a branch base,
//! a checked-out branch, a commit base with the inside-a-worktree warning, a folder error from
//! the disk check), plus the branch and commit menu items that open it. The worktrees are
//! injected into the shared fixture's snapshot with fixed paths so the images are deterministic.

use super::snapshot_tests::{harness, remove_fixture, render, with_repo};
use super::snapshot_tests_select::{click, row_y};
use crate::msg::Msg;
use crate::msg::PrefsMsg;
use crate::state::worktrees::WorktreeForm;
use crate::state::{AppState, Modal};
use crate::ui::snapshot_check::CheckSnapshot;
use crate::worktree::WorktreeMsg;
use eframe::egui::accesskit::Role;
use eframe::egui::{Event, Modifiers, PointerButton, Pos2, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Node, Queryable};
use git2::Oid;
use std::path::{Path, PathBuf};
use trellis_core::repo::{ChangeCounts, ChangesRead, WorktreeInfo};
use trellis_core::settings::ThemeChoice;
use trellis_core::worktree_ops::{FolderProblem, WorktreeBase};

pub(super) type H = Harness<'static, (AppState, Vec<Msg>)>;

/// `s` with the platform's separators, as libgit2's paths are after loading. The fixed paths
/// are written as Windows ones (the approved images show them); elsewhere the drive is dropped
/// so they stay absolute (`/work/app`), as the dialog requires of a folder.
pub(super) fn p(s: &str) -> PathBuf {
    let s = if cfg!(windows) { s } else { s.strip_prefix("C:").unwrap_or(s) };
    Path::new(s).components().collect()
}

fn entry(path: &str, name: Option<&str>, branch: Option<&str>, head: Option<Oid>) -> WorktreeInfo {
    WorktreeInfo {
        name: name.map(str::to_owned),
        path: p(path),
        branch: branch.map(str::to_owned),
        head,
        current: false,
        locked: None,
        missing: false,
        prunable: false,
        folder: None,
        changes: ChangeCounts::default(),
        read: ChangesRead::NotCounted,
    }
}

/// The shared fixture with a main worktree at C:/work/app and four linked ones.
pub(super) fn with_worktrees(theme: ThemeChoice, name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    let snap = st.snap.as_mut().unwrap();
    let head = snap.head_oid;
    let other = snap.commits.get(2).map(|c| c.oid);
    let mut main = entry("C:/work/app", None, snap.head_branch.as_deref(), head);
    main.current = true;
    let login = entry("C:/work/app-worktrees/feature-login", Some("feature-login"), Some("feature/login"), other);
    let hotfix = entry("C:/work/app-worktrees/hotfix", Some("hotfix"), None, other);
    let mut old = entry("C:/work/app-worktrees/old", Some("old"), Some("old-ui"), head);
    old.locked = Some("on usb".into());
    let mut gone = entry("C:/work/app-worktrees/gone", Some("gone"), Some("spike"), head);
    gone.missing = true;
    gone.prunable = true;
    snap.worktrees = vec![main, login, hotfix, old, gone];
    (st, dir)
}

pub(super) fn center(h: &H, label: &str) -> Pos2 {
    let rect = h.get_by_label(label).bounding_box().expect("the row");
    pos2(((rect.x0 + rect.x1) / 2.0) as f32, ((rect.y0 + rect.y1) / 2.0) as f32)
}

/// The open dialog's button `label` where a left-panel header's + has the same name ("Add
/// remote"): the dialog draws after the panel, so its button comes last.
pub(super) fn dialog_button<'h>(h: &'h H, label: &'h str) -> Node<'h> {
    h.query_all_by_role_and_label(Role::Button, label).last().expect("the dialog's button")
}

pub(super) fn pushed(h: &H, want: &dyn Fn(&Msg) -> bool) -> bool {
    h.state().1.iter().any(want)
}

fn opens(h: &H, path: &str) -> bool {
    pushed(h, &|m| matches!(m, Msg::Worktree(WorktreeMsg::Open(x)) if *x == p(path)))
}

#[test]
fn worktrees_section() {
    let (st, dir) = with_worktrees(ThemeChoice::Dark, "worktrees");
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("Worktrees").is_some() || h.query_by_label("feature-login").is_some());
    assert!(h.query_by_label("✔  app").is_some(), "the current worktree is marked");
    h.check_snapshot("worktrees_section");
    drop(h);
    let (st, _) = with_worktrees(ThemeChoice::Light, "worktrees");
    render("worktrees_section_light", st, false);

    // Only the main worktree: no section.
    let (mut st, _) = with_worktrees(ThemeChoice::Dark, "worktrees");
    st.snap.as_mut().unwrap().worktrees.truncate(1);
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("✔  app").is_none() && h.query_by_label("feature-login").is_none());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn worktrees_counting() {
    // feature-login's changes are still being counted; hotfix was too large to count.
    let (mut st, dir) = with_worktrees(ThemeChoice::Dark, "worktrees-counting");
    let list = &mut st.snap.as_mut().unwrap().worktrees;
    list[1].read = ChangesRead::Pending;
    list[2].read = ChangesRead::TooLarge;
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("feature-login").is_some());
    h.check_snapshot("worktrees_counting");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn worktrees_filtered() {
    let (mut st, dir) = with_worktrees(ThemeChoice::Dark, "worktrees-filtered");
    st.view.ref_filter = "hotfix".into();
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("hotfix").is_some());
    for other in ["✔  app", "feature-login", "old", "gone"] {
        assert!(h.query_by_label(other).is_none(), "{other}");
    }
    h.check_snapshot("worktrees_filtered");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn worktree_menu() {
    let (st, dir) = with_worktrees(ThemeChoice::Dark, "worktree-menu");
    let mut h = harness(st, true);
    h.run();
    let at = center(&h, "feature-login");
    click(&mut h, at, PointerButton::Secondary);
    h.check_snapshot("worktree_menu");
    h.get_by_label("Open in new tab").click();
    h.run();
    assert!(opens(&h, "C:/work/app-worktrees/feature-login"));

    click(&mut h, at, PointerButton::Secondary);
    h.get_by_label("Copy path").click();
    h.run();
    let want = p("C:/work/app-worktrees/feature-login").display().to_string();
    assert!(pushed(&h, &|m| matches!(m, Msg::Copy(x) if *x == want)));

    let at = center(&h, "✔  app");
    click(&mut h, at, PointerButton::Secondary);
    assert!(h.get_by_label("Open in new tab").is_disabled());
    assert!(!h.get_by_label("Open folder").is_disabled());
    h.press_key(eframe::egui::Key::Escape);
    h.run();

    let at = center(&h, "gone");
    click(&mut h, at, PointerButton::Secondary);
    assert!(h.get_by_label("Open in new tab").is_disabled());
    assert!(h.get_by_label("Open folder").is_disabled());
    h.press_key(eframe::egui::Key::Escape);
    h.run();
    drop(h);

    // A double-click opens it.
    let (st, _) = with_worktrees(ThemeChoice::Dark, "worktree-menu");
    let mut h = harness(st, true);
    h.run();
    let at = center(&h, "feature-login");
    // Both clicks in one frame: each harness step is a quarter second, past the double-click delay.
    let events = &mut h.input_mut().events;
    events.push(Event::PointerMoved(at));
    for pressed in [true, false, true, false] {
        events.push(Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
    }
    h.run();
    assert!(opens(&h, "C:/work/app-worktrees/feature-login"));
    drop(h);
    remove_fixture(&dir);
}

/// The fixture with its worktrees and the dialog open for `base`.
fn dialog(theme: ThemeChoice, name: &str, base: impl FnOnce(&AppState) -> WorktreeBase) -> (AppState, PathBuf) {
    let (mut st, dir) = with_worktrees(theme, name);
    let base = base(&st);
    st.modal = Some(Modal::NewWorktree(WorktreeForm::new(st.snap.as_ref().unwrap(), base)));
    (st, dir)
}

fn form(h: &mut H) -> &mut WorktreeForm {
    match h.state_mut().0.modal.as_mut() {
        Some(Modal::NewWorktree(f)) => f,
        _ => panic!("the dialog is closed"),
    }
}

#[test]
fn worktree_dialog_branch() {
    let (st, dir) = dialog(ThemeChoice::Dark, "worktree-dialog-branch", |_| WorktreeBase::Branch("search".into()));
    let mut h = harness(st, true);
    h.run();
    assert_eq!(PathBuf::from(&form(&mut h).folder), p("C:/work/app-worktrees/search"));
    h.check_snapshot("worktree_dialog_branch");
    assert!(!h.get_by_label("Create worktree").is_disabled());

    // Typing a new branch name moves the folder along, until the folder is edited.
    h.input_mut().events.push(Event::Text("feature/x".into()));
    h.run();
    assert_eq!(PathBuf::from(&form(&mut h).folder), p("C:/work/app-worktrees/feature-x"));
    h.get_by_label("Create worktree").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Worktree(WorktreeMsg::Submit))));
    h.get_by_label("Browse…").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Prefs(PrefsMsg::PickFolder(crate::msg::FolderPurpose::WorktreeParent)))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn worktree_dialog_checked_out() {
    let (st, dir) = dialog(ThemeChoice::Dark, "worktree-dialog-main", |st| {
        WorktreeBase::Branch(st.snap.as_ref().unwrap().head_branch.clone().unwrap())
    });
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("worktree_dialog_checked_out");
    assert!(h.query_by_label_contains("is checked out in this worktree").is_some());
    assert!(h.get_by_label("Create worktree").is_disabled());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn worktree_dialog_commit_light() {
    let (st, dir) =
        dialog(ThemeChoice::Light, "worktree-dialog-commit", |st| WorktreeBase::Commit(st.snap.as_ref().unwrap().head_oid.unwrap()));
    let mut h = harness(st, false);
    h.run();
    let f = form(&mut h);
    f.branch = "topic".into();
    f.folder = p("C:/work/app/wt").display().to_string();
    f.folder_edited = true;
    h.run();
    h.check_snapshot("worktree_dialog_commit_light");
    assert!(h.query_by_label_contains("Create worktree at").is_some());
    assert!(h.query_by_label_contains("This folder is inside the worktree at").is_some());
    assert!(!h.get_by_label("Create worktree").is_disabled(), "a warning doesn't block");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn worktree_dialog_folder_error() {
    let (st, dir) = dialog(ThemeChoice::Dark, "worktree-dialog-error", |_| WorktreeBase::Branch("search".into()));
    let mut h = harness(st, true);
    let f = form(&mut h);
    let message = FolderProblem::NotEmpty.message(&f.folder);
    f.error = Some((f.folder.clone(), message.clone()));
    h.run();
    h.check_snapshot("worktree_dialog_folder_error");
    assert!(h.query_by_label(&message).is_some());
    assert!(h.get_by_label("Create worktree").is_disabled());

    // Typing in the folder field hides it.
    let folder = form(&mut h).folder.clone();
    h.get(By::new().role(Role::TextInput).value(&folder)).type_text("2");
    h.run();
    assert!(form(&mut h).folder_edited);
    assert!(h.query_by_label(&message).is_none());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn menus_open_the_dialog() {
    let (st, dir) = with_repo(ThemeChoice::Dark, "worktree-menus");
    let row = st.snap.as_ref().unwrap().row_of[&st.snap.as_ref().unwrap().head_oid.unwrap()] + 1;
    let head = st.snap.as_ref().unwrap().head_oid.unwrap();
    let mut h = harness(st, true);
    h.run();
    let rect = h.get_all_by_label("search").filter_map(|n| n.bounding_box()).min_by(|a, b| a.y0.total_cmp(&b.y0)).unwrap();
    click(&mut h, pos2(((rect.x0 + rect.x1) / 2.0) as f32, ((rect.y0 + rect.y1) / 2.0) as f32), PointerButton::Secondary);
    h.get_by_label("Create worktree from this branch…").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Worktree(WorktreeMsg::CreateDialog(WorktreeBase::Branch(b))) if b == "search")));

    click(&mut h, pos2(700.0, row_y(row)), PointerButton::Secondary);
    h.get_by_label("Create worktree from this commit…").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Worktree(WorktreeMsg::CreateDialog(WorktreeBase::Commit(o)))if *o == head)));
    drop(h);
    remove_fixture(&dir);
}
