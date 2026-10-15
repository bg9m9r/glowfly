//! Renaming a stash from the STASHES row menu and the palette: the dialog opens prefilled for the
//! stash found by commit, the rename runs as one job that keeps every stash's commit and place
//! (and the undo history), and blank, unchanged, busy, vanished and shifted cases are refused.
//! Only the stash list entry changes: the stash commit's own message keeps the old text, and the
//! reflog dates of the entries down to the renamed one become the time of the rename.

use super::palette_stash_tests::two_stashes;
use super::tests::{drive, git};
use super::update;
use crate::effect::Effect;
use crate::msg::Msg;
use crate::msg::PaletteMsg;
use crate::msg::RepoMsg;
use crate::msg::StashMsg;
use crate::state::palette::{Items, Mode, PaletteCommand as C, Pick, PickKind, shown};
use crate::state::{AppState, Modal, RunningJob};
use git2::Oid;
use std::time::Instant;

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn form(st: &AppState) -> &crate::state::stashes::RenameStashForm {
    match &st.modal {
        Some(Modal::RenameStash(f)) => f,
        _ => panic!("the Rename stash dialog is not open"),
    }
}

fn last_log(st: &AppState) -> (bool, String) {
    let l = st.log.last().expect("a log line");
    (l.ok, l.text.clone())
}

fn rename(oid: Oid, message: &str) -> Msg {
    Msg::Stash(StashMsg::Rename { oid, message: message.into() })
}

fn pick(st: &AppState) -> &Pick {
    match st.palette.as_ref().map(|p| &p.mode) {
        Some(Mode::Pick(p)) => p,
        _ => panic!("the palette is not in a sub-list"),
    }
}

