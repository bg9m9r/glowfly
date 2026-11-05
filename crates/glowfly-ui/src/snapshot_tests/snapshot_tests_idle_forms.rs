//! The dialogs, Preferences' own fields and the command palette, drawn from a frame copy, send
//! `Msg::Edit` only when something changed: an idle frame sends none, and typing sends one that
//! lands on the model. Covers a form dialog (New tag), the danger confirmation with its box, the
//! auto-stash question, Generate SSH key over its page, every Preferences page and the palette's
//! command list and sub-list. The panels' drafts are in `snapshot_tests_idle_panels`.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::snapshot_tests_idle_edits::{idle, typing_edits};
use super::snapshot_tests_ssh::{HOME, listing, ssh_page, two_keys};
use crate::snapshot_fixtures::edits_taken;
use glowfly_core::settings::ThemeChoice;
use glowfly_core::stash_ops::AutoStashFor;
use glowfly_model::msg::Msg;
use glowfly_model::state::auto_stash::AutoStashAsk;
use glowfly_model::state::palette::{Mode, Palette, Pick};
use glowfly_model::state::tags::TagForm;
use glowfly_model::state::{Modal, PrefPage, PrefsState};

#[test]
fn a_form_dialog_idles_and_takes_what_was_typed() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "idle-new-tag");
    let at = st.snap.as_ref().and_then(|s| s.head_oid).expect("born HEAD").to_string();
    st.modal = Some(Modal::NewTag(TagForm { at, name: "v1.0".into(), ..TagForm::default() }));
    let mut h = idle(st, "New tag");
    typing_edits(&mut h, "v1.0");
    assert!(matches!(&h.state().0.modal, Some(Modal::NewTag(f)) if f.name == "v1.0x"), "the form took the text");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn the_questions_idle_without_edits() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "idle-questions");
    st.modal = Some(Modal::danger("Discard changes", "Delete notes.txt?", "Discard", Msg::ClearLog).with_ack("I understand"));
    drop(idle(st, "danger confirmation"));
    remove_fixture(&dir);
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "idle-auto-stash");
    let ask = AutoStashAsk { op: AutoStashFor::Pull { branch: "main".into() }, files: 2, always: true, retry: Box::new(Msg::CloseModal) };
    st.modal = Some(Modal::AutoStash(ask));
    drop(idle(st, "auto-stash question"));
    remove_fixture(&dir);
}

#[test]
fn generate_ssh_key_idles_and_takes_what_was_typed() {
    let mut st = ssh_page(ThemeChoice::Dark, Some(listing(two_keys(), true)));
    let location = format!("{HOME}/.ssh/id_ed25519_laptop");
    st.prefs.as_mut().unwrap().ssh.location = location.clone();
    st.view.secrets.ssh_passphrase = "one".into();
    st.view.secrets.ssh_confirm = "one".into();
    st.modal = Some(Modal::SshKeygen);
    let mut h = idle(st, "Generate SSH key");
    typing_edits(&mut h, &location);
    assert_eq!(h.state().0.prefs.as_ref().unwrap().ssh.location, format!("{location}x"));
}

#[test]
fn every_preferences_page_idles_without_edits() {
    use PrefPage::*;
    let pages = [
        Git,
        Profile,
        Integrations,
        Keyboard,
        Tools,
        Terminal,
        Signing,
        Ssh,
        RepoIdentity,
        RepoHooks,
        RepoTracker,
        RepoLfs,
        RepoGitFlow,
        RepoSparse,
    ];
    for page in pages {
        let (mut st, dir) = with_repo(ThemeChoice::Dark, &format!("idle-page-{page:?}").to_lowercase());
        st.prefs = Some(PrefsState::new(page));
        // Some pages show a spinner while their values load: step frames rather than settle.
        let mut h = harness(st, true);
        h.run_steps(4);
        edits_taken();
        h.run_steps(3);
        assert_eq!(edits_taken(), 0, "{page:?}: an idle frame sent an edit");
        drop(h);
        remove_fixture(&dir);
    }
}

#[test]
fn the_preferences_search_takes_what_was_typed() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "idle-prefs-search");
    let mut p = PrefsState::new(PrefPage::General);
    p.search = "sign".into();
    st.prefs = Some(p);
    let mut h = idle(st, "Preferences search");
    typing_edits(&mut h, "sign");
    assert_eq!(h.state().0.prefs.as_ref().unwrap().search, "signx");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn the_palette_idles_and_takes_the_query() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "idle-palette");
    st.palette = Some(Palette { query: "ch".into(), selected: 1, ..Default::default() });
    let mut h = idle(st, "command list");
    typing_edits(&mut h, "ch");
    let p = h.state().0.palette.as_ref().unwrap();
    assert_eq!((p.query.as_str(), p.selected), ("chx", 0), "a new query starts at the top");
    drop(h);

    remove_fixture(&dir);
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "idle-palette-pick");
    st.palette = Some(Palette { query: "Per".into(), mode: Mode::Pick(Pick::profiles(&st.settings)), ..Default::default() });
    let mut h = idle(st, "profile sub-list, ranked by update");
    typing_edits(&mut h, "Per");
    let Some(Palette { mode: Mode::Pick(pk), query, .. }) = &h.state().0.palette else { panic!("the sub-list") };
    assert_eq!(pk.ranked().map(|r| r.query.as_str()), Some(query.as_str()), "ranked for the typed query");
    drop(h);
    remove_fixture(&dir);
}
