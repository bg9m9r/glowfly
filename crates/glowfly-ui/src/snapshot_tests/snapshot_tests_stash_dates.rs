//! The stash dates: the STASHES rows (relative and absolute, light, German, a narrow panel, an
//! auto-stash row, the hover text) and the palette's stash sub-list (its detail's tooltip). The
//! made-up stashes of `snapshot_tests_palette_stash` carry times from the pinned `NOW`.

use super::snapshot_tests::{NOW, harness, remove_fixture, with_repo};
use super::snapshot_tests_palette_stash::fixture_stashes;
use super::snapshot_tests_stash::with_stashes;
use crate::snapshot_check::{CheckSnapshot, text_snapshot};
use eframe::egui::{Event, Pos2, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::{SizedPanel, ThemeChoice};
use glowfly_model::msg::Msg;
use glowfly_model::state::AppState;
use glowfly_model::state::palette::{Mode, Palette, Pick};
use glowfly_model::state::stash_confirm::StashAction;

type H = Harness<'static, (AppState, Vec<Msg>)>;

/// The fixture stashes' messages, in list order.
const FIRST: &str = "On main: login form tweaks";
const SECOND: &str = "WIP on main: 1a2b3c4 Add search";
const THIRD: &str = "On login-page: half-done validation";
const AUTO: &str = "On main: Auto-stash before checking out login-page";

/// Rests the pointer at `at` and lets the tooltip come up.
fn hover(h: &mut H, at: Pos2) {
    h.input_mut().events.push(Event::PointerMoved(at));
    for _ in 0..4 {
        h.step();
    }
}

/// The middle of the (leftmost) row named `label`.
fn row_centre(h: &H, label: &str) -> Pos2 {
    let b = h.get_all_by_label(label).filter_map(|n| n.bounding_box()).min_by(|a, b| a.x0.total_cmp(&b.x0)).expect("the row");
    pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32)
}

/// Whether some label (a tooltip) holds `text`.
fn has_tip(h: &H, text: &str) -> bool {
    h.query_by_label_contains(text).is_some()
}

