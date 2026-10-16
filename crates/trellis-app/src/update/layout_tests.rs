//! Headless tests for zooming and showing or hiding the panels: steps and limits, the single
//! updating toast, saving only on a change, the palette entries and Go to the commit message
//! bringing the details panel back.

use super::tests::{keys, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::keymap::{BindingId, KeyAction};
use crate::msg::{LayoutMsg, Msg, PaletteMsg};
use crate::state::AppState;
use crate::state::layout::{Panel, shown};
use crate::state::palette::{Entry, PaletteCommand as C, entries};
use std::time::{Duration, Instant};
use trellis_core::repo::{self, LoadOptions};

fn run(st: &mut AppState, msg: LayoutMsg) -> Vec<Effect> {
    update(st, Msg::Layout(msg), Instant::now())
}

fn saves(fx: &[Effect]) -> bool {
    fx.iter().any(|e| matches!(e, Effect::SaveSettings(_)))
}

fn toast_texts(st: &AppState) -> Vec<&str> {
    st.toasts.iter().map(|t| t.text.as_str()).collect()
}

fn find(st: &AppState, label: &str) -> Option<Entry> {
    entries(st).into_iter().find(|e| e.label == label)
}

#[test]
fn zoom_in_steps_saves_and_toasts() {
    let mut st = state();
    let fx = run(&mut st, LayoutMsg::ZoomIn);
    assert_eq!(st.settings.zoom_percent, 110);
    assert!(saves(&fx));
    assert_eq!(toast_texts(&st), ["Zoom 110%"]);
}

#[test]
fn repeated_zoom_updates_one_toast() {
    let mut st = state();
    run(&mut st, LayoutMsg::ZoomIn);
    run(&mut st, LayoutMsg::ZoomIn);
    assert_eq!(st.toasts.len(), 1);
    assert_eq!(toast_texts(&st), ["Zoom 125%"]);
    assert_eq!(st.view.zoom_toast, Some(st.toasts[0].id));
}

#[test]
fn zoom_out_stops_at_smallest() {
    let mut st = state();
    st.settings.zoom_percent = 50;
    let fx = run(&mut st, LayoutMsg::ZoomOut);
    assert_eq!(st.settings.zoom_percent, 50);
    assert!(!saves(&fx));
    assert_eq!(toast_texts(&st), ["Zoom 50% (the smallest)"]);
}

#[test]
fn zoom_in_stops_at_largest() {
    let mut st = state();
    st.settings.zoom_percent = 175;
    assert!(saves(&run(&mut st, LayoutMsg::ZoomIn)));
    assert_eq!(toast_texts(&st), ["Zoom 200% (the largest)"]);
    let fx = run(&mut st, LayoutMsg::ZoomIn);
    assert_eq!(st.settings.zoom_percent, 200);
    assert!(!saves(&fx));
    assert_eq!(toast_texts(&st), ["Zoom 200% (the largest)"]);
}

#[test]
fn reset_returns_to_100_and_saves() {
    let mut st = state();
    st.settings.zoom_percent = 150;
    assert!(saves(&run(&mut st, LayoutMsg::ZoomReset)));
    assert_eq!(st.settings.zoom_percent, 100);
    assert_eq!(toast_texts(&st), ["Zoom 100%"]);
}

#[test]
fn reset_at_100_does_not_save() {
    let mut st = state();
    assert!(!saves(&run(&mut st, LayoutMsg::ZoomReset)));
    assert_eq!(toast_texts(&st), ["Zoom 100%"]);
}

#[test]
fn hand_edited_value_steps_from_raw() {
    let mut st = state();
    st.settings.zoom_percent = 130;
    run(&mut st, LayoutMsg::ZoomOut);
    assert_eq!(st.settings.zoom_percent, 125);
}

#[test]
fn toggle_panel_flips_and_saves() {
    for p in Panel::ALL {
        let mut st = state();
        assert!(saves(&run(&mut st, LayoutMsg::TogglePanel(p))));
        assert!(!shown(&st.settings, p), "{p:?}");
        assert!(Panel::ALL.iter().filter(|&&o| o != p).all(|&o| shown(&st.settings, o)), "only {p:?} changes");
        assert!(saves(&run(&mut st, LayoutMsg::TogglePanel(p))));
        assert!(shown(&st.settings, p), "{p:?}");
        // Only hiding the menu bar says how to get it back.
        assert_eq!(st.toasts.len(), usize::from(p == Panel::MenuBar), "{p:?}");
    }
}

#[test]
fn hiding_the_menu_bar_offers_it_back() {
    let mut st = state();
    assert!(st.settings.show_menu_bar);
    assert!(saves(&run(&mut st, LayoutMsg::SetPanel { panel: Panel::MenuBar, shown: false })));
    assert!(!st.settings.show_menu_bar);
    assert_eq!(toast_texts(&st), ["Menu bar hidden"]);
    let action = st.toasts[0].action.as_ref().expect("a Show menu bar button");
    assert_eq!(action.label, "Show menu bar");
    assert!(action.workdir.is_none());
    assert!(matches!(*action.msg, Msg::Layout(LayoutMsg::SetPanel { panel: Panel::MenuBar, shown: true })));
    // Already hidden: nothing saved, no second toast.
    assert!(!saves(&run(&mut st, LayoutMsg::SetPanel { panel: Panel::MenuBar, shown: false })));
    assert_eq!(st.toasts.len(), 1);

    let id = st.toasts[0].id;
    let fx = update(&mut st, Msg::ToastAction(id), Instant::now());
    assert!(st.settings.show_menu_bar);
    assert!(saves(&fx));
    assert!(st.toasts.is_empty(), "the toast goes with its button");
    // Showing it says nothing.
    assert!(!saves(&run(&mut st, LayoutMsg::SetPanel { panel: Panel::MenuBar, shown: true })));
    assert!(st.toasts.is_empty());
}

#[test]
fn palette_lists_the_menu_bar_without_a_key() {
    let mut st = state();
    let hide = find(&st, "Hide menu bar").expect("Hide menu bar");
    assert_eq!(hide.shortcut(&st.keymap), None);
    assert!(find(&st, "Show menu bar").is_none());
    update(&mut st, Msg::Palette(PaletteMsg::TogglePalette), Instant::now());
    let fx = update(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(hide.command)), Instant::now());
    assert!(!st.settings.show_menu_bar);
    assert!(saves(&fx));
    assert_eq!(st.palette_recent.keys(), ["menu-bar"]);
    let show = find(&st, "Show menu bar").expect("Show menu bar");
    assert_eq!(show.shortcut(&st.keymap), None);
    assert!(find(&st, "Hide menu bar").is_none());
}

