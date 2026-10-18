//! The built-in terminal panel under the central area: a header (the shell and repository, the
//! terminal tabs, New terminal tab, Kill all, Kill or Restart, Hide) over the active tab's
//! screen. Its height is remembered when a drag of its edge ends.

mod colors;
mod grid;
mod input;
mod mouse;
mod paint;
mod tabs;

use crate::msg::{Msg, PrefsMsg};
use crate::state::panel_layout::{self, MAIN_MIN_HEIGHT, TERMINAL};
use crate::state::terminal::{SessionStatus, TermSession};
use crate::state::{AppState, PrefPage};
use crate::terminal::TerminalMsg;
use crate::ui::common::menu_item_if;
use crate::ui::theme::color;
use eframe::egui::{self, Align, FontSelection, Layout, RichText, text::LayoutJob};
use trellis_core::i18n::Lang;
use trellis_core::settings::Settings;

const KILL_TIP: &str = "Stop the shell and everything running in it";

/// Draws the panel while it is open and a repository is.
pub fn show(ctx: &egui::Context, st: &mut AppState, msgs: &mut Vec<Msg>) {
    if !st.terminal.open || st.snap.is_none() {
        st.terminal.focused = false;
        return;
    }
    // Sized from the height left under the bars and the logs panel, keeping the main view.
    let fit = panel_layout::fit_one(ctx.available_rect().height(), MAIN_MIN_HEIGHT, TERMINAL, st.settings.terminal.panel_height);
    let id = egui::Id::new("terminal-panel");
    crate::ui::panel_size::pin(ctx, id, fit.size, st.terminal.resizing);
    let resp = egui::TopBottomPanel::bottom(id)
        .resizable(true)
        .default_height(fit.size)
        .height_range(fit.lo..=fit.hi)
        .frame(egui::Frame::new().fill(color::panel()).inner_margin(egui::Margin::symmetric(6, 4)))
        .show(ctx, |ui| {
            header(ui, st, msgs);
            body(ui, st, msgs);
        });
    // Saved once a drag of the edge ends, not every frame or when the window clamps it.
    let dragging = crate::ui::panel_size::dragging(ctx, id);
    if st.terminal.resizing && !dragging {
        msgs.push(Msg::Terminal(TerminalMsg::PanelHeight(resp.response.rect.height())));
    }
    st.terminal.resizing = dragging;
}

fn header(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let session = crate::state::terminal::active(st);
    let snap = st.snap.as_ref();
    let tab = snap.map(|s| s.name.as_str()).unwrap_or_default();
    let alive = snap.map_or(0, |s| st.terminal.sessions_for(&s.workdir).filter(|t| t.alive()).count());
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui
                .small_button("Hide")
                .on_hover_text(st.keymap.tip("Hide the terminal panel", crate::keymap::BindingId::ToggleTerminal))
                .clicked()
            {
                msgs.push(Msg::Terminal(TerminalMsg::Hide));
            }
            match session {
                Some(s) if !s.alive() => {
                    if ui.small_button("Restart").clicked() {
                        msgs.push(Msg::Terminal(TerminalMsg::Restart));
                    }
                }
                _ => {
                    let alive = session.is_some_and(|s| s.alive());
                    if ui.add_enabled(alive, egui::Button::new("Kill").small()).on_hover_text(KILL_TIP).clicked() {
                        msgs.push(Msg::Terminal(TerminalMsg::Kill));
                    }
                }
            }
            if alive >= 2 && ui.small_button("Kill all").on_hover_text("Stop every terminal of this repository").clicked() {
                msgs.push(Msg::Terminal(TerminalMsg::AskKillAll));
            }
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                let program = session.map(|s| s.program.as_str()).unwrap_or_default();
                let mut job = LayoutJob::default();
                RichText::new("Terminal").strong().append_to(&mut job, ui.style(), FontSelection::Default, Align::Center);
                RichText::new(subtitle(program, tab)).color(color::weak()).append_to(
                    &mut job,
                    ui.style(),
                    FontSelection::Default,
                    Align::Center,
                );
                ui.label(job);
                tabs::strip(ui, st, msgs);
            });
        });
    });
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
        SessionStatus::Failed(error) => {
            st.terminal.focused = false;
            ui.label(RichText::new(format!("Could not start the shell: {error}")).color(color::red()));
            ui.horizontal(|ui| {
                if ui.button("Restart").clicked() {
                    msgs.push(Msg::Terminal(TerminalMsg::Restart));
                }
                if ui.button("Open Preferences").clicked() {
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

/// What follows "Terminal" in the header: " · program · tab", leaving out what is empty.
fn subtitle(program: &str, tab: &str) -> String {
    [program, tab].iter().filter(|s| !s.is_empty()).map(|s| format!(" · {s}")).collect()
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
            if ui.small_button("Restart").clicked() {
                msgs.push(Msg::Terminal(TerminalMsg::Restart));
            }
        });
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn subtitle_separates_the_program_and_tab_from_terminal() {
        assert_eq!(format!("Terminal{}", super::subtitle("pwsh", "demo")), "Terminal · pwsh · demo");
        assert_eq!(super::subtitle("", "demo"), " · demo");
        assert_eq!(super::subtitle("bash", ""), " · bash");
        assert_eq!(super::subtitle("", ""), "");
    }

    #[test]
    fn right_click_item_follows_the_setting() {
        let mut s = trellis_core::settings::Settings::default();
        assert_eq!(super::right_click_item(trellis_core::i18n::Lang::En, &s), "Open in external terminal");
        s.terminal.open_in_panel = true;
        assert_eq!(super::right_click_item(trellis_core::i18n::Lang::En, &s), "New terminal tab");
    }
}
