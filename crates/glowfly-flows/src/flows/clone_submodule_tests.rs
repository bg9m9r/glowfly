//! Headless tests for Clone submodules too: its default from Preferences > General, the flag a
//! plain clone carries, the submodule step of a sparse clone and its own message when it fails,
//! and a clone without them that isn't given them when it opens, also under another name.

use super::path_alias_tests::alias_of;
use super::submodule_tests::{PATH, opened};
use super::tests::{drive, git, state};
use super::update;
use crate::effect::{CloneEffect, Effect};
use crate::msg::{CloneMsg, Msg};
use crate::state::clone::CloneRequest;
use crate::state::{AppState, Modal};
use glowfly_core::clone_ops::{CloneOptions, CloneOutcome, Sparse, sparse_submodule_cmds};
use glowfly_core::repo::SubmoduleState;
use std::path::PathBuf;
use std::time::Instant;

const URL: &str = "https://example.com/team/app.git";

fn work() -> &'static str {
    if cfg!(windows) { "C:/work" } else { "/work" }
}

fn request(sparse: Option<Sparse>, submodules: bool) -> CloneRequest {
    CloneRequest { url: URL.into(), parent: work().into(), name: "app".into(), options: CloneOptions { shallow: None, sparse, submodules } }
}

fn dialog_default(st: &mut AppState) -> bool {
    update(st, Msg::Clone(CloneMsg::Dialog), Instant::now());
    let Some(Modal::Clone(form)) = &st.modal else { panic!("the Clone dialog is open") };
    form.submodules
}

#[test]
fn the_box_starts_as_preferences_keep_submodules_updated() {
    let mut st = state();
    assert!(!dialog_default(&mut st));
    st.settings.submodules.keep_updated = true;
    assert!(dialog_default(&mut st));
    let Some(Modal::Clone(form)) = &mut st.modal else { unreachable!() };
    form.url = URL.into();
    assert!(form.request(chrono::NaiveDate::from_ymd_opt(2026, 2, 1).unwrap()).unwrap().options.submodules);
    form.submodules = false;
    assert!(!form.request(chrono::NaiveDate::from_ymd_opt(2026, 2, 1).unwrap()).unwrap().options.submodules, "per clone");
}

#[test]
fn a_clone_with_submodules_carries_the_flag() {
    let mut st = state();
    let fx = update(&mut st, Msg::Clone(CloneMsg::Start(request(None, true))), Instant::now());
    match fx.as_slice() {
        [Effect::RunGit { cmds, .. }] => {
            assert_eq!(cmds, &[["clone", "--recurse-submodules", "--", URL, "app"].map(String::from).to_vec()])
        }
        other => panic!("{other:?}"),
    }
    assert!(st.submodules.offered.is_empty(), "nothing to keep from it");

    let mut st = state();
    let fx = update(&mut st, Msg::Clone(CloneMsg::Start(request(None, false))), Instant::now());
    assert!(matches!(fx.as_slice(), [Effect::RunGit { cmds, .. }] if !cmds[0].iter().any(|a| a == "--recurse-submodules")), "{fx:?}");
    assert_eq!(st.submodules.offered, [std::path::PathBuf::from(work()).join("app")]);
}

#[test]
fn a_sparse_clone_updates_its_folders_submodules_afterwards() {
    let mut st = state();
    let sparse = Some(Sparse { folders: vec!["docs".into()], partial: true });
    let fx = update(&mut st, Msg::Clone(CloneMsg::Start(request(sparse.clone(), true))), Instant::now());
    let [Effect::Clone(CloneEffect::Sparse { cmd, submodules, .. })] = fx.as_slice() else { panic!("{fx:?}") };
    assert!(!cmd.iter().any(|a| a == "--recurse-submodules"), "{cmd:?}");
    assert_eq!(submodules, &sparse_submodule_cmds(&["docs".to_owned()]));

    let mut st = state();
    let fx = update(&mut st, Msg::Clone(CloneMsg::Start(request(sparse, false))), Instant::now());
    assert!(matches!(fx.as_slice(), [Effect::Clone(CloneEffect::Sparse { submodules, .. })] if submodules.is_empty()), "{fx:?}");
}

