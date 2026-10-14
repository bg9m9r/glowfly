//! The built-in terminal panel under the central area: a header (the shell and repository,
//! Hide, Kill or Restart) over the active repository's shell screen.

mod colors;
mod grid;
mod input;

use crate::msg::Msg;
use crate::state::AppState;
use crate::state::terminal::SessionStatus;
use crate::terminal::TerminalMsg;
use crate::ui::theme::color;
use eframe::egui::{self, Align, FontSelection, Layout, RichText, text::LayoutJob};

const HIDE_TIP: &str = "Hide the terminal panel (Ctrl+`)";
const KILL_TIP: &str = "Stop the shell and everything running in it";

/// Ctrl+` (Ctrl on every system) shows or hides the panel, also while the terminal has the
/// keyboard. Preferences, dialogs and the palette keep it.
pub fn shortcut(ctx: &egui::Context, st: &AppState, msgs: &mut Vec<Msg>) {
    if st.prefs.is_some() || st.modal.is_some() || st.askpass.showing() || st.palette.is_some() {
        return;
    }
    ctx.input_mut(|i| {
        if super::shortcuts::pressed(i, crate::keymap::BindingId::ToggleTerminal) {
            msgs.push(Msg::Terminal(TerminalMsg::Toggle));
        }
    });
}

/// Draws the panel while it is open and a repository is.
pub fn show(ctx: &egui::Context, st: &mut AppState, msgs: &mut Vec<Msg>) {
    if !st.terminal.open || st.snap.is_none() {
        st.terminal.focused = false;
        return;
    }
    let max = ctx.screen_rect().height() * 0.7;
    egui::TopBottomPanel::bottom("terminal-panel")
        .resizable(true)
        .default_height(220.0)
        .min_height(90.0)
        .max_height(max)
        .frame(egui::Frame::new().fill(color::panel()).inner_margin(egui::Margin::symmetric(6, 4)))
        .show(ctx, |ui| {
            header(ui, st, msgs);
            body(ui, st, msgs);
        });
}

fn header(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let session = crate::state::terminal::active(st);
    let tab = st.snap.as_ref().map(|s| s.name.as_str()).unwrap_or_default();
    ui.horizontal(|ui| {
        let program = session.map(|s| s.program.as_str()).unwrap_or_default();
        let mut job = LayoutJob::default();
        RichText::new("Terminal").strong().append_to(&mut job, ui.style(), FontSelection::Default, Align::Center);
        RichText::new(subtitle(program, tab)).color(color::weak()).append_to(&mut job, ui.style(), FontSelection::Default, Align::Center);
        ui.label(job);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.small_button("Hide").on_hover_text(HIDE_TIP).clicked() {
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
        });
    });
}

fn body(ui: &mut egui::Ui, st: &mut AppState, msgs: &mut Vec<Msg>) {
    let workdir = st.snap.as_ref().map(|s| s.workdir.clone()).unwrap_or_default();
    let Some(index) = st.terminal.sessions.iter().position(|s| s.workdir == workdir) else {
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
            if ui.button("Restart").clicked() {
                msgs.push(Msg::Terminal(TerminalMsg::Restart));
            }
        }
    }
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
}
