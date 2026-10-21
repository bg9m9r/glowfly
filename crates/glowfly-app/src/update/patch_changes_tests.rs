//! Headless tests for the Create patch dialog on uncommitted changes: opening it, Save… with
//! each scope and its refusals, the trust prompt in an untrusted repository, and an end-to-end
//! save whose toast shows the file in the file manager.

use super::super::test_mods::patch_tests::{beside, cleanup, last_log, last_toast, run, three_commits};
use super::super::tests::{drive, git};
use super::super::update;
use crate::effect::Effect;
use crate::effect::OsEffect;
use crate::msg::{Msg, RepoMsg};
use crate::patch::{PatchEffect, PatchMsg};
use crate::state::patch::changes::{NO_PICK, NOTHING_STAGED, RESOLVE_FIRST, SaveChangesForm};
use crate::state::{AppState, Modal};
use glowfly_core::patch_ops::{ChangesRequest, NO_CHANGES, PatchSource, WipScope};
use glowfly_core::repo::{FileEntry, Hazard};
use std::path::PathBuf;
use std::time::Instant;

/// `three_commits` with a.txt edited and c.txt new (untracked).
fn dirty(name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = three_commits(name);
    std::fs::write(dir.join("a.txt"), "one\ntwo\nthree\n").unwrap();
    std::fs::write(dir.join("c.txt"), "c\n").unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    (st, dir)
}

fn save(st: &mut AppState, scope: WipScope, paths: Option<&[&str]>, untracked: bool) -> Vec<Effect> {
    let paths = paths.map(|p| p.iter().map(|s| (*s).to_owned()).collect());
    run(st, PatchMsg::SaveChanges { scope, paths, untracked })
}

/// The (request, name) of the save dialog `fx` asks for.
fn picked(fx: &[Effect]) -> (ChangesRequest, String) {
    match fx {
        [Effect::Patch(PatchEffect::PickSave { what: PatchSource::Changes(r), name, .. })] => (r.clone(), name.clone()),
        other => panic!("unexpected effects {other:?}"),
    }
}

#[test]
fn the_dialog_opens_on_a_scope_or_one_file() {
    let (mut st, dir) = dirty("patch-wip-dialog");
    run(&mut st, PatchMsg::ChangesDialog { scope: WipScope::All, path: None });
    assert!(matches!(&st.modal, Some(Modal::SavePatch(f)) if *f == SaveChangesForm::new(WipScope::All, None)));
    run(&mut st, PatchMsg::ChangesDialog { scope: WipScope::Staged, path: Some("a.txt".into()) });
    let Some(Modal::SavePatch(f)) = &st.modal else { panic!("the dialog") };
    assert_eq!((f.scope, f.pick, f.chosen.iter().map(String::as_str).collect::<Vec<_>>()), (WipScope::Staged, true, vec!["a.txt"]));
    cleanup(&dir);
}

#[test]
fn save_asks_where_with_the_scope_and_a_name() {
    let (mut st, dir) = dirty("patch-wip-save");
    st.modal = Some(Modal::SavePatch(SaveChangesForm::new(WipScope::All, None)));
    let (r, name) = picked(&save(&mut st, WipScope::All, None, true));
    assert_eq!(
        (r.scope, r.paths.is_empty(), r.picked, r.untracked.as_slice(), r.unborn),
        (WipScope::All, true, false, &["c.txt".to_owned()][..], false)
    );
    assert_eq!(name, "main-changes.patch");
    assert!(st.modal.is_none(), "the dialog closes once the save dialog is asked for");

    let (r, _) = picked(&save(&mut st, WipScope::Unstaged, None, false));
    assert!(r.untracked.is_empty(), "untracked files left out");
    let (r, name) = picked(&save(&mut st, WipScope::Unstaged, Some(&["c.txt"]), false));
    assert_eq!((r.paths.len(), r.untracked.as_slice(), r.picked), (0, &["c.txt".to_owned()][..], true));
    assert_eq!(name, "c-txt-changes.patch");
    assert!(st.job.is_none(), "saving a patch is no job");
    cleanup(&dir);
}