#[test]
fn stash_dates() {
    let (st, dir) = with_stashes(ThemeChoice::Dark, "stash-dates");
    let time = st.snap.as_ref().unwrap().stashes[1].time.unwrap();
    let tip = st.settings.date_tooltip(time, NOW, 0);
    let mut h = harness(st, true);
    h.run();
    let painted = text_snapshot(&h);
    for date in ["2 hours ago", "3 days ago", "1 month ago"] {
        assert!(painted.contains(&format!("{date:?}")), "{date} is drawn:\n{painted}");
    }
    for message in [FIRST, SECOND, THIRD] {
        assert!(h.query_by_label(message).is_some(), "{message} names its row");
    }
    h.check_snapshot("stash_dates");
    let at = row_centre(&h, SECOND);
    hover(&mut h, at);
    assert!(has_tip(&h, &tip), "the row's tooltip has the full date: {tip:?}");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn stash_dates_auto_stash_row() {
    let (mut st, dir) = with_stashes(ThemeChoice::Dark, "stash-dates-auto");
    st.snap.as_mut().unwrap().stashes[0].message = AUTO.into();
    let mut h = harness(st, true);
    h.run();
    let painted = text_snapshot(&h);
    assert!(painted.contains("\"auto\"") && painted.contains("\"2 hours ago\""), "{painted}");
    h.check_snapshot("stash_dates_auto");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn stash_dates_narrow() {
    let (mut st, dir) = with_stashes(ThemeChoice::Dark, "stash-dates-narrow");
    st.snap.as_mut().unwrap().stashes[0].message = AUTO.into();
    let time = st.snap.as_ref().unwrap().stashes[1].time.unwrap();
    let tip = st.settings.date_tooltip(time, NOW, 0);
    st.settings.panel_sizes.set(SizedPanel::Sidebar, 200.0);
    let mut h = harness(st, true);
    h.run();
    let painted = text_snapshot(&h);
    assert!(painted.contains("\"auto\""), "auto stays:\n{painted}");
    assert!(!painted.contains(" ago\""), "no date is drawn under 240 px:\n{painted}");
    h.check_snapshot("stash_dates_narrow");
    let at = row_centre(&h, SECOND);
    hover(&mut h, at);
    assert!(has_tip(&h, &tip), "the date is still on hover: {tip:?}");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn stash_dates_absolute_light() {
    let (mut st, dir) = with_stashes(ThemeChoice::Light, "stash-dates-absolute");
    st.settings.relative_dates = false;
    let times: Vec<i64> = st.snap.as_ref().unwrap().stashes.iter().filter_map(|s| s.time).collect();
    let dates: Vec<String> = times.iter().map(|&t| st.settings.format_date(t, 0)).collect();
    let tip = st.settings.date_tooltip(times[1], NOW, 0);
    let mut h = harness(st, false);
    h.run();
    let painted = text_snapshot(&h);
    for date in &dates {
        assert!(painted.contains(&format!("{date:?}")), "{date} is drawn:\n{painted}");
    }
    h.check_snapshot("stash_dates_absolute_light");
    let at = row_centre(&h, SECOND);
    hover(&mut h, at);
    assert_eq!(tip.lines().nth(1), Some("3 days ago"));
    assert!(has_tip(&h, &tip), "the full date, then the age: {tip:?}");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn stash_dates_german() {
    let (mut st, dir) = with_stashes(ThemeChoice::Dark, "stash-dates-german");
    st.settings.language = "de".into();
    let mut h = harness(st, true);
    h.run();
    let painted = text_snapshot(&h);
    for date in ["vor 2 Stunden", "vor 3 Tagen", "vor 1 Monat"] {
        assert!(painted.contains(&format!("{date:?}")), "{date} is drawn whole:\n{painted}");
    }
    for message in [FIRST, SECOND, THIRD] {
        assert!(h.query_by_label(message).is_some(), "{message} names its row");
    }
    h.check_snapshot("stash_dates_de");
    drop(h);
    remove_fixture(&dir);
}

/// The Apply sub-list over the fixture stashes, with its first row selected.
fn apply_list(st: &AppState) -> Palette {
    let mut pick = Pick::stashes(StashAction::Apply, &fixture_stashes(st));
    pick.refresh("");
    Palette { selected: 0, mode: Mode::Pick(pick), ..Default::default() }
}

#[test]
fn palette_stash_dates_absolute() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "palette-stash-dates");
    st.settings.relative_dates = false;
    st.palette = Some(apply_list(&st));
    let shown: Vec<String> = fixture_stashes(&st)
        .iter()
        .enumerate()
        .map(|(n, s)| format!("{} · stash@{{{n}}}", st.settings.format_date(s.time.unwrap(), 0)))
        .collect();
    let mut h = harness(st, true);
    h.run();
    let painted = text_snapshot(&h);
    for d in &shown {
        assert!(painted.contains(&format!("{d:?}")), "{d} is drawn:\n{painted}");
    }
    h.check_snapshot("palette_stash_dates_absolute");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn palette_stash_date_tooltip() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "palette-stash-date-tip");
    let tip = st.settings.date_tooltip(fixture_stashes(&st)[1].time.unwrap(), NOW, 0);
    st.palette = Some(apply_list(&st));
    let mut h = harness(st, true);
    h.run();
    let row = h.get_by_label(SECOND).bounding_box().expect("the row has bounds");
    let y = ((row.y0 + row.y1) / 2.0) as f32;
    hover(&mut h, pos2(row.x0 as f32 + 60.0, y));
    assert!(!has_tip(&h, &tip), "no tooltip over the label");
    hover(&mut h, pos2(row.x1 as f32 - 40.0, y));
    assert!(has_tip(&h, &tip), "the detail shows the full date: {tip:?}");
    drop(h);
    remove_fixture(&dir);
}
