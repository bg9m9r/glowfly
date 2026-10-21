//! Custom commands in the palette (with a key hint and each entry's detail) and in the Commands
//! submenu of the commit, local branch and file menus: an item that can't run from the row is
//! disabled, a click sends the run message, and the submenu is absent when no command shows
//! there. Shares the fixture repository and renderer with `snapshot_tests`.

use super::menu_testing::{open_submenu, submenu_item};
use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::snapshot_tests_select::{click, row_y};
use crate::custom_commands::{CommandRef, CustomCommandMsg, Target};
use crate::msg::{Msg, PaletteMsg};
use crate::state::AppState;
use crate::state::palette::{Palette, PaletteCommand};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::accesskit::Role;
use eframe::egui::{PointerButton, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use git2::Oid;
use trellis_core::custom_commands::{Source, parse};
use trellis_core::settings::ThemeChoice;

type H = Harness<'static, (AppState, Vec<Msg>)>;

const USER: &str = r#"
[[command]]
name = "Deploy preview"
kind = "shell"
template = "./deploy.sh preview {branch}"
key = "Mod+Alt+D"

[[command]]
name = "Deploy commit"
kind = "shell"
template = "./deploy.sh at {sha}"
show = ["palette", "commit"]

[[command]]
name = "Show file at commit"
kind = "git"
template = "show {sha} -- {files}"
show = ["commit", "file"]

[[command]]
name = "Push branch to staging"
kind = "git"
template = "push staging {branch}"
show = ["branch"]
"#;

const REPO: &str = r#"
[[command]]
name = "Deploy with project script"
kind = "shell"
template = "make deploy"
show = ["palette", "commit", "branch", "file"]
"#;

/// The fixture with the commands above loaded, the user's and the repository's.
fn with_commands(name: &str) -> (AppState, std::path::PathBuf) {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, name);
    let (user, problems) = parse(USER);
    assert!(problems.is_empty(), "{problems:?}");
    let (repo, problems) = parse(REPO);
    assert!(problems.is_empty(), "{problems:?}");
    let t = st.settings.lang();
    let (keys, problems) =
        crate::state::custom_commands::chords(t, &st.keymap, &[(Source::User, "commands.toml", &user), (Source::Repo, "repo", &repo)]);
    assert!(problems.is_empty(), "{problems:?}");
    let cc = &mut st.custom_commands;
    (cc.user, cc.repo, cc.keys) = (user, repo, keys);
    cc.repo_for = Some(st.snap.as_ref().unwrap().workdir.clone());
    (st, dir)
}

/// Whether a message to run `name` from `at` was sent, from a menu or the palette.
fn pushed_run(h: &H, name: &str, at: &Target) -> bool {
    let runs = |m: &CustomCommandMsg| matches!(m, CustomCommandMsg::Run { command: CommandRef { name: n, .. }, at: a, .. } if n == name && a == at);
    h.state().1.iter().any(|m| match m {
        Msg::CustomCommand(c) => runs(c),
        Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::Custom(c))) => runs(c),
        _ => false,
    })
}

#[test]
fn palette_custom_commands() {
    let (mut st, dir) = with_commands("cc-palette");
    st.palette = Some(Palette { query: "deploy".into(), ..Default::default() });
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("palette_custom_commands");
    assert!(h.query_by_label("Show file at commit").is_none(), "not shown in the palette");
    h.get_by_label("Deploy preview").click();
    h.run();
    assert!(pushed_run(&h, "Deploy preview", &Target::Palette));
    h.state_mut().0.palette = Some(Palette { query: "reload custom".into(), ..Default::default() });
    h.run();
    assert!(h.query_by_label("Reload custom commands").is_some());
    drop(h);
    remove_fixture(&dir);
}

fn commit(st: &AppState, summary: &str) -> Oid {
    st.snap.as_ref().unwrap().commits.iter().find(|c| c.summary == summary).unwrap().oid
}

#[test]
fn commit_menu_commands() {
    let (st, dir) = with_commands("cc-commit-menu");
    let fix = commit(&st, "Fix build on Windows");
    let row = st.snap.as_ref().unwrap().row_of[&fix] + 1;
    let mut h = harness(st, true);
    h.run();
    click(&mut h, pos2(700.0, row_y(row)), PointerButton::Secondary);
    open_submenu(&mut h, "Commands");
    h.check_snapshot("commit_menu_commands");
    assert!(submenu_item(&h, "Commands", "Show file at commit").is_disabled(), "a commit row has no file");
    assert!(h.query_by_label("Push branch to staging").is_none(), "only in the branch menu");
    submenu_item(&h, "Commands", "Deploy commit").click();
    h.run();
    assert!(pushed_run(&h, "Deploy commit", &Target::Commit(fix)));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn branch_menu_commands() {
    let (st, dir) = with_commands("cc-branch-menu");
    let mut h = harness(st, true);
    h.run();
    let rect = h.get_all_by_label("search").filter_map(|n| n.bounding_box()).min_by(|a, b| a.y0.total_cmp(&b.y0)).expect("the row");
    click(&mut h, pos2(((rect.x0 + rect.x1) / 2.0) as f32, ((rect.y0 + rect.y1) / 2.0) as f32), PointerButton::Secondary);
    open_submenu(&mut h, "Commands");
    h.check_snapshot("branch_menu_commands");
    submenu_item(&h, "Commands", "Push branch to staging").click();
    h.run();
    assert!(pushed_run(&h, "Push branch to staging", &Target::Branch("search".into())));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn file_menu_commands() {
    let (st, dir) = with_commands("cc-file-menu");
    let mut h = harness(st, true);
    h.run();
    let b = h.get_by_role_and_label(Role::Button, "README.md").bounding_box().expect("the row");
    click(&mut h, pos2(b.x1 as f32 - 12.0, ((b.y0 + b.y1) / 2.0) as f32), PointerButton::Secondary);
    open_submenu(&mut h, "Commands");
    h.check_snapshot("file_menu_commands");
    // The submenu sits over the menu at the window's edge; its labels are unique here.
    h.get_by_label("Show file at commit").click();
    h.run();
    assert!(pushed_run(&h, "Show file at commit", &Target::File { path: "README.md".into(), at: None }));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn no_commands_no_submenu() {
    let (st, dir) = with_repo(ThemeChoice::Dark, "cc-no-commands");
    let fix = commit(&st, "Fix build on Windows");
    let row = st.snap.as_ref().unwrap().row_of[&fix] + 1;
    let mut h = harness(st, true);
    h.run();
    click(&mut h, pos2(700.0, row_y(row)), PointerButton::Secondary);
    assert!(h.query_by_label("Copy").is_some(), "the menu is open");
    assert!(h.query_by_label("Commands").is_none());
    drop(h);
    remove_fixture(&dir);
}