#[test]
fn the_dialog_opens_prefilled_for_the_stash() {
    let (mut st, dir) = two_stashes("stash-rename-dialog");
    let first = st.snap.as_ref().unwrap().stashes[1].clone();
    run(&mut st, Msg::Stash(StashMsg::RenameDialog { oid: first.oid }));
    let f = form(&st);
    assert_eq!((f.oid, f.index), (first.oid, 1));
    assert_eq!(f.original, first.message);
    assert_eq!(f.message, "first");
    assert_eq!(f.branch(), first.message.strip_prefix("On ").and_then(|r| r.split_once(':')).map(|(b, _)| b));
    assert!(!f.changed(), "unchanged until edited");

    st.modal = None;
    run(&mut st, Msg::Stash(StashMsg::RenameDialog { oid: Oid::zero() }));
    assert!(st.modal.is_none());
    let (ok, text) = last_log(&st);
    assert!(!ok && text == "That stash no longer exists.", "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn renaming_keeps_the_commit_and_the_place() {
    let (mut st, dir) = two_stashes("stash-rename-run");
    let before = st.snap.as_ref().unwrap().stashes.clone();
    let first = before[1].clone();
    let branch = first.message.strip_prefix("On ").and_then(|r| r.split_once(':')).map(|(b, _)| b.to_owned()).unwrap();
    let kept = trellis_core::undo::Action::DiscardAll { stash: before[0].oid, head: before[0].oid };
    st.undo.record(kept.clone());
    run(&mut st, Msg::Stash(StashMsg::RenameDialog { oid: first.oid }));

    let fx = run(&mut st, rename(first.oid, "  renamed \n"));
    let want = format!("On {branch}: renamed");
    match &fx[..] {
        [Effect::RenameStash { index: 1, oid, message, .. }] => {
            assert_eq!(*oid, first.oid);
            assert_eq!(*message, want);
        }
        other => panic!("expected one RenameStash, got {other:?}"),
    }
    assert!(st.modal.is_none(), "the dialog closes when the job starts");
    let job = st.job.as_ref().expect("a running job");
    assert_eq!(job.label, "Rename stash");
    assert!(job.keeps_undo);

    super::tests::finish(&mut st, fx);
    assert!(st.job.is_none());
    let (ok, text) = last_log(&st);
    assert!(ok && text.starts_with("Rename stash done"), "{text}");
    let after = &st.snap.as_ref().unwrap().stashes;
    assert_eq!(after.len(), 2);
    assert_eq!(after[0].oid, before[0].oid);
    assert_eq!(after[0].message, before[0].message, "the one above is untouched");
    assert_eq!(after[1].oid, first.oid, "same commit, same place");
    assert!(after[1].message.ends_with(": renamed"), "{}", after[1].message);
    assert_eq!(st.undo.undo.last(), Some(&kept), "the undo history is kept");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn refusals_leave_or_close_the_dialog() {
    let (mut st, dir) = two_stashes("stash-rename-refused");
    let first = st.snap.as_ref().unwrap().stashes[1].clone();
    let open = |st: &mut AppState| {
        run(st, Msg::Stash(StashMsg::RenameDialog { oid: first.oid }));
    };

    open(&mut st);
    assert!(run(&mut st, rename(first.oid, " \n ")).is_empty());
    assert_eq!(last_log(&st), (false, "Enter a message for the stash.".into()));
    assert!(st.modal.is_some(), "a blank message keeps the dialog");

    assert!(run(&mut st, rename(first.oid, "first")).is_empty());
    let (ok, text) = last_log(&st);
    assert!(!ok && text.contains("already has this message"), "{text}");
    assert!(st.modal.is_none());

    open(&mut st);
    st.job = Some(RunningJob {
        label: "Fetch".into(),
        workdir: None,
        quiet: false,
        clear_commit: false,
        then_open: None,
        then_push: false,
        keeps_undo: true,
    });
    assert!(run(&mut st, rename(first.oid, "renamed")).is_empty());
    assert_eq!(last_log(&st), (false, "Another operation is still running.".into()));
    assert!(matches!(st.modal, Some(Modal::RenameStash(_))), "busy keeps the dialog");
    assert_eq!(st.job.as_ref().unwrap().label, "Fetch");
    st.job = None;

    assert!(run(&mut st, rename(Oid::zero(), "renamed")).is_empty());
    assert_eq!(last_log(&st), (false, "That stash no longer exists.".into()));
    assert!(st.modal.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_shifted_stash_list_refuses_the_rename() {
    let (mut st, dir) = two_stashes("stash-rename-shifted");
    let first = st.snap.as_ref().unwrap().stashes[1].clone();
    // A terminal pushes a stash after the snapshot: stash@{1} is now "second".
    std::fs::write(dir.join("d.txt"), "d\n").unwrap();
    git(&dir, &["stash", "push", "-u", "-m", "third"]);
    drive(&mut st, rename(first.oid, "renamed"));
    let (ok, text) = last_log(&st);
    assert!(!ok && text.contains("stash list changed") && text.contains("nothing was done"), "{text}");
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.stashes.len(), 3);
    let msgs: Vec<&str> = snap.stashes.iter().map(|s| s.message.as_str()).collect();
    assert!(msgs[0].ends_with(": third") && msgs[1].ends_with(": second") && msgs[2].ends_with(": first"), "{msgs:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_palette_offers_rename_after_delete() {
    let (mut st, dir) = two_stashes("stash-rename-palette");
    let entries = shown(&st, "");
    let at = entries.iter().position(|s| s.entry.label == "Delete stash…").unwrap();
    let e = &entries[at + 1].entry;
    assert_eq!(e.label, "Rename stash…");
    assert_eq!(e.detail.as_deref(), Some("2 stashes"));
    assert!(shown(&st, "reword").iter().any(|s| s.entry.label == "Rename stash…"));

    let msgs: Vec<String> = st.snap.as_ref().unwrap().stashes.iter().map(|s| s.message.clone()).collect();
    run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::ChooseStashToRename)));
    assert_eq!(pick(&st).kind, PickKind::RenameStash);
    assert_eq!(pick(&st).items, Items::Ready(msgs));
    assert_eq!(pick(&st).detail(1).as_deref(), Some("stash@{1}"));
    assert_eq!(st.palette_recent.keys(), ["stash-rename"]);
    let k = PickKind::RenameStash;
    assert_eq!((k.title(), k.prompt()), ("Rename stash", "Find a stash to rename"));
    assert_eq!(k.footer(), "Up/Down to choose, Enter to rename, Backspace or Esc to go back");

    let first = st.snap.as_ref().unwrap().stashes[1].oid;
    let cmd = pick(&st).command(1).unwrap();
    assert_eq!(cmd, C::RenameStash { oid: first });
    run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(cmd)));
    assert!(st.palette.is_none());
    assert_eq!((form(&st).oid, form(&st).index), (first, 1));
    assert_eq!(st.palette_recent.keys(), ["stash-rename"], "a pick is not remembered");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_palette_follows_the_stash_by_commit() {
    let (mut st, dir) = two_stashes("stash-rename-palette-oid");
    run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::ChooseStashToRename)));
    let first = st.snap.as_ref().unwrap().stashes[1].oid;
    let stale = pick(&st).command(1).unwrap();

    // The top stash is dropped while the sub-list is open: "first" is now stash@{0}.
    git(&dir, &["stash", "drop", "stash@{0}"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(stale.clone())));
    assert_eq!((form(&st).oid, form(&st).index), (first, 0));

    // Gone: nothing opens.
    st.modal = None;
    git(&dir, &["stash", "drop", "stash@{0}"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(stale)));
    assert!(st.modal.is_none());
    assert_eq!(last_log(&st), (false, "That stash no longer exists.".into()));
    let _ = std::fs::remove_dir_all(&dir);
}
