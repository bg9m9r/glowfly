//! The conflict view's file bar, one 40 px row: at the left the path and "ours ← theirs"; in
//! the middle the conflict counter between its previous and next arrows, a progress meter and
//! "1 of 2 resolved"; at the right the more-actions menu (Save with markers, Open in merge tool,
//! Mark resolved as is, Reset picks…), the primary Mark resolved and a quiet ×. The middle
//! stays centred while it fits; otherwise it follows the left group, which gives way first (the
//! side names, then the path down to its file name), then the "resolved" text hides. With no
//! merge tool set, Open in merge tool stays enabled ("Set up…") and opens Preferences. Files
//! with no chunk view get only the path, the menu and ×.

use super::super::access;
use super::super::common::{menu_danger, menu_item_if, menu_item_setup};
use super::super::theme::color;
use super::super::widgets::menu::separator;
use super::super::widgets::reason::{self, ItemState};
use super::super::widgets::{self, Icon, buttons, icon_button};
use super::step_tip;
use crate::keymap::Keymap;
use crate::msg::{ConflictMsg, Msg};
use crate::state::conflict::ConflictView;
use crate::tools::ToolsMsg;
use crate::ui::theme::type_scale;
use eframe::egui::{self, Align, FontId, Layout, Rect, RichText, Sense, Stroke, Ui, UiBuilder, pos2, vec2};
use glowfly_core::i18n::Lang;
use glowfly_core::repo::ConflictContent;

/// The bar's height, its padding at each end and the gap between its items.
const BAR_H: f32 = 40.0;
const PAD: f32 = 12.0;
const GAP: f32 = 8.0;
/// The room the middle group keeps clear of the groups beside it.
const CLEAR: f32 = 16.0;
/// Between the counter's group and the meter.
const METER_GAP: f32 = 12.0;
const METER: egui::Vec2 = egui::vec2(64.0, 4.0);
/// A painted step arrow's width (`icon_button`).
const STEP_W: f32 = 24.0;
/// The painted "←" between the side names, with its margins.
const ARROW_W: f32 = 22.0;
/// The least room the side names keep before they hide.
const MIN_SIDES: f32 = 60.0;

/// The merge tool entry: why it can't open (if it can't), and its setup note while no merge
/// tool is set (it then opens Preferences > External Tools).
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct MergeTool<'a> {
    pub blocked: Option<&'a str>,
    pub setup: Option<&'static str>,
}

/// The width of `s` in `font`.
fn text_w(ui: &Ui, s: &str, font: &FontId) -> f32 {
    ui.fonts(|f| f.layout_no_wrap(s.to_owned(), font.clone(), color::text()).size().x)
}