#[test]
fn save_refusals() {
    let (mut st, dir) = dirty("patch-wip-refuse");
    assert!(save(&mut st, WipScope::Staged, None, true).is_empty());
    assert_eq!(last_log(&st), NOTHING_STAGED);
    assert!(save(&mut st, WipScope::All, Some(&[]), true).is_empty());
    assert_eq!(last_log(&st), NO_PICK);
    // Only untracked files, which are left out.
    git(&dir, &["checkout", "--", "a.txt"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(save(&mut st, WipScope::Unstaged, None, false).is_empty());
    assert_eq!(last_log(&st), NO_CHANGES);
    st.snap.as_mut().unwrap().unstaged.push(FileEntry::new("x.txt", '!'));
    assert!(save(&mut st, WipScope::All, None, true).is_empty());
    assert_eq!(last_log(&st), RESOLVE_FIRST);
    cleanup(&dir);
}

#[test]
fn an_untrusted_repository_asks_first() {
    let (mut st, dir) = dirty("patch-wip-trust");
    let real = st.snap.as_ref().unwrap().real_workdir.clone();
    crate::state::trust::record(&mut st.settings, &real, glowfly_core::settings::RepoTrust::NotTrusted);
    assert!(st.held());
    let fx = save(&mut st, WipScope::All, None, true);
    assert!(!fx.iter().any(|e| matches!(e, Effect::Patch(_))), "{fx:?}");
    assert_eq!(last_log(&st), super::changes::HELD);
    assert!(matches!(st.modal, Some(Modal::Trust(_))), "the trust prompt");
    cleanup(&dir);
}

#[test]
fn saved_end_to_end_with_show_in_folder() {
    let (mut st, dir) = dirty("patch-wip-e2e");
    let (req, _) = picked(&save(&mut st, WipScope::All, None, true));
    let dest = beside(&dir, "wip.patch");
    let workdir = st.workdir().unwrap();
    let fx = run(&mut st, PatchMsg::SaveTo { workdir: workdir.clone(), what: PatchSource::Changes(req.clone()), dest: Some(dest.clone()) });
    assert!(matches!(&fx[..], [Effect::Patch(PatchEffect::Export { dest: d, .. })] if *d == dest), "{fx:?}");
    drive(&mut st, Msg::Patch(PatchMsg::SaveTo { workdir, what: PatchSource::Changes(req), dest: Some(dest.clone()) }));
    assert_eq!(last_toast(&st), (true, "Saved the changes to wip.patch (2 files)".to_owned()));
    let text = std::fs::read_to_string(&dest).unwrap();
    assert!(text.contains("b/a.txt") && text.contains("b/c.txt"), "{text}");
    let id = st.toasts.last().unwrap().id;
    let fx = update(&mut st, Msg::ToastAction(id), Instant::now());
    assert!(matches!(&fx[..], [Effect::Os(OsEffect::RevealPath(p))] if *p == dest), "{fx:?}");
    cleanup(&dir);
}

#[test]
fn a_row_trellis_cannot_name_is_refused() {
    let (mut st, dir) = dirty("patch-wip-hazard");
    let shown = "caf\\xE9.txt";
    let odd = FileEntry { hazard: Some(Hazard::NotUtf8(b"caf\xe9.txt".to_vec())), ..FileEntry::new(shown, '?') };
    st.snap.as_mut().unwrap().unstaged.push(odd);
    let fx = run(&mut st, PatchMsg::ChangesDialog { scope: WipScope::Unstaged, path: Some(shown.into()) });
    assert!(fx.is_empty() && st.modal.is_none());
    assert!(st.toasts.last().is_some_and(|t| !t.ok && t.text.contains("isn't UTF-8")));
    cleanup(&dir);
}
