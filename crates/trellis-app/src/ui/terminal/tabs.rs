//! The terminal panel's tab strip: one "Terminal N" per shell of the repository (shown from two
//! on), each with a painted close cross, then New terminal tab. Many tabs scroll sideways
//! instead of pushing out the header's buttons.

use crate::msg::Msg;
use crate::state::AppState;
use crate::terminal::TerminalMsg;
use crate::ui::theme::color;
use crate::ui::widgets::{self, Icon};
use eframe::egui;

/// Room kept for the New terminal tab button after the strip.
const PLUS_W: f32 = 24.0;

pub(super) fn strip(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let Some(workdir) = st.snap.as_ref().map(|s| &s.workdir) else { return };
    let tabs: Vec<_> = st.terminal.sessions_for(workdir).collect();
    let active = st.terminal.active_id(workdir);
    if tabs.len() >= 2 {
        ui.add_space(8.0);
        egui::ScrollArea::horizontal().id_salt("terminal-tabs").max_width((ui.available_width() - PLUS_W).max(0.0)).show(ui, |ui| {
            ui.horizontal(|ui| {
                for s in &tabs {
                    let name = format!("Terminal {}", s.number);
                    let label = if s.alive() { name.clone() } else { format!("{name} (exited)") };
                    if ui.selectable_label(active == Some(s.id), label).clicked() {
                        msgs.push(Msg::Terminal(TerminalMsg::SelectTab(s.id)));
                    }
                    if widgets::small_icon(ui, Icon::Close, &format!("Close {name}"), color::weak()).clicked() {
                        msgs.push(Msg::Terminal(TerminalMsg::CloseTab(s.id)));
                    }
                    ui.add_space(4.0);
                }
            });
        });
    } else {
        ui.add_space(4.0);
    }
    if widgets::small_icon(ui, Icon::Plus, "New terminal tab", color::weak()).clicked() {
        msgs.push(Msg::Terminal(TerminalMsg::NewTab));
    }
}
