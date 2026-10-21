//! The commit pairs: 56 px painted rows in git's order, laid out only where visible. Line 1 is
//! the status chip (Same, Changed, Dropped, New), the commit's summary and, on a changed pair,
//! the interdiff's +N −M; line 2 is each version's index and short SHA with a painted arrow
//! between them ("none" for a missing side). A click selects the pair.

use crate::msg::Msg;
use crate::range_diff::RangeDiffMsg;
use crate::state::AppState;
use crate::state::journal::short;
use crate::state::range_diff::status_label;
use crate::ui::common::{added_color, modified_color, removed_color};
use crate::ui::theme::tokens::radius;
use crate::ui::theme::{color, contrast, type_scale};
use crate::ui::widgets::{Icon, paint_cut_text, paint_icon, row_fill};
use eframe::egui::{self, Color32, CornerRadius, CursorIcon, Painter, Rect, Sense, Ui, pos2, vec2};
use glowfly_core::i18n::Lang;
use glowfly_core::range_diff::{Pair, Side, Status};

const ROW_H: f32 = 56.0;
const PAD_X: f32 = 16.0;
const ARROW_W: f32 = 20.0;

pub(super) fn show(ui: &mut Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let Some(view) = &st.range_diff else { return };
    let Some(c) = view.compared() else { return };
    let t = st.settings.lang();
    egui::ScrollArea::vertical().id_salt("range-diff-pairs").auto_shrink([false, false]).show_rows(
        ui,
        ROW_H,
        c.pairs.len(),
        |ui, range| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for i in range {
                let pair = &c.pairs[i];
                let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
                let name = format!("{}: {}", status_label(t, pair.status), pair.summary());
                crate::ui::access::name_button(&resp, true, &name);
                if ui.is_rect_visible(rect) {
                    row_fill::paint(ui.painter(), rect, view.selected == Some(i), resp.hovered());
                    paint(ui.painter(), t, rect, pair);
                }
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }
                if resp.clicked() {
                    msgs.push(Msg::RangeDiff(RangeDiffMsg::Select(i)));
                }
                ui.painter().hline(rect.x_range(), rect.bottom() - 0.5, (1.0, color::border()));
            }
        },
    );
}

/// The chip's colour: weak for Same, else the modified, removed or added colour.
fn tone(s: Status) -> Color32 {
    match s {
        Status::Unchanged => color::weak(),
        Status::Changed => modified_color(),
        Status::Removed => removed_color(),
        Status::Added => added_color(),
    }
}

fn paint(p: &Painter, t: Lang, rect: Rect, pair: &Pair) {
    let (left, right) = (rect.left() + PAD_X, rect.right() - PAD_X);
    let (y1, y2) = (rect.top() + 18.0, rect.top() + 39.0);
    let chip = chip(p, left, y1, status_label(t, pair.status), tone(pair.status));
    let mut end = right;
    if pair.status == Status::Changed {
        end = crate::ui::line_counts::paint_total_right(p, pos2(right, y1), pair.counts()).left() - 12.0;
    }
    let x = chip.right() + 8.0;
    paint_cut_text(p, pos2(x, y1), pair.summary(), type_scale::font(type_scale::BODY), color::text(), (end - x).max(0.0));

    let mono = type_scale::mono(type_scale::SMALL);
    let side = |s: &Option<Side>| match s {
        Some(s) => format!("#{} {}", s.index, short(s.oid)),
        None => t.tr("range_diff.side.none").to_owned(),
    };
    let old = paint_cut_text(p, pos2(left, y2), &side(&pair.old), mono.clone(), color::weak(), (right - left).max(0.0));
    let arrow_x = old.rect.right() + 6.0;
    paint_icon(p, pos2(arrow_x + ARROW_W / 2.0, y2), Icon::Into, color::weak());
    let x = arrow_x + ARROW_W + 6.0;
    paint_cut_text(p, pos2(x, y2), &side(&pair.new), mono, color::weak(), (right - x).max(0.0));
}

/// A small status chip at `x` on the line centred at `y`, filled and inked for `col`; returns
/// its rect.
fn chip(p: &Painter, x: f32, y: f32, text: &str, col: Color32) -> Rect {
    const H: f32 = 18.0;
    const PAD: f32 = 6.0;
    let (fill, ink) = contrast::change_chip(col, &color::current());
    let galley = p.layout_no_wrap(text.to_owned(), type_scale::font(type_scale::SMALL), ink);
    let rect = Rect::from_min_size(pos2(x, y - H / 2.0), vec2(galley.size().x + 2.0 * PAD, H));
    p.rect_filled(rect, CornerRadius::same(radius::SM), fill);
    p.galley(pos2(x + PAD, y - galley.size().y / 2.0), galley, ink);
    rect
}