/// The bar, with the merge `tool` entry in the menu; nothing writes the file while `in_tool`.
pub(super) fn header_bar(ui: &mut Ui, t: Lang, view: &ConflictView, tool: MergeTool, in_tool: bool, keys: &Keymap, msgs: &mut Vec<Msg>) {
    let (bar, _) = ui.allocate_exact_size(vec2(ui.available_width(), BAR_H), Sense::hover());
    ui.painter().rect_filled(bar, 0.0, color::panel());
    ui.painter().hline(bar.x_range(), bar.bottom() - 0.5, Stroke::new(1.0_f32, color::border()));
    let inner = bar.shrink2(vec2(PAD, 0.0));
    let text = view.file.as_ref().is_some_and(|f| matches!(f.content, ConflictContent::Text(_)));
    let mut right = ui.new_child(UiBuilder::new().max_rect(inner).layout(Layout::right_to_left(Align::Center)));
    right.spacing_mut().item_spacing.x = GAP;
    trailing(&mut right, t, view, text, tool, in_tool, msgs);
    let trail_left = right.min_rect().left();

    let fonts = (type_scale::strong(ui, type_scale::BODY), FontId::proportional(type_scale::BODY));
    let labels = view.file.as_ref().map(|f| (f.labels.ours.as_str(), f.labels.theirs.as_str()));
    let sides_w = labels.map_or(0.0, |(o, th)| GAP + text_w(ui, o, &fonts.1) + ARROW_W + text_w(ui, th, &fonts.1));
    let left_w = text_w(ui, &view.path, &fonts.0) + sides_w;
    let mut left_room = trail_left - CLEAR - inner.left();
    if text {
        let mut m = Middle::of(t, view, |s| text_w(ui, s, &fonts.1));
        let centred = bar.center().x - m.width() / 2.0;
        let x = if centred >= inner.left() + left_w + CLEAR && centred + m.width() <= trail_left - CLEAR {
            centred
        } else {
            // The left group gives way down to the file name; below that the text hides.
            let least = text_w(ui, &format!("\u{2026}/{}", file_name(&view.path)), &fonts.0).min(left_w);
            if trail_left - CLEAR - m.width() - CLEAR - inner.left() < least {
                m.text_w = None;
            }
            (inner.left() + left_w + CLEAR).min(trail_left - CLEAR - m.width()).max(inner.left())
        };
        left_room = x - CLEAR - inner.left();
        let r = Rect::from_min_max(pos2(x, inner.top()), pos2(trail_left, inner.bottom()));
        let mut mid = ui.new_child(UiBuilder::new().max_rect(r).layout(Layout::left_to_right(Align::Center)));
        mid.spacing_mut().item_spacing.x = 0.0;
        m.show(&mut mid, t, view, keys, msgs);
    }
    let r = Rect::from_min_max(inner.min, pos2(inner.left() + left_room.max(0.0), inner.bottom()));
    let mut left = ui.new_child(UiBuilder::new().max_rect(r).layout(Layout::left_to_right(Align::Center)));
    left.spacing_mut().item_spacing.x = 0.0;
    left_group(&mut left, t, &view.path, labels, fonts, left_room);
}

/// The part of `path` after its last `/`.
fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// The path in `strong`, then "ours ← theirs" in `weak`, in `room`: the side names truncate
/// first and hide below `MIN_SIDES`, then the path keeps only its file name after "…/".
fn left_group(ui: &mut Ui, t: Lang, full: &str, labels: Option<(&str, &str)>, (path_font, body): (FontId, FontId), room: f32) {
    let full_w = text_w(ui, full, &path_font);
    let path = if full_w <= room || !full.contains('/') { full.to_owned() } else { format!("\u{2026}/{}", file_name(full)) };
    let path_w = text_w(ui, &path, &path_font).min(room.max(0.0));
    let r = Rect::from_min_size(ui.cursor().min, vec2(path_w, ui.available_height()));
    let shown = ui.put(r, egui::Label::new(RichText::new(&path).font(path_font.clone()).color(color::strong())).truncate());
    if path != full || text_w(ui, &path, &path_font) > path_w {
        shown.on_hover_text(full);
    }
    let sides_room = room - path_w - GAP;
    let Some((ours, theirs)) = labels.filter(|_| sides_room >= MIN_SIDES) else { return };
    ui.add_space(GAP);
    let start = ui.cursor().min;
    let (ours_full, theirs_full) = (text_w(ui, ours, &body), text_w(ui, theirs, &body));
    let ours_w = ours_full.min(sides_room - ARROW_W - 8.0).max(0.0);
    let r = Rect::from_min_size(ui.cursor().min, vec2(ours_w, ui.available_height()));
    ui.put(r, egui::Label::new(RichText::new(ours).font(body.clone()).color(color::weak())).truncate());
    let (arrow, _) = ui.allocate_exact_size(vec2(ARROW_W, ui.available_height()), Sense::hover());
    paint_left_arrow(ui, arrow);
    let room = ui.available_width();
    let theirs_w = if room > 8.0 { theirs_full.min(room) } else { 0.0 };
    if theirs_w > 0.0 {
        let r = Rect::from_min_size(ui.cursor().min, vec2(theirs_w, ui.available_height()));
        ui.put(r, egui::Label::new(RichText::new(theirs).font(body).color(color::weak())).truncate());
    }
    // Cut short: both names whole on hover (the fonts lack the arrow, so in words).
    if ours_w < ours_full || theirs_w < theirs_full {
        let all = Rect::from_min_size(start, vec2(ours_w + ARROW_W + theirs_w, ui.available_height()));
        let (o, th) = (t.tr("conflict.panes.ours"), t.tr("conflict.panes.theirs"));
        ui.interact(all, ui.id().with("conflict-sides"), Sense::hover()).on_hover_text(format!("{o}: {ours} · {th}: {theirs}"));
    }
}

