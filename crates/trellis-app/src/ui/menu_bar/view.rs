//! View: zoom, the panels, the terminal and the activity log, the diff settings, the theme and
//! Refresh.

use super::items::{clicked, item};
use crate::keymap::BindingId as I;
use crate::msg::{DiffMsg, LayoutMsg, Msg, PrefsMsg, RepoMsg};
use crate::state::layout::{Panel, shown};
use crate::state::menu_bar::zoom_blocked;
use crate::state::{AppState, PrefPage};
use crate::terminal::TerminalMsg;
use crate::ui::theme::color;
use eframe::egui::{self, Button, RichText};
use trellis_core::settings::{DiffLayout, DiffMode, ThemeChoice};

pub(super) fn menu(ui: &mut egui::Ui, st: &mut AppState, msgs: &mut Vec<Msg>) {
    ui.menu_button("View", |ui| {
        ui.set_min_width(260.0);
        let keys = &st.keymap;
        let [zoom_in, zoom_out, reset] = zoom_blocked(st.settings.zoom_level());
        item(ui, keys, "Zoom in", Some(I::ZoomIn), zoom_in.as_deref(), Msg::Layout(LayoutMsg::ZoomIn), msgs);
        item(ui, keys, "Zoom out", Some(I::ZoomOut), zoom_out.as_deref(), Msg::Layout(LayoutMsg::ZoomOut), msgs);
        item(ui, keys, "Reset zoom to 100%", Some(I::ZoomReset), reset.as_deref(), Msg::Layout(LayoutMsg::ZoomReset), msgs);
        ui.separator();
        for panel in Panel::ALL {
            let on = shown(&st.settings, panel);
            let label = format!("{} {}", if on { "Hide" } else { "Show" }, panel.name());
            item(ui, keys, label, panel.binding(), None, Msg::Layout(LayoutMsg::SetPanel { panel, shown: !on }), msgs);
        }
        let terminal = if st.terminal.open { "Hide terminal" } else { "Show terminal" };
        let no_repo = st.snap.is_none().then_some("Open a repository first");
        item(ui, keys, terminal, Some(I::ToggleTerminal), no_repo, Msg::Terminal(TerminalMsg::Toggle), msgs);
        let log = if st.view.show_log { "Hide activity log" } else { "Show activity log" };
        // View state, like the toolbar's Activity button.
        if clicked(ui, Button::new(log), None) {
            st.view.show_log = !st.view.show_log;
            ui.close_menu();
        }
        ui.separator();
        diff_items(ui, st, msgs);
        ui.menu_button("Theme", |ui| theme_items(ui, st, msgs));
        ui.separator();
        item(ui, &st.keymap, "Refresh", Some(I::Refresh), None, Msg::Repo(RepoMsg::Refresh), msgs);
    });
}

/// The diff viewer's saved settings: they apply whether or not a diff is open.
fn diff_items(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let s = &st.settings;
    ui.label(RichText::new("Diff").size(12.0).color(color::weak()));
    let mut push = |ui: &mut egui::Ui, m: DiffMsg| {
        msgs.push(Msg::Diff(m));
        ui.close_menu();
    };
    for (layout, label) in [(DiffLayout::Inline, "Inline"), (DiffLayout::Split, "Side by side")] {
        if ui.radio(s.diff_layout == layout, label).clicked() {
            push(ui, DiffMsg::SetDiffLayout(layout));
        }
    }
    for (mode, label) in [(DiffMode::Hunks, "Changed lines only"), (DiffMode::WholeFile, "Whole file")] {
        if ui.radio(s.diff_mode == mode, label).clicked() {
            push(ui, DiffMsg::SetDiffMode(mode));
        }
    }
    let mut ignore = s.diff_ignore_whitespace;
    if ui.checkbox(&mut ignore, "Ignore whitespace").clicked() {
        push(ui, DiffMsg::SetIgnoreWhitespace(ignore));
    }
    let mut wrap = s.diff_word_wrap;
    if ui.checkbox(&mut wrap, "Wrap long lines").clicked() {
        push(ui, DiffMsg::SetWordWrap(wrap));
    }
}

/// The built-in themes, then the theme files, then the Preferences page.
fn theme_items(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    ui.set_min_width(200.0);
    let current = &st.settings.theme;
    let mut pick = |ui: &mut egui::Ui, choice: ThemeChoice, label: String| {
        if ui.radio(*current == choice, label).clicked() {
            msgs.push(Msg::Prefs(PrefsMsg::SetTheme(choice)));
            ui.close_menu();
        }
    };
    for choice in ThemeChoice::BUILT_IN {
        let label = choice.label();
        pick(ui, choice, label);
    }
    let files = &st.themes.catalog.files;
    if !files.is_empty() {
        ui.separator();
    }
    for f in files {
        let label = if f.problems.is_empty() { f.name.clone() } else { format!("{} (has problems)", f.name) };
        pick(ui, ThemeChoice::Custom(f.id.clone()), label);
    }
    ui.separator();
    if ui.button("Theme settings…").clicked() {
        msgs.push(Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Ui)));
        ui.close_menu();
    }
}
