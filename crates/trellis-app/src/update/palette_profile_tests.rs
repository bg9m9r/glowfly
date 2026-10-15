//! Headless tests for switching profiles from the palette: when "Switch profile…" is offered,
//! the profile sub-list (order, details, matching an email) and running the chosen profile.

use super::tests::state;
use super::update;
use crate::effect::Effect;
use crate::msg::{Msg, PaletteMsg};
use crate::profile::ProfileEffect;
use crate::state::AppState;
use crate::state::palette::PaletteCommand as C;
use crate::state::palette::{Mode, Pick, PickKind, entries, row_count};
use std::time::Instant;
use trellis_core::settings::{Profile, ProfileId};

const SWITCH: &str = "Switch profile…";

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn add(st: &mut AppState, id: u32, name: &str, email: &str) {
    st.settings.profiles.push(Profile {
        id: ProfileId(id),
        name: name.into(),
        author_name: "Ada".into(),
        author_email: email.into(),
        signing_key: None,
    });
}

/// "Work" (current) and "Personal", with no repository open.
fn two() -> AppState {
    let mut st = state();
    add(&mut st, 1, "Work", "ada@work.example");
    add(&mut st, 2, "Personal", "ada@home.example");
    st.settings.active_profile = Some(ProfileId(1));
    st
}

fn switch_entry(st: &AppState) -> Option<Option<String>> {
    entries(st).into_iter().find(|e| e.label == SWITCH).map(|e| e.detail)
}

fn pick(st: &AppState) -> &Pick {
    match st.palette.as_ref().map(|p| &p.mode) {
        Some(Mode::Pick(p)) => p,
        _ => panic!("the palette is not in a sub-list"),
    }
}

#[test]
fn offered_with_two_profiles_and_no_switch_running() {
    let mut st = state();
    assert_eq!(switch_entry(&st), None, "no profiles");
    add(&mut st, 1, "Work", "ada@work.example");
    st.settings.active_profile = Some(ProfileId(1));
    assert_eq!(switch_entry(&st), None, "one profile");
    add(&mut st, 2, "Personal", "ada@home.example");
    assert!(st.snap.is_none());
    assert_eq!(switch_entry(&st), Some(Some("current: Work".to_owned())));
    st.profile_switch = Some(ProfileId(2));
    assert_eq!(switch_entry(&st), None, "a switch is running");
    // Nor does the step open then.
    run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::ChooseProfile)));
    assert!(st.palette.is_none());
}

#[test]
fn choosing_a_profile_switches_to_it() {
    let mut st = two();
    run(&mut st, Msg::Palette(PaletteMsg::TogglePalette));
    run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::ChooseProfile)));
    assert_eq!(st.palette_recent.keys(), &["profile-switch"]);
    let pk = pick(&st);
    assert_eq!(pk.kind, PickKind::Profile);
    assert_eq!((pk.label(0), pk.label(1)), ("Work", "Personal"));
    assert_eq!(pk.detail(0).as_deref(), Some("current · ada@work.example"));
    assert_eq!(pk.detail(1).as_deref(), Some("ada@home.example"));

    // The email matches through the detail.
    st.palette.as_mut().unwrap().query = "home.ex".into();
    assert_eq!(row_count(&mut st), 1);
    let pk = pick(&st);
    let cmd = pk.command(pk.ranked().unwrap().hits[0].index).unwrap();
    assert_eq!(cmd, C::SwitchProfile(ProfileId(2)));

    let fx = run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(cmd)));
    assert!(st.palette.is_none());
    assert!(matches!(&fx[..], [Effect::Profile(ProfileEffect::Apply { id: ProfileId(2), .. })]), "{fx:?}");
    assert_eq!(st.profile_switch, Some(ProfileId(2)));
    assert_eq!(st.palette_recent.keys(), &["profile-switch"], "the picked profile isn't remembered");
}
