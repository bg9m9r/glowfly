//! Headless tests for the palette's Delete branch… entry: listed only while there is a local
//! branch besides the checked-out one, its step lists every local branch (those that can be
//! deleted first, the others dim with their reason), a chosen branch opens the Delete branch
//! dialog and a blocked one changes nothing, and for a typed query it lists below creating,
//! switching and checking out branches.

use crate::flows::branch_tests::with_origin;
use crate::flows::palette::palette_tests::repo_with_topic;
use crate::flows::worktree_branch_tests::{cleanup, with_side};
use crate::flows::{drive, git, state, update};
use glowfly_core::i18n::Lang;
use glowfly_model::effect::Effect;
use glowfly_model::msg::{BranchMsg, Msg, PaletteMsg, RepoMsg};
use glowfly_model::state::palette::{Mode, PaletteCommand as C, Pick, PickKind, shown};
use glowfly_model::state::worktree_branches::delete_blocked;
use glowfly_model::state::{AppState, Modal};
use std::time::Instant;

const ENTRY: &str = "Delete branch…";

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

fn at(list: &[String], label: &str) -> usize {
    list.iter().position(|l| l == label).unwrap_or_else(|| panic!("{label:?} is listed in {list:?}"))
}

#[test]
fn the_entry_comes_and_goes_with_the_other_branches() {
    let mut st = state();
    assert!(!labels(&st, "").iter().any(|l| l == ENTRY), "no repository: no entry");
    run(&mut st, C::ChooseBranchToDelete);
    assert!(st.palette.is_none(), "no repository: no step");

    let (mut st, dir) = repo_with_topic("pal-del-branch-entry");
    assert!(labels(&st, "").iter().any(|l| l == ENTRY));
    git(&dir, &["branch", "-D", "topic"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(!labels(&st, "").iter().any(|l| l == ENTRY), "only the checked-out branch: no entry");
    run(&mut st, C::ChooseBranchToDelete);
    assert!(st.palette.is_none(), "only the checked-out branch: no step");

    git(&dir, &["checkout", "-q", "--detach"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(labels(&st, "").iter().any(|l| l == ENTRY), "a detached HEAD has no current branch");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_step_lists_every_local_branch() {
    let (mut st, work, bare) = with_origin("pal-del-branch-list");
    git(&work, &["branch", "other"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    run(&mut st, C::ChooseBranchToDelete);
    let pk = pick(&st);
    assert_eq!(pk.kind, PickKind::DeleteBranch);
    let names = ["feature", "other", "main"];
    assert_eq!(names.len(), st.snap.as_ref().unwrap().locals.len());
    for (i, name) in names.iter().enumerate() {
        assert_eq!(pk.label(i), *name);
        assert_eq!(pk.command(i), Some(C::DeleteBranch((*name).to_owned())));
    }
    assert_eq!(pk.detail(0), Some("origin/feature".to_owned()));
    assert_eq!(pk.detail(1), None);
    let k = PickKind::DeleteBranch;
    assert_eq!((k.title(), k.prompt(), k.loading()), ("Delete branch", "Find a branch to delete", "Loading branches…"));
    assert_eq!((k.none_text(), k.empty_text()), ("No matching branches", "This repository has no local branches"));
    assert_eq!(k.footer(), "Up/Down to choose, Enter to choose, Backspace or Esc to go back");
    assert_eq!(k.more_text(1, 2), "Showing 1 of 2 branches. Type more to narrow the list.");
    assert!(k.lists_refs() && !k.dims_folders());
    let _ = std::fs::remove_dir_all(&work);
    let _ = std::fs::remove_dir_all(&bare);
}

#[test]
fn the_checked_out_branch_is_blocked_with_its_reason() {
    let (mut st, work, bare) = with_origin("pal-del-branch-head");
    run(&mut st, C::ChooseBranchToDelete);
    let pk = pick(&st);
    let rows = pk.rank("").unwrap().total;
    let main = (0..rows).find(|&i| pk.label(i) == "main").expect("main is listed");
    let reason = st.settings.lang().trf("branch.delete.checked_out", &[("branch", &"main")]);
    assert_eq!(pk.blocked(main), Some(reason.as_str()));
    assert_eq!(pk.shown_detail(main, &st.settings, 0, 0), Some(reason.clone()));
    assert_eq!(pk.detail(main), Some("origin/main".to_owned()), "the keyword stays the upstream");
    for i in (0..rows).filter(|&i| i != main) {
        assert_eq!(pk.blocked(i), None);
    }
    assert_eq!(pk.rank("deleting").unwrap().total, 0, "the reason is no keyword");
    assert_eq!(pk.rank("origin").unwrap().total, 2, "the upstream is");

    let (log, toasts) = (st.log.len(), st.toasts.len());
    let fx = run(&mut st, C::DeleteBranch("main".into()));
    assert!(fx.is_empty() && st.modal.is_none());
    assert!(matches!(st.palette.as_ref().map(|p| &p.mode), Some(Mode::Pick(p)) if p.kind == PickKind::DeleteBranch));
    assert_eq!((st.log.len(), st.toasts.len()), (log, toasts), "nothing is logged or toasted");
    let _ = std::fs::remove_dir_all(&work);
    let _ = std::fs::remove_dir_all(&bare);
}

#[test]
fn a_chosen_branch_opens_the_delete_dialog() {
    let (mut st, work, bare) = with_origin("pal-del-branch-open");
    git(&work, &["branch", "other"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    // What the branch menu's dialog is, to compare with.
    let fx = update(&mut st, Msg::Branch(BranchMsg::DeleteDialog("feature".into())), Instant::now());
    let (modal, effects) = (st.modal.take(), format!("{fx:?}"));
    let Some(Modal::DeleteBranch(form)) = modal else { panic!("the menu's dialog opens") };
    assert_eq!(form.name, "feature");

    assert_eq!(C::ChooseBranchToDelete.recall_key(), Some("delete-branch"));
    assert_eq!(C::DeleteBranch("feature".into()).recall_key(), None);
    run(&mut st, C::ChooseBranchToDelete);
    let fx = run(&mut st, C::DeleteBranch("feature".into()));
    assert!(st.palette.is_none(), "the palette closes for the dialog");
    assert!(matches!(st.modal.take(), Some(Modal::DeleteBranch(f)) if f == form), "the same dialog");
    assert_eq!(format!("{fx:?}"), effects, "and the same effects");
    assert_eq!(st.palette_recent.keys(), ["delete-branch"], "the step is remembered, the choice isn't");

    // A branch that has gone closes the palette and is logged, as from the menu.
    git(&work, &["branch", "-D", "other"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    run(&mut st, C::ChooseBranchToDelete);
    run(&mut st, C::DeleteBranch("other".into()));
    assert!(st.palette.is_none());
    assert!(st.log.last().is_some_and(|l| !l.ok && l.text == "Branch other no longer exists."));
    let _ = std::fs::remove_dir_all(&work);
    let _ = std::fs::remove_dir_all(&bare);
}

#[test]
fn a_branch_in_another_worktree_is_blocked_with_its_reason() {
    let (mut st, dir, side) = with_side("pal-del-branch-worktree");
    run(&mut st, C::ChooseBranchToDelete);
    let pk = pick(&st);
    let rows: Vec<&str> = (0..3).map(|i| pk.label(i)).collect();
    assert_eq!(rows, ["free", "main", "side"], "free first, then main and side");
    let snap = st.snap.as_ref().unwrap();
    let why = delete_blocked(Lang::En, snap, "side").expect("another worktree has side");
    assert_eq!(pk.blocked(2), Some(why.as_str()));
    assert_eq!(pk.blocked(0), None);

    let log = st.log.len();
    let fx = run(&mut st, C::DeleteBranch("side".into()));
    assert!(fx.is_empty() && st.modal.is_none() && st.log.len() == log);
    assert!(matches!(st.palette.as_ref().map(|p| &p.mode), Some(Mode::Pick(_))), "the step stays open");
    run(&mut st, C::DeleteBranch("free".into()));
    assert!(matches!(&st.modal, Some(Modal::DeleteBranch(f)) if f.name == "free"));
    cleanup(&dir, &side);
}

#[test]
fn branch_query_ranks_create_and_checkouts_first() {
    let (mut st, dir) = repo_with_topic("pal-del-branch-rank");
    let below_the_others = |l: &[String]| {
        let delete = at(l, ENTRY);
        for before in ["Create branch at HEAD…", "Switch branch…", "Checkout topic"] {
            assert!(at(l, before) < delete, "{before} is above {ENTRY} in {l:?}");
        }
    };
    for query in ["branch", "bran", "br"] {
        below_the_others(&labels(&st, query));
    }
    run(&mut st, C::ChooseBranchToDelete);
    below_the_others(&labels(&st, "branch"));
    assert_eq!(labels(&st, "")[0], ENTRY, "a recent run is first for an empty query");
    for query in ["del", "delete b"] {
        assert_eq!(labels(&st, query)[0], ENTRY, "typed by its own name it isn't demoted: {query:?}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
