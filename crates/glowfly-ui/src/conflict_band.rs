//! A conflict's band across the ours and theirs panes, one column at a time: "Conflict N" (with
//! a faint tag when its pick was made automatically or its text typed in the output) and a Take
//! checkbox in its side's colour. Amber marks what is still to resolve: an unresolved band has
//! a 3 px amber mark at its left; a resolved one goes neutral. The current band takes the
//! selection fill and a strong title. With both sides taken, the ours column says their order
//! ("Ours, then theirs") with a quiet Swap. A chunk edited in the output has Discard edit in
//! the theirs column instead of Take.

use super::conflict_cells::side_color;
use super::diff::mono;
use super::theme::color;
use crate::theme::controls;
use crate::theme::type_scale;
use eframe::egui::{self, Align2, FontId, Rect, RichText, Sense, Ui, pos2, vec2};
use glowfly_core::conflict_pick::ChunkPick;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{Chunk, Side};
use glowfly_model::msg::{ConflictMsg, Msg};
use glowfly_model::state::conflict::ConflictView;

/// The width of a band's Take checkbox and of its Discard edit button, for their English text;
/// a longer translation widens them by the difference.
const TAKE_W: f32 = 64.0;
const DISCARD_W: f32 = 90.0;
/// The height of Take's hit area.
const TAKE_HIT: f32 = 24.0;
/// The unresolved mark's width at the band's left.
const MARK_W: f32 = 3.0;
/// Before the order text, and between it and Swap.
const ORDER_GAP: f32 = 12.0;
const SWAP_GAP: f32 = 8.0;
/// Swap's padding at each side.
const SWAP_PAD: f32 = 6.0;
/// Between the title and the tag.
const TAG_GAP: f32 = 12.0;

/// Chunk `k`'s band in the column `r` of `side`.
pub(super) fn band(ui: &mut Ui, t: Lang, r: Rect, view: &ConflictView, k: usize, side: Side, msgs: &mut Vec<Msg>) {
    let (Some(pick), Some(c)) = (view.picks.get(k), view.chunk(k)) else { return };
    let (open, current) = (pick.is_open(), k == view.current);
    let p = ui.painter().with_clip_rect(r.intersect(ui.clip_rect()));
    let fill = if current {
        color::selected()
    } else if open {
        color::hunk_bg()
    } else {
        color::raised()
    };
    p.rect_filled(r, 0.0, fill);
    if open {
        p.rect_filled(Rect::from_min_size(r.min, vec2(MARK_W, r.height())), 0.0, color::warn());
    }
    let ink = if current {
        color::strong()
    } else if open {
        color::hunk_fg()
    } else {
        color::weak()
    };
    let name = t.trf("conflict.panes.chunk.title", &[("number", &(k + 1))]);
    let title = p.text(pos2(r.left() + 12.0, r.center().y), Align2::LEFT_CENTER, name, mono(), ink);
    let edited = pick.is_edited();
    // Kept in view when the panes scroll sideways.
    let x = r.right().min(ui.clip_rect().right()) - 8.0;
    let (label, english) = if edited {
        (t.tr("conflict.panes.discard_edit.button"), Lang::En.tr("conflict.panes.discard_edit.button"))
    } else {
        (t.tr("conflict.panes.take.check"), Lang::En.tr("conflict.panes.take.check"))
    };
    let body = FontId::proportional(type_scale::BODY);
    let text_w = |s: &str, f: &FontId| ui.fonts(|fo| fo.layout_no_wrap(s.to_owned(), f.clone(), color::text()).size().x);
    let grow = (text_w(label, &body) - text_w(english, &body)).max(0.0);
    let w = if edited { DISCARD_W } else { TAKE_W } + grow;
    let check = Rect::from_min_max(pos2(x - w, r.top()), pos2(x, r.bottom()));
    // Take's hit area is 24 px tall, past the band's row; its box stays 14 px.
    let take_at = Rect::from_center_size(check.center(), vec2(w, TAKE_HIT));
    // The tag (full, then short, then none) and, in the ours column with both sides taken, the
    // order and Swap: the order text gives way first, Swap never.
    let tag = match view.auto.get(k).copied().flatten() {
        _ if edited => Some((t.tr("conflict.panes.edited.tag").to_owned(), None, t.tr("conflict.panes.edited.tip").to_owned())),
        Some(why) => Some((
            t.trf("conflict.panes.auto.tag", &[("why", &why.label(t))]),
            Some(t.tr("conflict.panes.auto.short")),
            t.trf("conflict.panes.auto.tip", &[("why", &why.label(t))]),
        )),
        None => None,
    };
    let order = (side == Side::Ours && !edited && pick.takes_both(c)).then(|| Order::of(ui, t, &view.picks[k], c));
    let start = title.right() + TAG_GAP;
    let room = check.left() - 8.0 - start;
    let tag_w = |s: &str| text_w(s, &mono()) + TAG_GAP;
    let (full, short) = match &tag {
        Some((f, s, _)) => (Some(f.as_str()), s.map(|s| s.to_owned())),
        None => (None, None),
    };
    let tries: [(Option<&str>, bool); 4] = [(full, true), (short.as_deref(), true), (short.as_deref().or(full), false), (None, false)];
    let (shown, with_text) = tries
        .into_iter()
        .find(|(s, text)| s.map_or(0.0, tag_w) + order.as_ref().map_or(0.0, |o| o.width(*text)) <= room)
        .unwrap_or((None, false));
    if let (Some(s), Some((_, _, tip))) = (shown, &tag) {
        let at = Rect::from_min_size(pos2(start, r.top()), vec2(text_w(s, &mono()), r.height()));
        p.text(pos2(start, r.center().y), Align2::LEFT_CENTER, s, mono(), color::faint());
        ui.interact(at, ui.id().with(("conflict-tag", k, side)), Sense::hover()).on_hover_text(tip);
    }
    if let Some(o) = order {
        let from = start + shown.map_or(0.0, tag_w) - TAG_GAP + ORDER_GAP;
        o.show(ui, k, r, from, with_text, msgs);
    }
    if edited {
        if side == Side::Theirs {
            let b = ui.put(check, egui::Button::new(RichText::new(label).size(type_scale::SMALL)));
            if b.on_hover_text(t.tr("conflict.panes.discard_edit.tip")).clicked() {
                msgs.push(Msg::Conflict(ConflictMsg::ConflictDiscardEdit(k)));
            }
        }
        return;
    }
    let mut taken = pick.takes_side(c, side);
    let branch = super::conflict_panes::side_name(t, side, view.file.as_ref().map(|f| &f.labels));
    let args: [(&str, &dyn std::fmt::Display); 2] = [("number", &(k + 1)), ("branch", &branch)];
    let name = match side {
        Side::Ours => t.trf("conflict.panes.take.ours.name", &args),
        Side::Theirs => t.trf("conflict.panes.take.theirs.name", &args),
    };
    let check_box = controls::Check::new(&mut taken, label)
        .indeterminate(pick.partly_takes_side(c, side))
        .tint(side_color(side))
        .hit(vec2(0.0, TAKE_HIT))
        .name(name);
    // A child that doesn't grow the band's row to the taller hit area.
    let mut child =
        ui.new_child(egui::UiBuilder::new().max_rect(take_at).layout(egui::Layout::centered_and_justified(egui::Direction::TopDown)));
    if child.add(check_box).changed() {
        msgs.push(Msg::Conflict(ConflictMsg::ConflictToggle { chunk: k, side }));
    }
}

