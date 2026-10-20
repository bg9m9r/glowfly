//! One terminal tab in the header's strip: a rounded pill with the shell icon, the tab's name,
//! an amber dot once the shell has ended, and a close slot whose × shows on the active pill and
//! on hover. Double-clicking renames it in place (`rename_field`).

use crate::msg::Msg;
use crate::state::terminal::{SessionStatus, TabRename, TermSession};
use crate::state::tip::Tip;
use crate::terminal::TerminalMsg;
use crate::ui::theme::{color, controls, type_scale};
use crate::ui::widgets::tab_glyph::{self, TabGlyph};
use crate::ui::widgets::truncate::{cut_galley, cut_job};
use crate::ui::widgets::{Icon, paint_icon_sized, tooltip};
use eframe::egui::{self, Align, CornerRadius, CursorIcon, Id, Key, Rect, Response, Sense, Ui, WidgetInfo, WidgetType, pos2, vec2};
use trellis_core::i18n::Lang;

pub(super) const HEIGHT: f32 = 28.0;
const RADIUS: u8 = 14;
const PAD_X: f32 = 10.0;
const ICON: f32 = 14.0;
const GAP: f32 = 6.0;
const DOT: f32 = 6.0;
const CLOSE: f32 = 16.0;
/// Names longer than this are cut with an ellipsis.
const NAME_MAX: f32 = 200.0;
/// The rename field is at least this wide.
const FIELD_MIN: f32 = 160.0;

fn ended(s: &TermSession) -> bool {
    matches!(s.status, SessionStatus::Exited(_) | SessionStatus::Failed(_))
}

fn name_galley(ui: &Ui, name: &str, col: egui::Color32) -> std::sync::Arc<egui::Galley> {
    cut_galley(ui.ctx(), cut_job(name, type_scale::font(type_scale::BODY), col), NAME_MAX)
}

/// The pill's width for `name`: the close slot is always counted, so it never jumps.
pub(super) fn width(ui: &Ui, s: &TermSession, name: &str) -> f32 {
    let dot = if ended(s) { GAP + DOT } else { 0.0 };
    PAD_X + ICON + GAP + name_galley(ui, name, color::text()).size().x + dot + GAP + CLOSE + PAD_X
}

/// The status line of the pill's tooltip.
fn status_line(t: Lang, status: &SessionStatus) -> String {
    match status {
        SessionStatus::Starting => t.tr("terminal.tab.starting").to_owned(),
        SessionStatus::Running => t.tr("terminal.tab.running").to_owned(),
        SessionStatus::Exited(Some(code)) => t.trf("terminal.tab.exited", &[("code", code)]),
        SessionStatus::Exited(None) => t.tr("terminal.tab.exited_unknown").to_owned(),
        SessionStatus::Failed(_) => t.tr("terminal.tab.failed").to_owned(),
    }
}

/// Draws session `s` as a pill named `name`; a click selects it, a double-click renames it and
/// its × closes it.
pub(super) fn show(
    ui: &mut Ui,
    s: &TermSession,
    name: &str,
    active: bool,
    keys: &crate::keymap::Keymap,
    t: Lang,
    msgs: &mut Vec<Msg>,
) -> Response {
    let (rect, _) = ui.allocate_exact_size(vec2(width(ui, s, name), HEIGHT), Sense::hover());
    let resp = ui.interact(rect, Id::new(("terminal-tab", s.id)), Sense::click());
    // The pill's own rect, so hovering its × still counts.
    let hovered = ui.rect_contains_pointer(rect);
    let (fill, fg) = if active {
        (Some(color::selected()), color::strong())
    } else if hovered {
        (Some(color::hover()), color::text())
    } else {
        (None, color::weak())
    };
    let p = ui.painter();
    if let Some(fill) = fill {
        p.rect_filled(rect, CornerRadius::same(RADIUS), fill);
    }
    let cy = rect.center().y;
    let mut x = rect.left() + PAD_X;
    paint_icon_sized(p, pos2(x + ICON / 2.0, cy), Icon::Terminal, fg, ICON);
    x += ICON + GAP;
    let galley = name_galley(ui, name, fg);
    let w = galley.size().x;
    p.galley(pos2(x, cy - galley.size().y / 2.0), galley, fg);
    x += w;
    if ended(s) {
        p.circle_filled(pos2(x + GAP + DOT / 2.0, cy), DOT / 2.0, color::warn());
    }
    let slot = Rect::from_center_size(pos2(rect.right() - PAD_X - CLOSE / 2.0, cy), vec2(CLOSE, CLOSE));
    if active || hovered {
        let label = t.trf("terminal.tab.close", &[("name", &name)]);
        let box_fill = if active { color::hover() } else { color::selected() };
        let close = tab_glyph::button_at(ui, Id::new(("terminal-tab-close", s.id)), slot.center(), TabGlyph::Close, &label, fg, box_fill);
        if close.clicked() {
            msgs.push(Msg::Terminal(TerminalMsg::CloseTab(s.id)));
        }
    }
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, name));
    let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
    let tip = || Tip::new(name).state(status_line(t, &s.status)).body(t.tr("terminal.tab.rename_hint"));
    let resp = tooltip::attach(resp, keys, t, tip);
    if resp.double_clicked() {
        msgs.push(Msg::Terminal(TerminalMsg::StartRename(s.id)));
    } else if resp.clicked() {
        msgs.push(Msg::Terminal(TerminalMsg::SelectTab(s.id)));
    }
    resp
}

/// The inline rename field in place of a pill `pill_w` wide: focused and fully selected when
/// it opens; Enter or leaving it renames, Esc cancels.
pub(super) fn rename_field(ui: &mut Ui, draft: &mut TabRename, pill_w: f32, msgs: &mut Vec<Msg>) {
    let id = Id::new(("terminal-tab-rename", draft.id));
    let w = pill_w.max(FIELD_MIN);
    let edit = egui::TextEdit::singleline(&mut draft.text)
        .id(id)
        .desired_width(w - 12.0)
        .min_size(vec2(w, HEIGHT))
        .vertical_align(Align::Center)
        .margin(vec2(6.0, 3.0));
    let r = controls::field(ui, edit);
    if draft.focus {
        r.request_focus();
        draft.focus = false;
    }
    if r.gained_focus() {
        // Selected whole, so typing replaces the name.
        let mut state = egui::text_edit::TextEditState::load(ui.ctx(), id).unwrap_or_default();
        let all = egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(draft.text.chars().count()));
        state.cursor.set_char_range(Some(all));
        state.store(ui.ctx(), id);
    }
    if r.lost_focus() {
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            msgs.push(Msg::Terminal(TerminalMsg::CancelRename));
        } else {
            let submitted = ui.input(|i| i.key_pressed(Key::Enter));
            msgs.push(Msg::Terminal(TerminalMsg::Rename { id: draft.id, name: draft.text.clone(), submitted }));
        }
    }
}
