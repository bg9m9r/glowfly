//! Headless tests for Clone submodules too: its default from Preferences > General, the flag a
//! plain clone carries, the submodule step of a sparse clone, and a clone without them that
//! isn't given them when it opens.

use super::submodule_tests::{PATH, cleanup, opened};
use super::tests::{drive, state};
use super::update;
use crate::effect::{CloneEffect, Effect};
use crate::msg::{CloneMsg, Msg};
use crate::state::clone::CloneRequest;
use crate::state::{AppState, Modal};
use glowfly_core::clone_ops::{CloneOptions, Sparse, sparse_submodule_cmds};
use glowfly_core::repo::SubmoduleState;
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

#[test]
fn a_clone_without_submodules_is_not_given_them_on_open() {
    let (src_st, src, lib) = opened("clone-no-subs");
    drop(src_st);
    let parent = src.with_file_name(format!("{}-into", src.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&parent);
    std::fs::create_dir_all(&parent).unwrap();
    let mut st = state();
    st.settings.submodules.keep_updated = true;
    let req =
        CloneRequest { url: src.display().to_string(), parent: parent.clone(), name: "copy".into(), options: CloneOptions::default() };
    drive(&mut st, Msg::Clone(CloneMsg::Start(req)));
    let snap = st.snap.as_ref().expect("the clone is open");
    let sm = snap.submodules.iter().find(|s| s.path == PATH).expect("the submodule is listed");
    assert_eq!(sm.state, SubmoduleState::Uninitialized, "{:?}", st.log.iter().map(|l| &l.text).collect::<Vec<_>>());
    assert!(!st.log.iter().any(|l| l.text.contains("Initialize submodules")), "{:?}", st.log.iter().map(|l| &l.text).collect::<Vec<_>>());
    assert!(!st.toasts.iter().any(|t| t.action.as_ref().is_some_and(|a| a.label == "Initialize")), "not offered either");
    cleanup(&[&src, &lib, &parent]);
}