/// A 10 px "←" in `weak`, 1.5 px, centred in `r` (the bundled fonts lack the glyph).
fn paint_left_arrow(ui: &Ui, r: Rect) {
    let (c, s) = (r.center(), Stroke::new(1.5_f32, color::weak()));
    let p = ui.painter();
    p.line_segment([pos2(c.x - 5.0, c.y), pos2(c.x + 5.0, c.y)], s);
    p.line_segment([pos2(c.x - 5.0, c.y), pos2(c.x - 1.5, c.y - 3.5)], s);
    p.line_segment([pos2(c.x - 5.0, c.y), pos2(c.x - 1.5, c.y + 3.5)], s);
}

/// The middle group's texts and widths.
struct Middle {
    n: usize,
    counter: String,
    counter_w: f32,
    progress: String,
    /// The "resolved" text's width; None once it hides.
    text_w: Option<f32>,
}

impl Middle {
    fn of(t: Lang, view: &ConflictView, w: impl Fn(&str) -> f32) -> Self {
        let n = view.chunk_count();
        let counter = match n {
            0 => t.tr("conflict.header.no_lines").to_owned(),
            1 => t.tr("conflict.header.counter.single").to_owned(),
            _ => t.trf("conflict.header.counter.of", &[("current", &(view.current + 1)), ("total", &n)]),
        };
        let progress = t.trn("conflict.header.progress", n, &[("done", &view.picked())]);
        Self { n, counter_w: w(&counter), text_w: Some(w(&progress)), counter, progress }
    }

    fn width(&self) -> f32 {
        let steps = if self.n > 1 { 2.0 * (STEP_W + GAP) } else { 0.0 };
        let meter = if self.n > 0 { METER_GAP + METER.x + self.text_w.map_or(0.0, |w| GAP + w) } else { 0.0 };
        steps + self.counter_w + meter
    }

    /// Left to right: ↑, the counter, ↓, the meter and the text (no arrows for one conflict,
    /// only "No conflicting lines" for none).
    fn show(&self, ui: &mut Ui, t: Lang, view: &ConflictView, keys: &Keymap, msgs: &mut Vec<Msg>) {
        let n = self.n;
        if n == 0 {
            ui.label(RichText::new(&self.counter).color(color::weak()));
            return;
        }
        if n > 1 {
            let tip = step_tip(keys, t, false);
            if ui.add_enabled_ui(view.current > 0, |ui| icon_button(ui, Icon::PrevChange, &tip)).inner.clicked() {
                msgs.push(Msg::Conflict(ConflictMsg::ConflictStep(-1)));
            }
            ui.add_space(GAP);
        }
        ui.label(RichText::new(&self.counter).color(color::text()));
        if n > 1 {
            ui.add_space(GAP);
            let tip = step_tip(keys, t, true);
            if ui.add_enabled_ui(view.current + 1 < n, |ui| icon_button(ui, Icon::NextChange, &tip)).inner.clicked() {
                msgs.push(Msg::Conflict(ConflictMsg::ConflictStep(1)));
            }
        }
        ui.add_space(METER_GAP);
        let done = view.picked();
        let (meter, _) = ui.allocate_exact_size(METER, Sense::hover());
        let p = ui.painter();
        p.rect_filled(meter, 2.0, color::border());
        let fill = if done == n { color::added() } else { color::warn() };
        p.rect_filled(Rect::from_min_size(meter.min, vec2(METER.x * done as f32 / n as f32, METER.y)), 2.0, fill);
        if self.text_w.is_some() {
            ui.add_space(GAP);
            let tone = if done == n { color::added() } else { color::weak() };
            ui.label(RichText::new(&self.progress).color(tone));
        }
    }
}

