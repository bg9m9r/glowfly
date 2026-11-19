//! The open trust prompt without the runner (update/trust.rs): Show all and Show less on a
//! step row, and "Don't ask again for these places in this repository" on a prompt that names
//! places Glowfly doesn't read.

use super::tests::state;
use super::update;
use crate::msg::{Msg, TrustMsg};
use crate::state::trust::TrustPrompt;
use crate::state::{AppState, Modal};
use std::path::PathBuf;
use std::time::Instant;

fn open_step(st: &AppState) -> Option<usize> {
    match &st.modal {
        Some(Modal::Trust(p)) => p.open_step,
        _ => panic!("the prompt stays open"),
    }
}

#[test]
fn show_all_opens_one_step_and_show_less_closes_it() {
    let mut st = state();
    let folder = PathBuf::from("C:/work/tools");
    st.modal = Some(Modal::Trust(TrustPrompt {
        name: "tools".into(),
        workdir: folder.clone(),
        real: folder,
        scan: Default::default(),
        refused: false,
        held: None,
        gained: None,
        terminal: false,
        incoming: None,
        unchecked: None,
        open_step: None,
        remember: false,
    }));
    let toggle = |st: &mut AppState, i| assert!(update(st, Msg::Trust(TrustMsg::ToggleStep(i)), Instant::now()).is_empty());
    toggle(&mut st, 2);
    assert_eq!(open_step(&st), Some(2));
    toggle(&mut st, 0);
    assert_eq!(open_step(&st), Some(0), "one at a time");
    toggle(&mut st, 0);
    assert_eq!(open_step(&st), None);
    // Without the prompt it does nothing.
    st.modal = None;
    toggle(&mut st, 1);
    assert!(st.modal.is_none());
}

/// A prompt for a held job in `folder`, for `reason`.
fn held_for(folder: &std::path::Path, reason: glowfly_core::trust::incoming::Unchecked) -> Modal {
    Modal::Trust(TrustPrompt {
        name: "tools".into(),
        workdir: folder.to_path_buf(),
        real: folder.to_path_buf(),
        scan: Default::default(),
        refused: false,
        held: Some("Pull".into()),
        gained: None,
        terminal: false,
        incoming: None,
        unchecked: Some(Box::new(crate::effect::Accepted { at: None, reason })),
        open_step: None,
        remember: false,
    })
}

#[test]
fn dont_ask_again_keeps_the_prompt_s_places_with_the_trusted_decision() {
    use glowfly_core::trust::incoming::Unchecked;
    let folder = PathBuf::from("C:/work/tools");
    let places = vec!["//nas/team/hooks".to_owned(), "lib: //nas/team/ca.pem".to_owned()];
    let offsite = |files: Option<&str>| Unchecked::Offsite { places: places.clone(), files: files.map(str::to_owned) };
    let send = |st: &mut AppState, m: TrustMsg| update(st, Msg::Trust(m), Instant::now());
    let ticked = |st: &AppState| matches!(&st.modal, Some(Modal::Trust(p)) if p.remember);
    let decide = |trusted| TrustMsg::Decide { real: folder.clone(), trusted };

    // Only a prompt that names such places takes the tick.
    let mut st = state();
    st.modal = Some(held_for(&folder, Unchecked::State("rebase-merge/git-rebase-todo".into())));
    send(&mut st, TrustMsg::RememberPlaces(true));
    assert!(!ticked(&st), "nothing to remember for unread files");
    send(&mut st, decide(true));
    assert!(st.settings.remembered_places(&folder).is_empty());

    // Unticked, Trust and run remembers nothing; ticked, it keeps exactly the places shown,
    // alone or with unread files beside them.
    for files in [None, Some("rebase-merge/git-rebase-todo")] {
        let mut st = state();
        st.modal = Some(held_for(&folder, offsite(files)));
        send(&mut st, TrustMsg::RememberPlaces(true));
        send(&mut st, TrustMsg::RememberPlaces(false));
        send(&mut st, decide(true));
        assert!(st.settings.remembered_places(&folder).is_empty(), "{files:?}: not ticked");

        st.modal = Some(held_for(&folder, offsite(files)));
        send(&mut st, TrustMsg::RememberPlaces(true));
        assert!(ticked(&st));
        send(&mut st, decide(true));
        assert_eq!(st.settings.remembered_places(&folder), places, "{files:?}");
        assert!(st.modal.is_none());

        // Don't trust ends them, ticked or not; so does revoking.
        st.modal = Some(held_for(&folder, offsite(files)));
        send(&mut st, TrustMsg::RememberPlaces(true));
        send(&mut st, decide(false));
        assert!(st.settings.repo_trust_places.is_empty(), "{files:?}: not trusted");
        st.modal = Some(held_for(&folder, offsite(files)));
        send(&mut st, TrustMsg::RememberPlaces(true));
        send(&mut st, decide(true));
        assert_eq!(st.settings.remembered_places(&folder), places);
        send(&mut st, TrustMsg::Revoke { real: folder.clone() });
        assert!(st.settings.repo_trust_places.is_empty() && st.settings.saved_trust(&folder).is_none(), "{files:?}: revoked");
    }
}
