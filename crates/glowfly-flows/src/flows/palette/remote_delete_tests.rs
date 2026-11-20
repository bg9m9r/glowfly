//! Headless tests for the palette's Delete remote branch… entry: listed only while a remote has a
//! branch (not just its HEAD), its step lists every remote branch sorted with the local branches
//! tracking it, a chosen branch opens the same Delete remote branch dialog as the branch menu (a
//! branch that has gone is logged), and for a typed query it lists below creating, switching and
//! checking out branches.

use crate::flows::branch_tests::with_origin;
use crate::flows::palette::palette_tests::repo_with_topic;
use crate::flows::{drive, git, state, update};
use glowfly_model::effect::Effect;
use glowfly_model::msg::{BranchMsg, Msg, PaletteMsg, RepoMsg};
use glowfly_model::state::palette::{Mode, PaletteCommand as C, Pick, PickKind, shown};
use glowfly_model::state::{AppState, Modal};
use std::path::Path;
use std::time::Instant;

const ENTRY: &str = "Delete remote branch…";

fn pick(st: &AppState) -> &Pick {
    match st.palette.as_ref().map(|p| &p.mode) {
        Some(Mode::Pick(p)) => p,
        _ => panic!("the palette is not in a sub-list"),
    }
}

fn labels(st: &AppState, query: &str) -> Vec<String> {
    shown(st, query).into_iter().map(|s| s.entry.label).collect()
}

fn run(st: &mut AppState, cmd: C) -> Vec<Effect> {
    update(st, Msg::Palette(PaletteMsg::RunPaletteCommand(cmd)), Instant::now())
}

fn refresh(st: &mut AppState) {
    drive(st, Msg::Repo(RepoMsg::Refresh));
}

fn at(list: &[String], label: &str) -> usize {
    list.iter().position(|l| l == label).unwrap_or_else(|| panic!("{label:?} is listed in {list:?}"))
}

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

fn remote(name: &str, branch: &str) -> C {
    C::DeleteRemoteBranch { remote: name.to_owned(), branch: branch.to_owned() }
}

#[test]
fn the_entry_comes_with_a_remote_branch() {
    let mut st = state();
    assert!(!labels(&st, "").iter().any(|l| l == ENTRY), "no repository: no entry");
    run(&mut st, C::ChooseRemoteBranchToDelete);
    assert!(st.palette.is_none(), "no repository: no step");

    let (mut st, dir) = repo_with_topic("pal-del-remote-entry");
    assert!(!labels(&st, "").iter().any(|l| l == ENTRY), "no remotes: no entry");
    git(&dir, &["remote", "add", "origin", &dir.display().to_string()]);
    refresh(&mut st);
    assert!(!labels(&st, "").iter().any(|l| l == ENTRY), "a remote without fetched branches: no entry");
    run(&mut st, C::ChooseRemoteBranchToDelete);
    assert!(st.palette.is_none(), "no remote branches: no step");
    cleanup(&[&dir]);

    let (mut st, work, bare) = with_origin("pal-del-remote-entry-origin");
    assert!(labels(&st, "").iter().any(|l| l == ENTRY));
    git(&work, &["remote", "set-head", "origin", "main"]);
    refresh(&mut st);
    assert!(labels(&st, "").iter().any(|l| l == ENTRY));
    run(&mut st, C::ChooseRemoteBranchToDelete);
    let pk = pick(&st);
    assert!((0..pk.rank("").unwrap().total).all(|i| pk.label(i) != "origin/HEAD"));
    cleanup(&[&work, &bare]);
}

