//! Headless tests for Preferences > SSH's Use existing and Remove from list: the file dialog's
//! effect, a picked pair listed and remembered, a picked problem shown, a remembered key
//! forgotten, and the remembered keys passed to the listing.

use super::ssh_tests::{home, listing, loaded, page, page_mut, pair, run, ssh};
use super::tests::state;
use crate::effect::Effect;
use crate::msg::{Msg, PrefsMsg};
use crate::ssh::{SshEffect, SshMsg};
use crate::state::PrefPage;
use std::path::PathBuf;
use trellis_core::ssh_keys::{KeyFileProblem, SshKeyPair};

/// A pair outside the .ssh folder.
fn outside(name: &str) -> SshKeyPair {
    let dir = home().join("keys");
    SshKeyPair { private: dir.join(name), public: dir.join(format!("{name}.pub")), ..pair(name, "") }
}

fn picked(pair: &SshKeyPair) -> SshMsg {
    SshMsg::KeyPicked(Some((pair.private.clone(), Ok(pair.clone()))))
}

fn saves(fx: &[Effect]) -> usize {
    fx.iter().filter(|e| matches!(e, Effect::SaveSettings(_))).count()
}

fn names(st: &crate::state::AppState) -> Vec<String> {
    page(st).listing.as_ref().expect("listed").keys.iter().map(SshKeyPair::file_name).collect()
}

#[test]
fn use_existing_opens_the_picker() {
    let mut st = state();
    run(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Ssh)));
    assert!(ssh(&mut st, SshMsg::UseExisting).is_empty(), "not before the listing arrives");

    let mut st = loaded(vec![pair("id_ed25519", "")], true);
    let fx = ssh(&mut st, SshMsg::UseExisting);
    let [Effect::Ssh(SshEffect::PickKey { title, start, keygen })] = &fx[..] else { panic!("one PickKey, got {fx:?}") };
    assert_eq!(start, &home().join(".ssh"));
    assert!(keygen.is_some());
    assert_eq!(title, "Choose a private key");
    assert!(Effect::Ssh(SshEffect::PickKey { title: String::new(), start: home(), keygen: None }).scope().is_none(), "unscoped");
}

#[test]
fn picked_key_is_listed_and_remembered() {
    let mut st = loaded(vec![pair("id_ed25519", "")], true);
    let work = outside("work");
    let fx = ssh(&mut st, picked(&work));
    assert_eq!(names(&st), ["id_ed25519", "work"]);
    assert_eq!(st.settings.ssh_extra_keys, std::slice::from_ref(&work.private));
    assert_eq!(saves(&fx), 1);
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Added work to the list"));

    let fx = ssh(&mut st, picked(&work));
    assert_eq!(saves(&fx), 0, "the settings are left alone");
    assert_eq!(st.settings.ssh_extra_keys.len(), 1);
    assert_eq!(names(&st).len(), 2);
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("work is already in the list"));

    // A key in the .ssh folder is listed already.
    let fx = ssh(&mut st, picked(&pair("id_ed25519", "")));
    assert!(fx.is_empty());
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("id_ed25519 is already in the list"));

    let toasts = st.toasts.len();
    assert!(ssh(&mut st, SshMsg::KeyPicked(None)).is_empty(), "cancelled");
    assert_eq!((st.toasts.len(), names(&st).len()), (toasts, 2));
}

#[test]
fn picked_problem_is_shown() {
    let mut st = loaded(vec![], true);
    let lone = home().join("keys").join("lone");
    ssh(&mut st, SshMsg::KeyPicked(Some((lone, Err(KeyFileProblem::NoPublicKey)))));
    assert_eq!(page(&st).pick_error, Some(("lone".into(), KeyFileProblem::NoPublicKey)));
    assert!(st.settings.ssh_extra_keys.is_empty());

    ssh(&mut st, picked(&outside("work")));
    assert_eq!(page(&st).pick_error, None, "a later pick clears it");
}

#[test]
fn forget_key_drops_an_extra() {
    let mut st = loaded(vec![pair("id_ed25519", "")], true);
    let work = outside("work");
    ssh(&mut st, picked(&work));
    let fx = ssh(&mut st, SshMsg::ForgetKey(work.private.clone()));
    assert_eq!(names(&st), ["id_ed25519"]);
    assert!(st.settings.ssh_extra_keys.is_empty());
    assert_eq!(saves(&fx), 1);

    let fx = ssh(&mut st, SshMsg::ForgetKey(home().join(".ssh").join("id_ed25519")));
    assert!(fx.is_empty(), "a key in the .ssh folder isn't forgotten");
    assert_eq!(names(&st), ["id_ed25519"]);
}

#[test]
fn load_passes_extra_keys() {
    let mut st = state();
    let extra = home().join("keys").join("deploy");
    st.settings.ssh_extra_keys = vec![extra.clone()];
    let fx = run(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Ssh)));
    let lists: Vec<&Vec<PathBuf>> = fx
        .iter()
        .filter_map(|e| match e {
            Effect::Ssh(SshEffect::List { extra, .. }) => Some(extra),
            _ => None,
        })
        .collect();
    assert_eq!(lists, [&vec![extra]]);
    // A remembered key whose files have gone is skipped by the listing itself (core), so the
    // page shows what it gets.
    let generation = page(&st).generation;
    ssh(&mut st, SshMsg::Loaded { generation, result: Ok(listing(vec![], true)) });
    assert!(page_mut(&mut st).listing.as_ref().is_some_and(|l| l.keys.is_empty()));
}