/// "Ours, then theirs" in `weak` and Swap, frameless in the band's row (a styled button needs a
/// 28 px hit area): `text` colour, 6 px padding, a `hover` fill with radius 4 and the pointer.
struct Order {
    text: &'static str,
    text_w: f32,
    swap: &'static str,
    swap_w: f32,
    tip: &'static str,
}

impl Order {
    fn of(ui: &Ui, t: Lang, pick: &ChunkPick, c: &Chunk) -> Self {
        let (text, tip) = if pick.first_side(c) == Side::Theirs {
            (t.tr("conflict.panes.order.theirs_first"), t.tr("conflict.panes.swap.ours_first.tip"))
        } else {
            (t.tr("conflict.panes.order.ours_first"), t.tr("conflict.panes.swap.theirs_first.tip"))
        };
        let swap = t.tr("conflict.panes.swap.button");
        let w = |s: &str, size| ui.fonts(|f| f.layout_no_wrap(s.to_owned(), FontId::proportional(size), color::text()).size().x);
        Self { text, text_w: w(text, type_scale::BODY), swap, swap_w: w(swap, type_scale::SMALL) + 2.0 * SWAP_PAD, tip }
    }

    /// Its width from the gap before it, with the text or (`text` false) Swap alone.
    fn width(&self, text: bool) -> f32 {
        ORDER_GAP + if text { self.text_w + SWAP_GAP } else { 0.0 } + self.swap_w
    }

    fn show(&self, ui: &mut Ui, k: usize, r: Rect, from: f32, text: bool, msgs: &mut Vec<Msg>) {
        let mut x = from;
        if text {
            // A label, so screen readers hear the order too.
            let at = Rect::from_min_size(pos2(x, r.top()), vec2(self.text_w, r.height()));
            ui.put(at, egui::Label::new(RichText::new(self.text).size(type_scale::BODY).color(color::weak())).selectable(false));
            x += self.text_w + SWAP_GAP;
        }
        let rect = Rect::from_min_size(pos2(x, r.top()), vec2(self.swap_w, r.height()));
        let resp = ui.interact(rect, ui.id().with(("conflict-swap", k)), Sense::click());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, self.swap));
        let p = ui.painter().with_clip_rect(r.intersect(ui.clip_rect()));
        if resp.hovered() {
            p.rect_filled(rect, 4.0, color::hover());
        }
        p.text(rect.center(), Align2::CENTER_CENTER, self.swap, FontId::proportional(type_scale::SMALL), color::text());
        if resp.has_focus() {
            crate::theme::tokens::focus_ring(ui.painter(), rect, 4);
        }
        if resp.on_hover_text(self.tip).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            msgs.push(Msg::Conflict(ConflictMsg::ConflictSwap(k)));
        }
    }
}
