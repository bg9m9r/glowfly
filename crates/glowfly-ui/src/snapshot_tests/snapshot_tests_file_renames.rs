//! File rows of renamed and copied files: the weak note after the new path ("from old/path.rs",
//! "copy of src/path.rs") in the Staged list, a commit's details (List and Tree mode), a
//! comparison and a selected stash, how it gives way when the panel is narrow, and that a click
//! on the row still opens the diff. Shares the fixture repository with `snapshot_tests`.

use super::snapshot_tests::{git, harness, remove_fixture, with_repo};
use super::snapshot_tests_details_commit::{head, select};
use super::snapshot_tests_line_counts::{lines, write};
use super::snapshot_tests_painted::{bounds, texts_inside};
use super::snapshot_tests_select::click;
use crate::snapshot_check::CheckSnapshot;
use eframe::egui::accesskit::Role;
use eframe::egui::{PointerButton, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use git2::Oid;
use glowfly_core::repo::{self, DiffTarget, FileEntry, LoadOptions};
use glowfly_core::settings::{FileListMode, SizedPanel, ThemeChoice};
use glowfly_model::msg::{Msg, RepoMsg};
use glowfly_model::state::selection::MultiSelection;
use glowfly_model::state::{AppState, Selection};
use std::path::PathBuf;

type H = Harness<'static, (AppState, Vec<Msg>)>;

const D: &str = "2026-01-14T10:00:00+00:00";
const OLD: &str = "login.txt";
const NEW: &str = "authentication/sign_in.txt";

/// The fixture with login.txt moved (`git mv`) to `NEW`: the Staged Files row of a rename.
fn staged_rename(name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, name);
    std::fs::create_dir_all(dir.join("authentication")).unwrap();
    git(&dir, &["mv", OLD, NEW], D);
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
    st.session.selection = Selection::Wip;
    (st, dir)
}

/// The text painted on the row named `name`, left to right, with whether each piece was cut.
fn on_row(h: &H, name: &str) -> Vec<(String, bool)> {
    texts_inside(h, bounds(&h.get_by_role_and_label(Role::Button, name)))
}

fn said(texts: &[(String, bool)], text: &str) -> bool {
    texts.iter().any(|(t, _)| t == text)
}

#[test]
fn file_rows_renamed_staged() {
    let (st, dir) = staged_rename("renames-staged");
    let mut h = harness(st, true);
    h.run();
    let whole = format!("{OLD} to {NEW}");
    let on = on_row(&h, &whole);
    assert!(said(&on, NEW) && said(&on, "from login.txt"), "the path and its note: {on:?}");
    let at = |t: &str| on.iter().position(|(x, _)| x == t).unwrap();
    assert!(at(NEW) < at("from login.txt"), "the note follows the path");
    assert!(on.iter().all(|(_, cut)| !cut), "a wide row cuts nothing: {on:?}");
    // The Unstaged rows have no note.
    let readme = on_row(&h, "README.md");
    assert!(readme.iter().all(|(t, _)| !t.starts_with("from ")), "an unstaged row has no note: {readme:?}");
    h.check_snapshot("file_rows_renamed_staged");

    // A click on the row still opens the staged diff of the new path.
    let b = h.get_by_role_and_label(Role::Button, &whole).bounding_box().unwrap();
    let at = pos2((b.x1 - 12.0) as f32, ((b.y0 + b.y1) / 2.0) as f32);
    h.state_mut().1.clear();
    click(&mut h, at, PointerButton::Primary);
    let msgs = std::mem::take(&mut h.state_mut().1);
    let staged = DiffTarget::Staged(NEW.into());
    assert!(msgs.iter().any(|m| matches!(m, Msg::Repo(RepoMsg::OpenDiff(t)) if *t == staged)), "{} messages", msgs.len());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn file_rows_renamed_staged_narrow() {
    let (st, dir) = staged_rename("renames-staged-narrow");
    let mut h = harness(st, true);
    let whole = format!("{OLD} to {NEW}");
    let (mut cut, mut gone) = (None, None);
    for width in (260..=520).rev().step_by(4) {
        h.state_mut().0.settings.panel_sizes.set(SizedPanel::Details, width as f32);
        h.run();
        let on = on_row(&h, &whole);
        let note = on.iter().find(|(t, _)| t.starts_with("from"));
        let path_whole = on.iter().any(|(t, c)| t == NEW && !c);
        match note {
            Some((_, true)) if cut.is_none() => {
                assert!(path_whole, "{width}: the path is whole while the note is cut: {on:?}");
                cut = Some(width);
            }
            None if path_whole && gone.is_none() => {
                gone = Some(width);
                // The text snapshot shows the note gone while the path is whole.
                h.check_snapshot("file_rows_renamed_staged_narrow");
            }
            _ => {}
        }
        if gone.is_some() {
            assert!(note.is_none(), "{width}: the note stays gone as the panel narrows: {on:?}");
        }
    }
    let (cut, gone) = (cut.expect("a width cuts the note"), gone.expect("a width drops the note while the path is whole"));
    assert!(gone < cut, "the note is cut at {cut}, dropped only at {gone}");
    drop(h);
    remove_fixture(&dir);
}

/// Two commits: one adds docs/guide.md and notes/todo.md, the next renames them (the guide in its
/// folder, with an edit; the todo list to another folder). The newer one is selected.
fn with_renames(name: &str) -> (AppState, PathBuf, [Oid; 2]) {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, name);
    write(&dir, "docs/guide.md", &lines("Step", 24));
    write(&dir, "notes/todo.md", &lines("- item", 12));
    git(&dir, &["add", "-A"], D);
    git(&dir, &["commit", "-q", "-m", "Add the guide"], D);
    let first = head(&dir);
    std::fs::create_dir_all(dir.join("archive")).unwrap();
    git(&dir, &["mv", "docs/guide.md", "docs/manual.md"], D);
    git(&dir, &["mv", "notes/todo.md", "archive/todo.md"], D);
    write(&dir, "docs/manual.md", &format!("{}Step 25\n", lines("Step", 24)));
    git(&dir, &["add", "-A"], D);
    git(&dir, &["commit", "-q", "-m", "Rename the guide"], D);
    let second = head(&dir);
    select(&mut st, &dir, second);
    (st, dir, [second, first])
}

