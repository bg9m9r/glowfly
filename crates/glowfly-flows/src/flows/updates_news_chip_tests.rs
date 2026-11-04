//! Headless tests of the What's new chip (the default after an upgrade): put up instead of the
//! dialog, opening the notes with the new releases marked and going, running out after
//! `NEWS_CHIP_FOR`, hidden by the other choices, and kept when another copy's file is read.

use super::update;
use super::updates_notes_tests::{NOTES, dialog, saves};
use super::updates_tests::{NOW, app, send};
use glowfly_core::app_update::Version;
use glowfly_core::i18n::Lang;
use glowfly_core::settings::{NewsChip, Settings, WhatsNew};
use glowfly_model::msg::{Msg, UpdateMsg as U};
use glowfly_model::state::AppState;
use glowfly_model::state::release_notes::{NEWS_CHIP_FOR, news_chip_text};
use glowfly_model::state::settings_file::carry_records;
use std::time::Instant;

/// 0.2.0 starting after 0.1.0, with the default choice.
fn upgraded() -> AppState {
    let mut st = app();
    st.settings.updates.notes_seen = Some("0.1.0".into());
    st.updates.bundled_notes = NOTES;
    st
}

fn tick(st: &mut AppState) -> Vec<glowfly_model::effect::Effect> {
    let t = Instant::now();
    update(st, Msg::Tick(t), t)
}

#[test]
fn an_upgrade_puts_up_the_chip_instead_of_the_dialog() {
    let mut st = upgraded();
    assert_eq!(st.settings.updates.whats_new, WhatsNew::Chip, "the default");
    let fx = send(&mut st, U::Startup, Instant::now());
    assert!(dialog(&st).is_none() && st.updates.notes_pending.is_none(), "nothing opens by itself");
    let chip = st.updates_news_chip().expect("the chip");
    assert_eq!((chip.version.as_str(), chip.since.as_deref()), ("0.2.0", Some("0.1.0")));
    assert_eq!(news_chip_text(Lang::En, &chip.version), "What's new in 0.2.0");
    assert_eq!(st.settings.updates.notes_seen.as_deref(), Some("0.2.0"));
    assert!(saves(&fx), "the chip outlasts a restart");
    // The next start keeps it as it is.
    let fx = send(&mut st, U::Startup, Instant::now());
    assert!(!saves(&fx) && st.updates_news_chip().is_some() && dialog(&st).is_none());
}

#[test]
fn the_chip_opens_the_notes_with_the_new_releases_and_goes() {
    let mut st = upgraded();
    send(&mut st, U::Startup, Instant::now());
    let fx = send(&mut st, U::ShowNotes, Instant::now());
    let d = dialog(&st).expect("the notes open");
    assert_eq!(d.since, Version::parse("0.1.0"), "what came after 0.1.0 is marked new");
    assert_eq!(d.releases.iter().map(|r| d.is_new(r)).collect::<Vec<_>>(), [true, true, false]);
    assert!(st.updates_news_chip().is_none() && st.settings.updates.news_chip.is_none(), "clicked: gone");
    assert!(saves(&fx));
    // Opened again later (Help or the palette): nothing is marked any more.
    st.modal = None;
    let fx = send(&mut st, U::ShowNotes, Instant::now());
    assert!(dialog(&st).is_some_and(|d| d.since.is_none()) && !saves(&fx));
}

#[test]
fn the_chip_runs_out_after_its_days() {
    let mut st = upgraded();
    send(&mut st, U::Startup, Instant::now());
    // The first frame that knows the time starts the chip's days.
    assert!(saves(&tick(&mut st)));
    assert_eq!(st.settings.updates.news_chip.as_ref().and_then(|c| c.until), Some(NOW + NEWS_CHIP_FOR));
    assert!(!saves(&tick(&mut st)), "once");
    st.view.now = NOW + NEWS_CHIP_FOR - 1;
    assert!(!saves(&tick(&mut st)) && st.updates_news_chip().is_some(), "still there a second before");
    st.view.now = NOW + NEWS_CHIP_FOR;
    assert!(st.updates_news_chip().is_none(), "hidden once the time is up");
    assert!(saves(&tick(&mut st)) && st.settings.updates.news_chip.is_none(), "and dropped from the settings");
}

