//! View: zoom, the panels, the terminal and the activity log, the diff settings, the theme and
//! Refresh.

use super::items::{check_item, item};
use crate::keymap::BindingId as I;
use crate::msg::{DiffMsg, LayoutMsg, Msg, PrefsMsg, RepoMsg};
use crate::state::layout::{Panel, shown};
use crate::state::menu_bar::zoom_blocked;
use crate::state::{AppState, PrefPage};
use crate::terminal::TerminalMsg;
use crate::ui::theme::color;
use crate::ui::theme::controls;
use crate::ui::theme::type_scale;
use crate::ui::theme_names;
use eframe::egui::{self, RichText};
use trellis_core::i18n::Lang;
use trellis_core::settings::{DiffLayout, DiffMode, ThemeChoice};

pub(super) fn menu(ui: &mut egui::Ui, st: &mut AppState, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    ui.menu_button(t.tr("menu.view.title"), |ui| {
        ui.set_min_width(260.0);
        let keys = &st.keymap;
        let [zoom_in, zoom_out, reset] = zoom_blocked(t, st.settings.zoom_level());
        item(ui, keys, t.tr("menu.view.zoom_in"), Some(I::ZoomIn), zoom_in.as_deref(), Msg::Layout(LayoutMsg::ZoomIn), msgs);
        item(ui, keys, t.tr("menu.view.zoom_out"), Some(I::ZoomOut), zoom_out.as_deref(), Msg::Layout(LayoutMsg::ZoomOut), msgs);
        let zoom_reset = Msg::Layout(LayoutMsg::ZoomReset);
        item(ui, keys, t.tr("menu.view.zoom_reset"), Some(I::ZoomReset), reset.as_deref(), zoom_reset, msgs);
        ui.separator();
        for panel in Panel::ALL {
            // A drawn title bar replaces the menu bar.
            if panel == Panel::MenuBar && st.view.chrome.drawn() {
                continue;
            }
            let on = shown(&st.settings, panel);
            if check_item(ui, keys, panel_label(t, panel), on, panel.binding(), None) {
                msgs.push(Msg::Layout(LayoutMsg::SetPanel { panel, shown: !on }));
                ui.close_menu();
            }
        }
        let no_repo = st.snap.is_none().then_some(t.tr("menu.common.no_repo"));
        if check_item(ui, keys, t.tr("menu.view.terminal"), st.terminal.open, Some(I::ToggleTerminal), no_repo) {
            msgs.push(Msg::Terminal(TerminalMsg::Toggle));
            ui.close_menu();
        }
        // View state, like the toolbar's Activity button.
        if check_item(ui, keys, t.tr("menu.view.activity_log"), st.view.show_log, None, None) {
            st.view.show_log = !st.view.show_log;
            ui.close_menu();
        }
        ui.separator();
        diff_items(ui, t, st, msgs);
        ui.menu_button(t.tr("menu.view.theme"), |ui| theme_items(ui, t, st, msgs));
        ui.separator();
        item(ui, &st.keymap, t.tr("menu.view.refresh"), Some(I::Refresh), None, Msg::Repo(RepoMsg::Refresh), msgs);
    });
}

/// The check item's label for `panel`.
fn panel_label(t: Lang, panel: Panel) -> &'static str {
    match panel {
        Panel::Left => t.tr("menu.view.left_panel"),
        Panel::Details => t.tr("menu.view.details_panel"),
        Panel::Toolbar => t.tr("menu.view.toolbar"),
        Panel::MenuBar => t.tr("menu.view.menu_bar"),
    }
}

/// The diff viewer's saved settings: they apply whether or not a diff is open.
fn diff_items(ui: &mut egui::Ui, t: Lang, st: &AppState, msgs: &mut Vec<Msg>) {
    let s = &st.settings;
    ui.label(RichText::new(t.tr("menu.view.diff")).size(type_scale::SMALL).color(color::weak()));
    let mut push = |ui: &mut egui::Ui, m: DiffMsg| {
        msgs.push(Msg::Diff(m));
        ui.close_menu();
    };
    for (layout, label) in [(DiffLayout::Inline, t.tr("menu.view.inline")), (DiffLayout::Split, t.tr("menu.view.split"))] {
        if controls::radio(ui, s.diff_layout == layout, label).clicked() {
            push(ui, DiffMsg::SetDiffLayout(layout));
        }
    }
    for (mode, label) in [(DiffMode::Hunks, t.tr("menu.view.hunks")), (DiffMode::WholeFile, t.tr("menu.view.whole_file"))] {
        if controls::radio(ui, s.diff_mode == mode, label).clicked() {
            push(ui, DiffMsg::SetDiffMode(mode));
        }
    }
    let mut ignore = s.diff_ignore_whitespace;
    if controls::checkbox(ui, &mut ignore, t.tr("menu.view.ignore_whitespace")).clicked() {
        push(ui, DiffMsg::SetIgnoreWhitespace(ignore));
    }
    let mut wrap = s.diff_word_wrap;
    if controls::checkbox(ui, &mut wrap, t.tr("menu.view.wrap")).clicked() {
        push(ui, DiffMsg::SetWordWrap(wrap));
    }
}

/// The built-in themes, then the theme files, then the Preferences page.
fn theme_items(ui: &mut egui::Ui, t: Lang, st: &AppState, msgs: &mut Vec<Msg>) {
    ui.set_min_width(200.0);
    let current = &st.settings.theme;
    let mut pick = |ui: &mut egui::Ui, choice: ThemeChoice, label: String| {
        if controls::radio(ui, *current == choice, label).clicked() {
            msgs.push(Msg::Prefs(PrefsMsg::SetTheme(choice)));
            ui.close_menu();
        }
    };
    for choice in ThemeChoice::BUILT_IN {
        let label = theme_names::choice(t, &choice);
        pick(ui, choice, label);
    }
    let files = &st.themes.catalog.files;
    if !files.is_empty() {
        ui.separator();
    }
    for f in files {
        pick(ui, ThemeChoice::Custom(f.id.clone()), theme_names::file_entry(t, f));
    }
    ui.separator();
    if ui.button(t.tr("menu.view.theme_settings")).clicked() {
        msgs.push(Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Ui)));
        ui.close_menu();
    }
}