/// Right to left: ×, Mark resolved (text files) and the more-actions menu.
fn trailing(ui: &mut Ui, t: Lang, view: &ConflictView, text: bool, tool: MergeTool, in_tool: bool, msgs: &mut Vec<Msg>) {
    if ui.add(buttons::icon_only(Icon::Close, t.tr("conflict.header.close.tip"))).clicked() {
        msgs.push(Msg::Conflict(ConflictMsg::CloseConflict));
    }
    if text {
        let can = view.can_save() && !in_tool;
        let why = if in_tool {
            t.tr("tools.in_merge_tool").to_owned()
        } else if view.saving {
            t.tr("conflict.header.saving.tip").to_owned()
        } else {
            let first = view.picks.iter().position(|p| p.is_open()).unwrap_or(0) + 1;
            t.trf("conflict.header.save.pick_side.tip", &[("number", &first)])
        };
        let save = ui.add(buttons::primary(t.tr("conflict.header.save.button")).enabled(can).why(Some(&why)));
        let save = if can { save.on_hover_text(t.tr("conflict.header.save.tip")) } else { save };
        if save.clicked() {
            msgs.push(Msg::Conflict(ConflictMsg::SaveConflict));
        }
    }
    more_menu(ui, t, view, text, tool, in_tool, msgs);
}

/// The painted more-actions button and its menu, each item disabled with its reason. Save with
/// markers and Reset picks… are for text files (`text`) only.
fn more_menu(ui: &mut Ui, t: Lang, view: &ConflictView, text: bool, tool: MergeTool, in_tool: bool, msgs: &mut Vec<Msg>) {
    let path = &view.path;
    let menu = crate::ui::widgets::menu::custom_button(ui, egui::Button::new("").min_size(vec2(28.0, 28.0)), |ui| {
        if text {
            menu_item_if(
                msgs,
                ui,
                t.tr("conflict.header.markers.button"),
                markers_blocked(t, view, in_tool),
                Msg::Conflict(ConflictMsg::SaveConflictKeepMarkers),
            );
        }
        let open = Msg::Tools(ToolsMsg::ResolveInMergeTool(path.clone()));
        match tool.blocked {
            Some(why) => menu_item_if(msgs, ui, t.tr("tools.merge_tool.button"), Some(why), open),
            None => menu_item_setup(msgs, ui, t, t.tr("tools.merge_tool.button"), tool.setup, open),
        }
        // Stages the file as it is on disk, without the picks: said on hover.
        let blocked = in_tool.then(|| t.tr("tools.in_merge_tool"));
        let item = reason::menu_button(ui, t.tr("conflict.header.mark_resolved.button"), "", ItemState::blocked_by(blocked));
        let item = if blocked.is_none() { item.on_hover_text(t.tr("conflict.header.mark_resolved.tip")) } else { item };
        if item.clicked() {
            msgs.push(Msg::Conflict(ConflictMsg::MarkResolved(vec![path.clone()])));
            ui.close_menu();
        }
        // It asks before clearing the picks: last, set apart.
        if text {
            let blocked = if in_tool {
                Some(t.tr("tools.in_merge_tool"))
            } else if view.saving {
                Some(t.tr("conflict.header.saving.tip"))
            } else {
                None
            };
            separator(ui);
            menu_danger(msgs, ui, t.tr("conflict.header.reset.button"), blocked, view.reset_msg(t));
        }
    });
    let r = menu.response.on_hover_text(t.tr("conflict.header.more.tip"));
    widgets::paint_icon(ui.painter(), r.rect.center(), Icon::More, color::text());
    access::name_button(&r, true, t.tr("conflict.header.more.tip"));
}

/// Why Save with markers can't run: it needs something picked or edited and something left.
fn markers_blocked(t: Lang, view: &ConflictView, in_tool: bool) -> Option<&'static str> {
    if in_tool {
        Some(t.tr("tools.in_merge_tool"))
    } else if view.saving {
        Some(t.tr("conflict.header.saving.tip"))
    } else if view.unresolved() == 0 {
        Some(t.tr("conflict.header.markers.all_picked.tip"))
    } else if !view.can_save_partial() {
        Some(t.tr("conflict.header.markers.pick_first.tip"))
    } else {
        None
    }
}

#[cfg(test)]
#[path = "snapshot_tests/snapshot_tests_conflict_header.rs"]
mod snapshot_tests;
