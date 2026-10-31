//! Create patch from uncommitted changes, end to end through update and the runner: the export
//! (`git diff` in the working tree) is re-checked just before it runs, so settings the
//! repository gains while the save dialog is open wait for the trust prompt.

use super::tests::{git, scratch_repo, state};
use super::trust_recheck_tests::{cleanup, prompt, recheck, sealed, trust_and_run};
use super::trust_tests::{asking, open};
use crate::effect::{Effect, Sealed, WaitingKind};
use crate::msg::Msg;
use crate::patch::{PatchEffect, PatchMsg};
use glowfly_core::patch_ops::{ChangesRequest, PatchSource, WipScope};
use std::path::{Path, PathBuf};

/// Save to `dest` from the save dialog, for every uncommitted change in `workdir`.
fn save_to(workdir: &Path, dest: &Path) -> Msg {
    let req = ChangesRequest { scope: WipScope::All, paths: Vec::new(), picked: false, untracked: Vec::new(), unborn: false };
    Msg::Patch(PatchMsg::SaveTo { workdir: workdir.to_path_buf(), what: PatchSource::Changes(req), dest: Some(dest.to_path_buf()) })
}

/// The checked export among `fx`.
fn checked_export(fx: Vec<Effect>) -> Effect {
    let export = fx.into_iter().find(|e| matches!(e, Effect::Checked(c) if matches!(c.effect, Effect::Patch(PatchEffect::Export { .. }))));
    export.expect("a checked export")
}

fn dest(dir: &Path, name: &str) -> PathBuf {
    dir.with_file_name(format!("{}-{name}.patch", dir.file_name().unwrap().to_string_lossy()))
}

#[test]
fn a_patch_export_is_checked_again_before_git_diff_runs() {
    let dir = scratch_repo("trust-patch-export");
    std::fs::write(dir.join("p.txt"), "one\n").unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "p"]);
    std::fs::write(dir.join("p.txt"), "two\n").unwrap();
    let mut st = state();
    open(&mut st, &dir);
    assert!(!asking(&st), "nothing in it runs programs");
    // Unchanged since it was read: the export runs.
    let first = dest(&dir, "first");
    let export = checked_export(sealed(&mut st, save_to(&dir, &first)));
    let label = st.settings.lang().tr("patch.changes.export_label").to_owned();
    assert_eq!(export.trust_check().map(|c| c.sealed.clone()), Some(Sealed::Waiting { kind: WaitingKind::Read, label: Some(label) }));
    assert!(recheck(&mut st, export));
    assert!(first.exists(), "the patch was written");
    // The repository starts using a proxy while the save dialog is open: git diff doesn't run.
    git(&dir, &["config", "http.proxy", "proxy.example:8080"]);
    let second = dest(&dir, "second");
    let export = checked_export(sealed(&mut st, save_to(&dir, &second)));
    assert!(!recheck(&mut st, export), "the new setting isn't covered");
    assert!(!second.exists(), "nothing was written");
    let p = prompt(&st);
    assert_eq!(p.held.as_deref(), Some("Create patch"), "the prompt says what waits");
    assert!(p.scan.config.iter().any(|c| c.key == "http.proxy"), "naming the setting: {:?}", p.scan.config);
    // Trust and run exports it, checked again.
    let export = checked_export(trust_and_run(&mut st));
    assert!(recheck(&mut st, export));
    assert!(second.exists(), "written once trusted");
    cleanup(&[&dir, &first, &second]);
}

#[test]
fn a_patch_export_waits_while_the_repository_is_held() {
    let dir = scratch_repo("trust-patch-held");
    std::fs::write(dir.join("p.txt"), "one\n").unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "p"]);
    std::fs::write(dir.join("p.txt"), "two\n").unwrap();
    git(&dir, &["config", "lfs.customtransfer.agent.path", "./bin/agent"]);
    let mut st = state();
    open(&mut st, &dir);
    assert!(asking(&st), "a git-lfs transfer agent runs a program");
    st.modal = None;
    let out = dest(&dir, "held");
    let fx = sealed(&mut st, save_to(&dir, &out));
    assert!(!fx.iter().any(|e| matches!(e, Effect::Checked(_) | Effect::Patch(_))), "nothing runs yet: {fx:?}");
    assert_eq!(prompt(&st).held.as_deref(), Some("Create patch"));
    let export = checked_export(trust_and_run(&mut st));
    assert!(recheck(&mut st, export));
    assert!(out.exists());
    cleanup(&[&dir, &out]);
}