#[test]
fn set_panel_same_value_saves_nothing() {
    let mut st = state();
    assert!(!saves(&run(&mut st, LayoutMsg::SetPanel { panel: Panel::Toolbar, shown: true })));
    assert!(saves(&run(&mut st, LayoutMsg::SetPanel { panel: Panel::Toolbar, shown: false })));
    assert!(!st.settings.show_toolbar);
    assert!(!saves(&run(&mut st, LayoutMsg::SetPanel { panel: Panel::Toolbar, shown: false })));
}

#[test]
fn focus_message_reveals_hidden_details() {
    let dir = scratch_repo("layout-focus-message");
    let mut st = state();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(100)).unwrap());
    st.settings.show_details_panel = false;
    let fx = update(&mut st, Msg::Shortcut(KeyAction::FocusMessage), Instant::now());
    assert!(st.settings.show_details_panel);
    assert!(st.view.focus_message);
    assert!(saves(&fx));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn layout_messages_keep_nothing_for_inactive_tabs() {
    let all = [
        LayoutMsg::ZoomIn,
        LayoutMsg::ZoomOut,
        LayoutMsg::ZoomReset,
        LayoutMsg::TogglePanel(Panel::Left),
        LayoutMsg::SetPanel { panel: Panel::Details, shown: false },
    ];
    for m in all {
        assert!(m.clone().on_inactive().is_none(), "{m:?}");
    }
}

#[test]
fn toast_replacing_updates_a_live_toast_or_adds_one() {
    let mut st = state();
    let t0 = Instant::now();
    let id = st.toast_replacing(None, true, "one", t0);
    let later = t0 + Duration::from_secs(2);
    assert_eq!(st.toast_replacing(Some(id), false, "two", later), id);
    assert_eq!(st.toasts.len(), 1);
    assert_eq!((st.toasts[0].text.as_str(), st.toasts[0].ok, st.toasts[0].at), ("two", false, later));
    // Gone (expired and dropped by the view): a new toast.
    st.toasts.clear();
    let next = st.toast_replacing(Some(id), true, "three", later);
    assert_ne!(next, id);
    assert_eq!(toast_texts(&st), ["three"]);
}

#[test]
fn palette_offers_zoom_by_level() {
    let mut st = state();
    let zoom_in = find(&st, "Zoom in").expect("Zoom in");
    assert_eq!(zoom_in.detail.as_deref(), Some("now 100%"));
    assert_eq!(zoom_in.shortcut(&st.keymap), st.keymap.hint(BindingId::ZoomIn));
    assert_eq!(zoom_in.shortcut(&st.keymap).map(str::to_owned), Some(keys("Ctrl+=")));
    assert!(find(&st, "Zoom out").is_some_and(|e| e.detail.as_deref() == Some("now 100%")));
    assert!(find(&st, "Reset zoom to 100%").is_none());

    st.settings.zoom_percent = 200;
    assert!(find(&st, "Zoom in").is_none());
    assert!(find(&st, "Zoom out").is_some());
    assert!(find(&st, "Reset zoom to 100%").is_some_and(|e| e.detail.as_deref() == Some("now 200%")));
    st.settings.zoom_percent = 50;
    assert!(find(&st, "Zoom out").is_none());
}

#[test]
fn palette_panel_entries_follow_state() {
    let mut st = state();
    for label in ["Hide left panel", "Hide commit details panel", "Hide toolbar"] {
        assert!(find(&st, label).is_some(), "{label}");
    }
    let hide = find(&st, "Hide toolbar").unwrap();
    assert_eq!(hide.shortcut(&st.keymap), st.keymap.hint(BindingId::ToggleToolbar));
    st.settings.show_toolbar = false;
    assert!(find(&st, "Show toolbar").is_some());
    assert!(find(&st, "Hide toolbar").is_none());
}

#[test]
fn palette_runs_layout_commands() {
    let mut st = state();
    update(&mut st, Msg::Palette(PaletteMsg::TogglePalette), Instant::now());
    let hide = C::Layout(LayoutMsg::SetPanel { panel: Panel::Toolbar, shown: false });
    let fx = update(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(hide)), Instant::now());
    assert!(!st.settings.show_toolbar);
    assert!(saves(&fx));
    assert!(st.palette.is_none());
    assert_eq!(st.palette_recent.keys(), ["toolbar"]);

    update(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::Layout(LayoutMsg::ZoomIn))), Instant::now());
    assert_eq!(st.settings.zoom_percent, 110);
    assert_eq!(st.palette_recent.keys(), ["zoom-in", "toolbar"]);
}