#[test]
fn file_rows_renamed_commit() {
    let (mut st, dir, _) = with_renames("renames-commit");
    let detail = st.session.detail.as_mut().unwrap();
    let rows: Vec<_> = detail.files.iter().map(|f| (f.status, f.path.as_str(), f.renamed_from.as_deref())).collect();
    assert!(rows.contains(&('R', "docs/manual.md", Some("docs/guide.md"))), "{rows:?}");
    // A copy is listed as an added file with a source (made up here: core lists one only when
    // git is set to find copies).
    detail.files.push(FileEntry { renamed_from: Some("docs/manual.md".into()), ..FileEntry::new("docs/manual_copy.md", 'A') });
    let mut h = harness(st, true);
    h.run();
    let on = on_row(&h, "docs/guide.md to docs/manual.md");
    let at = |t: &str| on.iter().position(|(x, _)| x == t).unwrap_or_else(|| panic!("{t} on the row: {on:?}"));
    assert!(at("docs/manual.md") < at("from docs/guide.md") && at("from docs/guide.md") < at("+1"), "path, note, then the counts: {on:?}");
    let copy = on_row(&h, "docs/manual_copy.md, copied from docs/manual.md");
    assert!(said(&copy, "copy of docs/manual.md"), "{copy:?}");
    h.check_snapshot("file_rows_renamed_commit");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn file_rows_renamed_commit_tree() {
    let (mut st, dir, _) = with_renames("renames-commit-tree");
    st.settings.file_list_mode = FileListMode::Tree;
    let mut h = harness(st, true);
    h.run();
    // Within one folder only the old name; across folders the whole old path.
    let same = on_row(&h, "docs/guide.md to docs/manual.md");
    assert!(said(&same, "manual.md") && said(&same, "from guide.md"), "{same:?}");
    let across = on_row(&h, "notes/todo.md to archive/todo.md");
    assert!(said(&across, "todo.md") && said(&across, "from notes/todo.md"), "{across:?}");
    h.check_snapshot("file_rows_renamed_commit_tree");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn file_rows_renamed_compare() {
    let (mut st, dir, oids) = with_renames("renames-compare");
    let mut sel = MultiSelection::build(oids, oids[0], st.snap.as_ref().unwrap());
    if let Selection::Commits(m) = &mut sel {
        let (from, to) = m.compare_ends().expect("a pair");
        m.changes = Some(repo::compare(&dir, from, to).map_err(|e| e.to_string()));
    }
    st.session.selection = sel;
    let mut h = harness(st, true);
    h.run();
    let on = on_row(&h, "docs/guide.md to docs/manual.md");
    assert!(said(&on, "from docs/guide.md"), "{on:?}");
    h.check_snapshot("file_rows_renamed_compare");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn file_rows_renamed_stash() {
    let (mut st, dir) = staged_rename("renames-stash");
    git(&dir, &["stash"], D);
    let stash = git2::Repository::open(&dir).unwrap().revparse_single("refs/stash").unwrap().id();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
    st.session.selection = Selection::Commit(stash);
    st.session.detail = Some(repo::commit_detail(&dir, stash).unwrap());
    let mut h = harness(st, true);
    h.run();
    let on = on_row(&h, &format!("{OLD} to {NEW}"));
    assert!(said(&on, "from login.txt"), "a stash's list names the rename's old path: {on:?}");
    h.check_snapshot("file_rows_renamed_stash");
    drop(h);
    remove_fixture(&dir);
}
