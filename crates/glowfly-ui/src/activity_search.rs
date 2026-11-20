//! The logs panel header's filter controls and what the lists say when nothing is left: the
//! search field (one for the three tabs, with a painted clear button while it has text), the
//! Failures only toggle on the Activity tab, then the match count ("3 of 120", a polite live
//! region; "2 of 3 matches" while stepping) with the painted Previous / Next match buttons
//! (Activity tab only), and the faint line for a filter that shows nothing. Enter in the field
//! steps to the next shown row, Shift+Enter to the previous one (`LogsMsg::Step`). The text
//! lives in `ViewState::logs_search`, edited in place; the toggle and every change are messages
//! (`LogsMsg::SetFailuresOnly`, `LogsMsg::SearchChanged`).

use super::access::{self, Live};
use super::theme::{color, controls};
use crate::theme::sizes::ControlSize;
use crate::widgets::ghost::{ghost, icon_tip, text_w};
use crate::widgets::{self, Icon, paint_text, truncate};
use eframe::egui::{self, Align2, CursorIcon, Key, Rect, RichText, Sense, TextStyle, Ui, WidgetInfo, WidgetType, pos2, vec2};
use glowfly_core::i18n::Lang;
use glowfly_model::logs::{LogKind, LogsMsg};
use glowfly_model::msg::Msg;
use glowfly_model::state::activity_filter::{Filter, line_count, match_count, place};
use glowfly_model::state::tip::Tip;
use glowfly_model::state::{AppState, ViewState};

/// The field's widest and narrowest width.
const FIELD_W: f32 = 240.0;
const FIELD_MIN_W: f32 = 140.0;
/// What the Failures only toggle and the gap before it take from the field's room.
const TOGGLE_ROOM: f32 = 150.0;
/// What the match count and the two step buttons take from the field's room; always reserved, so
/// the field keeps its width while the count comes and goes.
const STEP_ROOM: f32 = 130.0;
/// The field's right inner margin, where the clear button sits so text never runs under it.
const CLEAR_ROOM: f32 = 26.0;

/// The search field, the Failures only toggle (Activity tab only), then the match count and step
/// buttons, in the header's left cluster after the tabs. The count goes after the toggle, so
/// turning the toggle on or off never moves it from under the pointer.
pub(super) fn controls(ui: &mut Ui, st: &AppState, view: &mut ViewState, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let hint = match st.logs.tab {
        LogKind::Activity => t.tr("logs.filter.search.activity"),
        LogKind::Errors | LogKind::Performance => t.tr("logs.filter.search.file"),
    };
    let width = (ui.available_width() - TOGGLE_ROOM - STEP_ROOM).clamp(FIELD_MIN_W, FIELD_W);
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
    if r.lost_focus()
        && ui.input(|i| i.key_pressed(Key::Enter))
        && st.logs.tab == LogKind::Activity
        && Filter::new(st.logs.failures_only, &view.logs_search).is_active()
    {
        // Enter and Shift+Enter both end the field's focus; it keeps it for the next press.
        let back = ui.input(|i| i.modifiers.shift);
        msgs.push(Msg::Logs(LogsMsg::Step { back }));
        r.request_focus();
    }
    if st.logs.tab == LogKind::Activity {
        ui.add_space(8.0);
        let mut on = st.logs.failures_only;
        let check = ui.add(controls::Check::new(&mut on, t.tr("logs.filter.failures"))).on_hover_text(t.tr("logs.filter.failures.tip"));
        if check.changed() {
            msgs.push(Msg::Logs(LogsMsg::SetFailuresOnly(on)));
        }
    }
    ui.add_space(8.0);
    counter(ui, st, view, msgs);
}

/// What the header says about the filter.
struct Count {
    /// The painted text, and what a screen reader reads.
    text: String,
    spoken: String,
    /// How many shown rows the Activity tab's step buttons can reach.
    steppable: Option<usize>,
}

/// The count the filter leaves: None while no filter is on, the log is empty, or a file tab has
/// nothing read yet.
fn count(st: &AppState, view: &ViewState) -> Option<Count> {
    let t = st.settings.lang();
    let plain = |shown: usize, total: usize| {
        (
            t.trf("logs.filter.count", &[("shown", &shown), ("total", &total)]),
            t.trn("logs.filter.count.access", shown, &[("total", &total)]),
        )
    };
    if st.logs.tab == LogKind::Activity {
        let f = Filter::new(st.logs.failures_only, &view.logs_search);
        let c = match_count(&st.log, &f).filter(|_| !st.log.is_empty())?;
        let at = view.activity_focus.filter(|_| view.activity_step).and_then(|id| place(&st.log, &f, id));
        let (text, spoken) = match at {
            Some(at) => {
                let said = t.trn("logs.filter.step", c.shown, &[("at", &at)]);
                (said.clone(), said)
            }
            None => plain(c.shown, c.total),
        };
        return Some(Count { text, spoken, steppable: Some(c.shown) });
    }
    let tail = st.logs.file.as_ref().filter(|v| v.kind == st.logs.tab)?.tail.as_ref()?.as_ref().ok()?;
    let c = line_count(&tail.lines, Filter::new(false, &view.logs_search).needle()).filter(|_| !tail.lines.is_empty())?;
    let (text, spoken) = plain(c.shown, c.total);
    Some(Count { text, spoken, steppable: None })
}

/// The weak count after the toggle, and on the Activity tab the Previous and Next match buttons.
/// Each is drawn only when its room is left, so a narrow window clips nothing.
fn counter(ui: &mut Ui, st: &AppState, view: &ViewState, msgs: &mut Vec<Msg>) {
    let Some(c) = count(st, view) else { return };
    let t = st.settings.lang();
    let h = ControlSize::Small.metrics().h;
    let gap = ui.spacing().item_spacing.x;
    let font = TextStyle::Body.resolve(ui.style());
    let w = text_w(ui, &c.text, font.clone());
    if ui.available_width() >= w {
        let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
        paint_text(ui.painter(), pos2(rect.left(), rect.center().y), Align2::LEFT_CENTER, &c.text, font, color::weak());
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &c.spoken));
        access::announce(&resp, egui::Id::new("logs-match-count"), Live::Polite);
    }
    let Some(shown) = c.steppable else { return };
    if ui.available_width() < 2.0 * h + 2.0 * gap {
        return;
    }
    for (back, icon, name, tip) in [
        (true, Icon::PrevChange, t.tr("logs.filter.previous.button"), t.tr("logs.filter.previous.tip")),
        (false, Icon::NextChange, t.tr("logs.filter.next.button"), t.tr("logs.filter.next.tip")),
    ] {
        let b = icon_tip(ghost(ui, ControlSize::Small, Some(icon), None, name, shown > 0), &st.keymap, t, Tip::new(name).body(tip));
        if b.clicked() && shown > 0 {
            msgs.push(Msg::Logs(LogsMsg::Step { back }));
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