/// Clones a repository with a submodule at `PATH` with `options` into a new folder, reached
/// through a link to it when `alias`, keeping submodules updated. The submodule's URL names a
/// folder that doesn't exist, so cloning it fails whatever git's protocol settings allow. Returns
/// the state and the folders to clean up.
fn clone_with_a_submodule(name: &str, options: CloneOptions, alias: bool) -> (AppState, Vec<PathBuf>) {
    let (_, src, lib) = opened(name);
    git(&src, &["config", "-f", ".gitmodules", &format!("submodule.{PATH}.url"), &format!("../{name}-gone")]);
    git(&src, &["commit", "-q", "-am", "the library moved"]);
    let parent = src.with_file_name(format!("{}-into", src.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&parent);
    std::fs::create_dir_all(&parent).unwrap();
    let mut dirs = vec![src.clone(), lib, parent.clone()];
    let into = if alias {
        let link = alias_of(&parent).expect("made a link");
        dirs.push(link.clone());
        link
    } else {
        parent
    };
    let mut st = state();
    st.settings.submodules.keep_updated = true;
    let req = CloneRequest { url: src.display().to_string(), parent: into, name: "copy".into(), options };
    drive(&mut st, Msg::Clone(CloneMsg::Start(req)));
    (st, dirs)
}

fn log_texts(st: &AppState) -> Vec<&str> {
    st.log.iter().map(|l| l.text.as_str()).collect()
}

/// The clone opened with its submodule left as it is: not initialized, nor offered.
fn left_alone(st: &AppState) {
    let snap = st.snap.as_ref().expect("the clone is open");
    let sm = snap.submodules.iter().find(|s| s.path == PATH).expect("the submodule is listed");
    assert_eq!(sm.state, SubmoduleState::Uninitialized, "{:?}", log_texts(st));
    assert!(!log_texts(st).iter().any(|l| l.contains("Initialize submodules")), "{:?}", log_texts(st));
    assert!(!st.toasts.iter().any(|t| t.action.as_ref().is_some_and(|a| a.label == "Initialize")), "not offered either");
}

fn remove(dirs: &[PathBuf]) {
    for d in dirs {
        let _ = std::fs::remove_dir(d).or_else(|_| std::fs::remove_file(d));
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn a_clone_without_submodules_is_not_given_them_on_open() {
    let (st, dirs) = clone_with_a_submodule("clone-no-subs", CloneOptions::default(), false);
    left_alone(&st);
    remove(&dirs);
}

#[test]
fn a_clone_without_submodules_made_through_a_link_is_not_given_them() {
    let (st, dirs) = clone_with_a_submodule("clone-no-subs-alias", CloneOptions::default(), true);
    let snap = st.snap.as_ref().expect("the clone is open");
    assert!(!glowfly_core::paths::same_spelling(&snap.real_workdir, &dirs[3].join("copy")), "opened by its other name");
    left_alone(&st);
    remove(&dirs);
}

#[test]
fn a_sparse_clone_whose_submodules_failed_says_so() {
    let sparse = Some(Sparse { folders: vec!["libs".into()], partial: false });
    let (st, dirs) = clone_with_a_submodule("clone-subs-refused", CloneOptions { sparse, submodules: true, shallow: None }, false);
    // Keeping submodules updated, the open doesn't try them again: one toast says what failed.
    let start = "Cloned copy and set its folders, but cloning its submodules failed: ";
    let texts: Vec<&str> = st.toasts.iter().map(|t| t.text.as_str()).collect();
    let toast = st.toasts.iter().find(|t| t.text.starts_with(start)).unwrap_or_else(|| panic!("{texts:?}"));
    assert!(!toast.ok);
    assert_eq!(st.toasts.iter().filter(|t| !t.ok).count(), 1, "{texts:?}");
    assert!(!log_texts(&st).iter().any(|l| l.contains("Initialize submodules")), "{:?}", log_texts(&st));
    assert!(toast.text.ends_with("Try again with Update all submodules in the SUBMODULES section."), "{}", toast.text);
    assert!(st.snap.as_ref().is_some_and(|s| s.workdir.ends_with("copy")), "the clone still opens");
    remove(&dirs);
}

#[test]
fn a_submodule_failure_names_the_submodules_not_the_folders() {
    let mut st = state();
    update(
        &mut st,
        Msg::Clone(CloneMsg::Start(request(Some(Sparse { folders: vec!["docs".into()], partial: false }), true))),
        Instant::now(),
    );
    let output = "$ git clone x\n$ git sparse-checkout set\n$ git submodule update --init\nfatal: repository 'https://example.com/lib.git/' not found";
    let o = CloneOutcome { cloned: true, folders_set: true, ok: false, output: output.into() };
    update(&mut st, Msg::Clone(CloneMsg::Finished(o)), Instant::now());
    let text = &st.toasts.last().unwrap().text;
    assert!(text.starts_with("Cloned app and set its folders, but cloning its submodules failed: "), "{text}");
    assert!(!text.contains("Sparse checkout"), "{text}");
}