#[test]
fn the_step_lists_every_remote_branch_sorted_with_its_trackers() {
    let (mut st, work, bare) = with_origin("pal-del-remote-list");
    git(&work, &["branch", "--track", "copy", "origin/feature"]);
    let backup = work.with_file_name(format!("{}-backup.git", work.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&backup);
    std::fs::create_dir_all(&backup).unwrap();
    git(&backup, &["init", "-q", "--bare", "-b", "main"]);
    git(&work, &["push", "-q", &backup.display().to_string(), "main", "feature"]);
    git(&work, &["remote", "add", "backup", &backup.display().to_string()]);
    git(&work, &["fetch", "-q", "backup"]);
    git(&work, &["push", "-q", "origin", "main:solo"]);
    refresh(&mut st);
    run(&mut st, C::ChooseRemoteBranchToDelete);
    let pk = pick(&st);
    assert_eq!(pk.kind, PickKind::DeleteRemoteBranch);
    let rows = ["backup/feature", "backup/main", "origin/feature", "origin/main", "origin/solo"];
    assert_eq!(pk.rank("").unwrap().total, rows.len());
    for (i, label) in rows.iter().enumerate() {
        assert_eq!(pk.label(i), *label);
        let (r, b) = label.split_once('/').unwrap();
        assert_eq!(pk.command(i), Some(remote(r, b)));
        assert_eq!(pk.blocked(i), None);
    }
    let details = [None, None, Some("copy, feature"), Some("main"), None];
    for (i, want) in details.iter().enumerate() {
        assert_eq!(pk.detail(i).as_deref(), *want, "{}", rows[i]);
    }
    assert_eq!(pk.rank("copy").unwrap().total, 1, "a tracking branch's name finds the remote branch");
    let k = PickKind::DeleteRemoteBranch;
    assert_eq!((k.title(), k.prompt(), k.loading()), ("Delete remote branch", "Find a remote branch to delete", "Loading branches…"));
    assert_eq!((k.none_text(), k.empty_text()), ("No matching remote branches", "This repository has no remote branches"));
    assert_eq!(k.footer(), "Up/Down to choose, Enter to choose, Backspace or Esc to go back");
    assert_eq!(k.more_text(1, 2), "Showing 1 of 2 remote branches. Type more to narrow the list.");
    assert!(k.lists_refs() && !k.dims_folders());
    cleanup(&[&work, &bare, &backup]);
}

#[test]
fn a_chosen_remote_branch_opens_the_delete_remote_dialog() {
    let (mut st, work, bare) = with_origin("pal-del-remote-open");
    git(&work, &["branch", "--track", "copy", "origin/feature"]);
    refresh(&mut st);
    // What the remote branch menu's dialog is, to compare with.
    let msg = Msg::Branch(BranchMsg::DeleteRemoteDialog { remote: "origin".into(), name: "feature".into() });
    let fx = update(&mut st, msg, Instant::now());
    let (modal, effects) = (st.modal.take(), format!("{fx:?}"));
    let Some(Modal::DeleteRemoteBranch(form)) = modal else { panic!("the menu's dialog opens") };
    assert_eq!(form.tracking, ["copy", "feature"]);

    assert_eq!(C::ChooseRemoteBranchToDelete.recall_key(), Some("delete-remote-branch"));
    assert_eq!(remote("origin", "feature").recall_key(), None);
    run(&mut st, C::ChooseRemoteBranchToDelete);
    let fx = run(&mut st, remote("origin", "feature"));
    assert!(st.palette.is_none(), "the palette closes for the dialog");
    assert!(matches!(st.modal.take(), Some(Modal::DeleteRemoteBranch(f)) if f == form), "the same dialog");
    assert_eq!(format!("{fx:?}"), effects, "and the same effects");
    assert_eq!(st.palette_recent.keys(), ["delete-remote-branch"], "the step is remembered, the choice isn't");
    cleanup(&[&work, &bare]);
}

#[test]
fn a_remote_branch_gone_since_the_list_was_made_is_logged() {
    let (mut st, work, bare) = with_origin("pal-del-remote-gone");
    git(&work, &["push", "-q", "origin", "main:solo"]);
    refresh(&mut st);
    run(&mut st, C::ChooseRemoteBranchToDelete);
    assert_eq!(pick(&st).label(2), "origin/solo");
    git(&work, &["branch", "-q", "-r", "-d", "origin/solo"]);
    refresh(&mut st);
    run(&mut st, remote("origin", "solo"));
    assert!(st.palette.is_none());
    assert!(st.modal.is_none());
    assert!(st.log.last().is_some_and(|l| !l.ok && l.text == "origin/solo no longer exists."), "{:?}", st.log.last());
    cleanup(&[&work, &bare]);
}

#[test]
fn branch_query_ranks_create_switch_and_checkouts_first() {
    let (mut st, work, bare) = with_origin("pal-del-remote-rank");
    let below_the_others = |l: &[String]| {
        let delete = at(l, ENTRY);
        for before in ["Create branch at HEAD…", "Switch branch…", "Checkout feature"] {
            assert!(at(l, before) < delete, "{before} is above {ENTRY} in {l:?}");
        }
    };
    for query in ["branch", "bran", "br"] {
        below_the_others(&labels(&st, query));
    }
    for query in ["delete rem", "delete remote"] {
        assert_eq!(labels(&st, query)[0], ENTRY, "typed by its own name it isn't demoted: {query:?}");
    }
    run(&mut st, C::ChooseRemoteBranchToDelete);
    assert_eq!(labels(&st, "")[0], ENTRY, "a recent run is first for an empty query");
    cleanup(&[&work, &bare]);
}