#[test]
fn before_the_clock_is_known_the_chip_waits_to_start_its_days() {
    let mut st = upgraded();
    st.view.now = 0;
    send(&mut st, U::Startup, Instant::now());
    assert!(!saves(&tick(&mut st)));
    assert!(st.updates_news_chip().is_some_and(|c| c.until.is_none()), "shown, its days not started");
}

#[test]
fn the_dialog_choice_opens_the_notes_and_off_shows_nothing() {
    let mut st = upgraded();
    st.settings.updates.whats_new = WhatsNew::Dialog;
    send(&mut st, U::Startup, Instant::now());
    assert!(dialog(&st).is_some() && st.settings.updates.news_chip.is_none());

    let mut st = upgraded();
    st.settings.updates.whats_new = WhatsNew::Off;
    let fx = send(&mut st, U::Startup, Instant::now());
    assert!(dialog(&st).is_none() && st.updates.notes_pending.is_none() && st.settings.updates.news_chip.is_none());
    assert_eq!(st.settings.updates.notes_seen.as_deref(), Some("0.2.0"), "recorded");
    assert!(saves(&fx));
}

#[test]
fn choosing_another_way_hides_a_waiting_chip() {
    let mut st = upgraded();
    send(&mut st, U::Startup, Instant::now());
    st.settings.updates.whats_new = WhatsNew::Off;
    assert!(st.updates_news_chip().is_none(), "Off hides it");
    st.settings.updates.whats_new = WhatsNew::Chip;
    assert!(st.updates_news_chip().is_some(), "back to the chip within its days: shown again");
}

#[test]
fn a_new_install_or_a_version_without_notes_gets_no_chip() {
    let mut st = upgraded();
    st.settings.welcome_done = false;
    update(&mut st, Msg::Onboarding(glowfly_model::onboarding::OnboardingMsg::Startup), Instant::now());
    send(&mut st, U::Startup, Instant::now());
    assert!(st.settings.updates.news_chip.is_none(), "the welcome instead");

    let mut st = upgraded();
    st.updates.current = Version::parse("0.2.1").unwrap();
    send(&mut st, U::Startup, Instant::now());
    assert!(st.settings.updates.news_chip.is_none(), "nothing bundled to show");
}

#[test]
fn a_chip_for_another_version_is_not_shown_and_the_next_upgrade_replaces_it() {
    let mut st = upgraded();
    let older = NewsChip { version: "0.1.5".into(), since: Some("0.1.0".into()), until: None };
    st.settings.updates.news_chip = Some(older);
    st.settings.updates.notes_seen = Some("0.2.0".into());
    assert!(st.updates_news_chip().is_none(), "0.2.0 runs: the 0.1.5 chip has nothing to say");
    st.settings.updates.notes_seen = Some("0.1.5".into());
    send(&mut st, U::Startup, Instant::now());
    let chip = st.updates_news_chip().expect("this version's chip");
    assert_eq!((chip.version.as_str(), chip.since.as_deref()), ("0.2.0", Some("0.1.5")));
}

#[test]
fn a_read_of_another_copys_file_keeps_this_copys_chip() {
    let mut ours = Settings::default();
    ours.updates.notes_seen = Some("0.2.0".into());
    ours.updates.news_chip = Some(NewsChip { version: "0.2.0".into(), since: Some("0.1.0".into()), until: Some(NOW) });
    let mut file = Settings::default();
    file.updates.notes_seen = Some("0.1.0".into());
    assert!(carry_records(&ours, &mut file));
    assert_eq!(file.updates.news_chip, ours.updates.news_chip);
    // Clicked here: the file's copy of the chip goes too.
    ours.updates.news_chip = None;
    assert!(carry_records(&ours, &mut file) && file.updates.news_chip.is_none());
    // A file that has seen newer notes keeps its own.
    file.updates.notes_seen = Some("0.3.0".into());
    file.updates.news_chip = Some(NewsChip { version: "0.3.0".into(), since: Some("0.2.0".into()), until: None });
    assert!(!carry_records(&ours, &mut file) && file.updates.news_chip.is_some());
}
