//! The logs panel header's filter controls and what the lists say when nothing is left: the
//! search field (one for the three tabs, with a painted clear button while it has text), the
//! Failures only toggle on the Activity tab, and the faint line for a filter that shows nothing.
//! The text lives in `ViewState::logs_search`, edited in place; the toggle and every change are
//! messages (`LogsMsg::SetFailuresOnly`, `LogsMsg::SearchChanged`).

use super::access;
use super::theme::{color, controls};
use crate::theme::sizes::ControlSize;
use crate::widgets::{self, Icon, truncate};
use eframe::egui::{self, CursorIcon, Rect, RichText, Sense, Ui, pos2, vec2};
use glowfly_core::i18n::Lang;
use glowfly_model::logs::{LogKind, LogsMsg};
use glowfly_model::msg::Msg;
use glowfly_model::state::{AppState, ViewState};

/// The field's widest and narrowest width.
const FIELD_W: f32 = 240.0;
const FIELD_MIN_W: f32 = 140.0;
/// What the Failures only toggle and the gap before it take from the field's room.
const TOGGLE_ROOM: f32 = 150.0;
/// The field's right inner margin, where the clear button sits so text never runs under it.
const CLEAR_ROOM: f32 = 26.0;

/// The search field, then the Failures only toggle (Activity tab only), in the header's left
/// cluster after the tabs.
pub(super) fn controls(ui: &mut Ui, st: &AppState, view: &mut ViewState, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let hint = match st.logs.tab {
        LogKind::Activity => t.tr("logs.filter.search.activity"),
        LogKind::Errors | LogKind::Performance => t.tr("logs.filter.search.file"),
    };
    let width = (ui.available_width() - TOGGLE_ROOM).clamp(FIELD_MIN_W, FIELD_W);
    let (shown_hint, cut) = truncate::hint(ui, hint, width - controls::FIELD_PAD_X - CLEAR_ROOM);
    let edit = egui::TextEdit::singleline(&mut view.logs_search).hint_text(shown_hint);
    let r = ui.add(controls::SizedField::new(ControlSize::Regular, edit).width(width).trailing_room(CLEAR_ROOM));
    access::name_if_unnamed(&r, hint);
    let r = if cut && view.logs_search.is_empty() { r.on_hover_text(hint) } else { r };
    let mut changed = r.changed();
    if !view.logs_search.is_empty() && clear_button(ui, r.rect, t.tr("search.clear.button")) {
        view.logs_search.clear();
        changed = true;
        r.request_focus();
    }
    if changed {
        msgs.push(Msg::Logs(LogsMsg::SearchChanged));
    }
    if st.logs.tab == LogKind::Activity {
        ui.add_space(8.0);
        let mut on = st.logs.failures_only;
        let check = ui.add(controls::Check::new(&mut on, t.tr("logs.filter.failures"))).on_hover_text(t.tr("logs.filter.failures.tip"));
        if check.changed() {
            msgs.push(Msg::Logs(LogsMsg::SetFailuresOnly(on)));
        }
    }
}

/// The painted × inside the field's right end; whether it was clicked.
fn clear_button(ui: &mut Ui, field: Rect, name: &str) -> bool {
    let x = Rect::from_center_size(pos2(field.right() - CLEAR_ROOM / 2.0, field.center().y), vec2(18.0, 18.0));
    let c = ui.interact(x, ui.id().with("logs-search-clear"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).on_hover_text(name);
    let col = if c.hovered() { color::text() } else { color::faint() };
    widgets::paint_icon(ui.painter(), x.center(), Icon::Close, col);
    access::name_button(&c, true, name);
    c.clicked()
}

/// The faint line in place of a list a filter left empty: `file` for a log tab's lines, else the
/// activity log's entries (by which of Failures only and the search is on).
pub(super) fn none_line(ui: &mut Ui, t: Lang, failures_only: bool, query: &str, file: bool) {
    let query = query.trim();
    let q: [(&str, &dyn std::fmt::Display); 1] = [("query", &query)];
    let text = match (file, failures_only, query.is_empty()) {
        (true, ..) => t.trf("logs.file.none", &q),
        (false, true, true) => t.trf("logs.filter.none.failures", &q),
        (false, true, false) => t.trf("logs.filter.none.both", &q),
        (false, false, _) => t.trf("logs.filter.none.search", &q),
    };
    ui.add_space(8.0);
    ui.add(egui::Label::new(RichText::new(text).color(color::faint())).wrap());
}
