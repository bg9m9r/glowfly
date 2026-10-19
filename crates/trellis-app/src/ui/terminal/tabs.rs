//! The terminal panel's tab strip: one "Terminal N" per shell of the repository (shown from two
//! on), each with a painted close cross, then New terminal tab. Many tabs scroll sideways
//! instead of pushing out the header's buttons. The crosses and the + are `tab_glyph` buttons
//! on the header's centre line.

use crate::msg::Msg;
use crate::state::AppState;
use crate::state::terminal::TermSession;
use crate::terminal::TerminalMsg;
use crate::ui::theme::color;
use crate::ui::widgets::tab_glyph::{self, TabGlyph};
use eframe::egui::{self, Align, Id, Layout, Sense, pos2, vec2};

/// Room kept for the New terminal tab button after the strip: its slot and the spacing.
const PLUS_W: f32 = 24.0;

/// The height of the strip's tab labels while the strip is shown (two shells or more). They
/// are taller than the header's small buttons, so the header's row starts this tall.
pub(super) fn row_height(ui: &egui::Ui, st: &AppState) -> Option<f32> {
    let workdir = &st.snap.as_ref()?.workdir;
    let label = ui.text_style_height(&egui::TextStyle::Button) + 2.0 * ui.spacing().button_padding.y;
    (st.terminal.sessions_for(workdir).count() >= 2).then(|| label.max(ui.spacing().interact_size.y))
}

pub(super) fn strip(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let Some(workdir) = st.snap.as_ref().map(|s| &s.workdir) else { return };
    let tabs: Vec<_> = st.terminal.sessions_for(workdir).collect();
    let active = st.terminal.active_id(workdir);
    let mut row = None;
    if tabs.len() >= 2 {
        ui.add_space(8.0);
        let w = (ui.available_width() - PLUS_W).max(0.0);
        // The strip sat 4 px below the buttons' line: a `ui.horizontal` in here starts as a row
        // only `interact_size.y` tall, which the header's centred row centres and the taller
        // labels then grow downward from, and the buttons were centred in the header's row
        // before the labels made it taller (`row_height` sizes it first now). A left-to-right
        // layout over the area's whole height centres every label, cross and the + on the line.
        let area = egui::ScrollArea::horizontal().id_salt("terminal-tabs").max_width(w);
        let centred = Layout::left_to_right(Align::Center);
        row = Some(area.show(ui, |ui| ui.with_layout(centred, |ui| tab_buttons(ui, &tabs, active, msgs)).response.rect).inner);
    } else {
        ui.add_space(4.0);
    }
    let (slot, _) = ui.allocate_exact_size(vec2(PLUS_W, tab_glyph::HIT), Sense::hover());
    let centre = pos2(slot.center().x, row.map_or(slot.center().y, |r| r.center().y));
    let plus =
        tab_glyph::button_at(ui, Id::new("new-terminal-tab"), centre, TabGlyph::Plus, "New terminal tab", color::weak(), color::hover());
    if plus.clicked() {
        msgs.push(Msg::Terminal(TerminalMsg::NewTab));
    }
}

/// Each shell's label and close cross.
fn tab_buttons(ui: &mut egui::Ui, tabs: &[&TermSession], active: Option<u64>, msgs: &mut Vec<Msg>) {
    for s in tabs {
        let name = format!("Terminal {}", s.number);
        let label = if s.alive() { name.clone() } else { format!("{name} (exited)") };
        if ui.selectable_label(active == Some(s.id), label).clicked() {
            msgs.push(Msg::Terminal(TerminalMsg::SelectTab(s.id)));
        }
        let (slot, _) = ui.allocate_exact_size(vec2(tab_glyph::HIT, tab_glyph::HIT), Sense::hover());
        let id = ui.id().with(("terminal-tab-close", s.id));
        let close = tab_glyph::button_at(ui, id, slot.center(), TabGlyph::Close, &format!("Close {name}"), color::weak(), color::hover());
        if close.clicked() {
            msgs.push(Msg::Terminal(TerminalMsg::CloseTab(s.id)));
        }
        ui.add_space(4.0);
    }
}
