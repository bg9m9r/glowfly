//! Headless tests for Preferences' edits (`EditMsg::Prefs`, state/prefs_fields.rs): the fields
//! taken back, a page's error cleared only when its field changed, values not yet read or a
//! form since closed left alone, and the secret fields (`ViewState::secrets`) wiped when
//! Preferences opens or closes.

use super::tests::state;
use super::update;
use crate::effect::Effect;
use crate::msg::{EditMsg, Msg, PrefsMsg};
use crate::state::prefs::GitConfigValues;
use crate::state::ssh::service::{UploadForm, UploadState};
use crate::state::{AppState, PrefPage, PrefsState};
use glowfly_core::forge::{Account, Provider};
use std::path::PathBuf;
use std::time::Instant;

fn edit(st: &mut AppState, e: EditMsg) -> Vec<Effect> {
    update(st, Msg::Edit(e), Instant::now())
}

fn open(page: PrefPage) -> AppState {
    let mut st = state();
    st.prefs = Some(PrefsState::new(page));
    st
}

fn prefs(st: &AppState) -> &PrefsState {
    st.prefs.as_ref().expect("Preferences open")
}

/// Sends Preferences' fields after `f` changed them.
fn typed(st: &mut AppState, f: impl FnOnce(&mut crate::state::prefs_fields::PrefsFields)) {
    let mut fields = prefs(st).fields();
    f(&mut fields);
    edit(st, EditMsg::Prefs(Box::new(fields)));
}

#[test]
fn the_fields_come_back_and_a_closed_window_drops_them() {
    let mut st = open(PrefPage::General);
    typed(&mut st, |f| {
        f.search = "sign".into();
        f.int_host = "git.example.com".into();
        f.keys_filter = "push".into();
        f.profile.author_name = "Ada".into();
    });
    let p = prefs(&st);
    assert_eq!((p.search.as_str(), p.int_host.as_str(), p.keys.filter.as_str()), ("sign", "git.example.com", "push"));
    assert_eq!(p.profiles.author_name, "Ada");

    let fields = prefs(&st).fields();
    st.prefs = None;
    edit(&mut st, EditMsg::Prefs(Box::new(fields)));
    assert!(st.prefs.is_none());
}

#[test]
fn a_page_error_clears_only_when_its_field_changed() {
    let mut st = open(PrefPage::General);
    let p = st.prefs.as_mut().unwrap();
    p.lfs.pattern_error = Some("bad pattern".into());
    p.gitflow.error = Some("bad prefix".into());
    p.sparse.paths_error = Some("bad path".into());
    typed(&mut st, |f| f.search = "lfs".into());
    let p = prefs(&st);
    assert!(p.lfs.pattern_error.is_some() && p.gitflow.error.is_some() && p.sparse.paths_error.is_some(), "nothing they show changed");

    typed(&mut st, |f| f.lfs_pattern = "*.psd".into());
    let p = prefs(&st);
    assert_eq!((p.lfs.new_pattern.as_str(), p.lfs.pattern_error.as_deref()), ("*.psd", None));
    assert!(p.gitflow.error.is_some());

    typed(&mut st, |f| f.gitflow.feature = "feat/".into());
    assert_eq!(prefs(&st).gitflow.error, None);
    assert_eq!(prefs(&st).gitflow.form.feature, "feat/");

    typed(&mut st, |f| f.sparse_paths = "src\n".into());
    assert_eq!((prefs(&st).sparse.paths.as_str(), prefs(&st).sparse.paths_error.as_deref()), ("src\n", None));
}

#[test]
fn git_values_and_the_upload_title_land_only_where_they_exist() {
    let mut st = open(PrefPage::General);
    typed(&mut st, |f| {
        f.git = Some(GitConfigValues { user_name: "Ada".into(), ..Default::default() });
        f.ssh_upload_title = Some("laptop".into());
    });
    assert!(prefs(&st).git.is_none(), "not read yet: a value can't be invented");
    assert!(prefs(&st).ssh.upload.is_none());

    let p = st.prefs.as_mut().unwrap();
    p.git = Some(GitConfigValues::default());
    p.ssh.upload = Some(UploadForm {
        key: PathBuf::from("id_ed25519.pub"),
        account: Account { provider: Provider::GitHub, host: "github.com".into(), org: String::new() },
        title: String::new(),
        default_title: "Glowfly".into(),
        state: UploadState::Editing,
    });
    typed(&mut st, |f| {
        f.git.as_mut().unwrap().user_name = "Ada".into();
        f.ssh_upload_title = Some("laptop".into());
    });
    assert_eq!(prefs(&st).git.as_ref().map(|g| g.user_name.as_str()), Some("Ada"));
    assert_eq!(prefs(&st).ssh.upload.as_ref().map(|u| u.title.as_str()), Some("laptop"));
}

#[test]
fn the_secrets_are_wiped_when_preferences_opens_or_closes() {
    let mut st = state();
    let fill = |st: &mut AppState| {
        let s = &mut st.view.secrets;
        s.int_token = "tok".into();
        s.signing_passphrase = "gpg".into();
        s.ssh_passphrase = "one".into();
        s.ssh_confirm = "one".into();
    };
    let wiped = |st: &AppState| {
        let s = &st.view.secrets;
        s.int_token.is_empty() && s.signing_passphrase.is_empty() && s.ssh_passphrase.is_empty() && s.ssh_confirm.is_empty()
    };
    fill(&mut st);
    update(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Integrations)), Instant::now());
    assert!(wiped(&st), "opening starts empty");
    fill(&mut st);
    assert!(!st.view.secrets.ssh_mismatch());
    update(&mut st, Msg::Prefs(PrefsMsg::ClosePrefs), Instant::now());
    assert!(st.prefs.is_none());
    assert!(wiped(&st), "closing forgets them");
}
