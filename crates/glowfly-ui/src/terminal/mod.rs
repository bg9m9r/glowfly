//! The built-in terminal panel under the central area: a 40 px header (header.rs: the tabs,
//! New terminal and the shell picker, and the panel's buttons) over the active tab's screen,
//! and the grid's cell size (metrics). Its height is remembered when a drag of its edge ends.
//! Maximised, it takes the central area instead (`show_in_main`).

mod colors;
mod grid;
mod header;
mod held;
mod input;
pub(super) mod metrics;
mod mouse;
mod paint;
mod pill;
mod tabs;

use crate::msg::{Msg, PrefsMsg};
use crate::state::panel_layout::{self, MAIN_MIN_HEIGHT, TERMINAL};
use crate::state::terminal::{SessionStatus, TermSession, maximized_shown};
use crate::state::{AppState, PrefPage};
use crate::ui::common::menu_item_if;
use crate::ui::theme::color;
use crate::ui::widgets::buttons;
use eframe::egui::{self, Align, Layout, RichText};
use glowfly_core::i18n::Lang;
use glowfly_core::settings::Settings;
use glowfly_model::terminal::TerminalMsg;

/// The shell menu, for its snapshot (drawn the same on every OS).
#[cfg(test)]
pub(super) use tabs::{shell_items, shell_menu};

/// The panel's frame: its surface, 8 px at the sides and 6 px under the grid; the header
/// starts at the top edge.
fn frame() -> egui::Frame {
    let side = header::SIDE as i8;
    egui::Frame::new().fill(color::panel()).inner_margin(egui::Margin { left: side, right: side, top: 0, bottom: 6 })
}

/// Draws the panel while it is open and a repository is, unless it is maximised (then
/// `show_in_main` draws it).
pub fn show(ctx: &egui::Context, st: &mut AppState, msgs: &mut Vec<Msg>) {
    if !st.terminal.open || st.snap.is_none() {
        st.terminal.focused = false;
        return;
    }
    if maximized_shown(st) {
        return;
    }
    // Sized from the height left under the bars and the logs panel, keeping the main view.
    let fit = panel_layout::fit_one(ctx.available_rect().height(), MAIN_MIN_HEIGHT, TERMINAL, st.settings.terminal.panel_height);
    let id = egui::Id::new("terminal-panel");
    crate::ui::panel_size::pin(ctx, id, fit.size, st.terminal.resizing);
    let resp = egui::TopBottomPanel::bottom(id).resizable(true).default_height(fit.size).height_range(fit.lo..=fit.hi).frame(frame()).show(
        ctx,
        |ui| {
            header::show(ui, st, msgs);
            body(ui, st, msgs);
        },
    );
    // Saved once a drag of the edge ends, not every frame or when the window clamps it.
    let dragging = crate::ui::panel_size::dragging(ctx, id);
    if st.terminal.resizing && !dragging {
        msgs.push(Msg::Terminal(TerminalMsg::PanelHeight(resp.response.rect.height())));
    }
    st.terminal.resizing = dragging;
}

/// The maximised panel in the central area, in place of the main view (no resize edge).
/// Returns where it was drawn, for the tour.
pub fn show_in_main(ctx: &egui::Context, st: &mut AppState, msgs: &mut Vec<Msg>) -> egui::Rect {
    st.terminal.resizing = false;
    egui::CentralPanel::default()
        .frame(frame())
        .show(ctx, |ui| {
            header::show(ui, st, msgs);
            body(ui, st, msgs);
        })
        .response
        .rect
}

fn body(ui: &mut egui::Ui, st: &mut AppState, msgs: &mut Vec<Msg>) {
    let index = st.snap.as_ref().and_then(|s| st.terminal.active_id(&s.workdir)).and_then(|id| st.terminal.index_of(id));
    let Some(index) = index else {
        st.terminal.focused = false;
        ui.label(RichText::new("Starting shell…").color(color::weak()));
        return;
    };
    match st.terminal.sessions[index].status.clone() {
        SessionStatus::Starting => {
            st.terminal.focused = false;
            ui.label(RichText::new("Starting shell…").color(color::weak()));
        }
        SessionStatus::Running => grid::show(ui, st, index, msgs),
        SessionStatus::Exited(code) => {
            ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                exit_strip(ui, code, msgs);
                ui.with_layout(Layout::top_down(Align::Min), |ui| grid::show(ui, st, index, msgs));
            });
        }
        SessionStatus::Held => {
            st.terminal.focused = false;
            held::show(ui, st, msgs);
        }
        SessionStatus::Failed(error) => {
            st.terminal.focused = false;
            ui.label(RichText::new(format!("Could not start the shell: {error}")).color(color::removed()));
            ui.horizontal(|ui| {
                if ui.add(buttons::secondary("Restart")).clicked() {
                    msgs.push(Msg::Terminal(TerminalMsg::Restart));
                }
                if ui.add(buttons::secondary("Open preferences")).clicked() {
                    msgs.push(Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Terminal)));
                }
            });
        }
    }
}

/// The grid's right-click menu.
fn context_menu(ui: &mut egui::Ui, s: &TermSession, has_selection: bool, msgs: &mut Vec<Msg>) {
    let t = |m| Msg::Terminal(m);
    menu_item_if(msgs, ui, "Copy", (!has_selection).then_some("Nothing is selected"), t(TerminalMsg::CopySelection { keep: true }));
    menu_item_if(msgs, ui, "Paste", (!s.alive()).then_some("The shell has ended"), t(TerminalMsg::Paste));
    menu_item_if(msgs, ui, "Select all", None, t(TerminalMsg::SelectAll));
    let full_screen = s.screen.alternate().then_some("A full-screen program is showing");
    menu_item_if(msgs, ui, "Clear", full_screen, t(TerminalMsg::Clear));
}

/// The toolbar Terminal button's right-click item, which sends `Msg::OpenTerminal`.
pub(super) fn right_click_item(t: Lang, s: &Settings) -> &'static str {
    if s.terminal.open_in_panel { t.tr("toolbar.terminal.new_tab") } else { t.tr("toolbar.terminal.external") }
}

/// "Process exited with code N" and a Restart button across the bottom of the panel.
fn exit_strip(ui: &mut egui::Ui, code: Option<u32>, msgs: &mut Vec<Msg>) {
    egui::Frame::new().fill(color::toolbar()).inner_margin(egui::Margin::symmetric(8, 4)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            let text = match code {
                Some(c) => format!("Process exited with code {c}"),
                None => "Process exited".to_owned(),
            };
            ui.label(RichText::new(text).color(color::text()));
            if ui.add(buttons::secondary("Restart").small()).clicked() {
                msgs.push(Msg::Terminal(TerminalMsg::Restart));
            }
        });
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn right_click_item_follows_the_setting() {
        let mut s = glowfly_core::settings::Settings::default();
        assert_eq!(super::right_click_item(glowfly_core::i18n::Lang::En, &s), "Open in external terminal");
        s.terminal.open_in_panel = true;
        assert_eq!(super::right_click_item(glowfly_core::i18n::Lang::En, &s), "New terminal tab");
    }
}
