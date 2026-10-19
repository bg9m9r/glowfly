//! The blame's lines in a scroll area of their own, only those in view laid out. Rows are one
//! height, or with Wrap lines on, as tall as their wrapped line (`RowMetrics`, measured once
//! per blame in `UiCache::blame_rows`). Unwrapped, long lines scroll sideways while the gutter
//! and the line numbers stay at the left, the code clipped at their edge.

use super::gutter::{self, Fonts, GUTTER_W, PAD};
use super::{menu, target};
use crate::msg::{BlameMsg, Msg};
use crate::state::AppState;
use crate::state::blame::{BlameData, BlameLanding, BlameView};
use crate::state::blame_find::ranges;
use crate::ui::UiCache;
use crate::ui::common::short;
use crate::ui::diff::wrap::{MIN_COLS, RowMetrics, Wrap};
use crate::ui::diff::{DiffStyle, paint_line};
use crate::ui::theme::{color, type_scale};
use eframe::egui::{self, Align2, CursorIcon, FontId, Rect, Sense, Ui, WidgetInfo, WidgetType, pos2, vec2};

/// What the body saw this frame: the first line in view and a code line clicked.
pub(super) struct BodyOut {
    pub top: usize,
    pub clicked: Option<usize>,
}

/// Where the scroll area starts this frame for `land` (None to leave it), for `n` rows placed
/// by `m`.
pub(super) fn offset(land: Option<BlameLanding>, m: &RowMetrics, n: usize, viewport_h: f32) -> Option<f32> {
    let l = land?;
    let line = l.line.min(n.saturating_sub(1));
    let y = m.top(line);
    Some(if l.center { (y - viewport_h / 2.0 + m.height(line) / 2.0).max(0.0) } else { y })
}

/// The lines, only those in view laid out.
/// The gutter's text size for code at `code_px`: the small step, less only with the smallest
/// code fonts, and never under the type scale's floor.
fn label_px(code_px: f32) -> f32 {
    type_scale::fit(type_scale::SMALL, code_px + 1.0)
}

pub(super) fn body(
    ui: &mut Ui,
    st: &AppState,
    cache: &mut UiCache,
    view: &BlameView,
    data: &BlameData,
    land: Option<BlameLanding>,
    msgs: &mut Vec<Msg>,
) -> BodyOut {
    let style = DiffStyle::new(&st.settings);
    let blame = &data.blame;
    let n = blame.lines.len();
    let (char_w, line_h, digit_w) =
        ui.fonts(|f| (f.glyph_width(&style.font, 'M'), f.row_height(&style.font), f.glyph_width(&style.font, '0')));
    let num_w = digit_w * n.to_string().len() as f32;
    // The code's left edge, from the content's left.
    let code_off = GUTTER_W + PAD + num_w + 12.0;
    let avail_w = ui.available_width();
    let scroll = &ui.spacing().scroll;
    let bar = scroll.allocated_width().max(scroll.bar_width);
    let wrap = st.settings.blame.wrap_lines.then(|| {
        let cols = if char_w > 0.0 { ((avail_w - bar - code_off - 8.0) / char_w).floor().max(0.0) as usize } else { 0 };
        Wrap { cols: cols.max(MIN_COLS), line_h }
    });
    let rc = &mut cache.blame_rows;
    rc.refresh(view.data_gen, &blame.lines, style.tab, style.row_h, wrap);
    let m = if wrap.is_some() { RowMetrics::Varied(&rc.offsets) } else { RowMetrics::Uniform(style.row_h) };
    let content_w = if wrap.is_some() { avail_w } else { avail_w.max(code_off + rc.widest as f32 * char_w + 40.0) };

    let small = label_px(style.font.size);
    let fonts = Fonts { name: FontId::proportional(small), sha: FontId::monospace(small) };
    ui.spacing_mut().item_spacing.y = 0.0;
    let mut out = BodyOut { top: view.top_line, clicked: None };
    let mut area = egui::ScrollArea::both().id_salt(("blame", &view.path, view.at)).auto_shrink([false, false]);
    if let Some(y) = offset(land, &m, n, ui.available_height()) {
        area = area.vertical_scroll_offset(y);
    }
    let marks_col = color::warn().gamma_multiply(0.45);
    let menu_id = ui.id().with("blame-gutter-menu-line");
    area.show_viewport(ui, |ui, vp| {
        ui.set_min_size(vec2(content_w, m.total(n)));
        if n == 0 {
            return;
        }
        let first = m.row_containing(vp.min.y, n);
        let end = (m.row_containing(vp.max.y, n) + 2).min(n);
        out.top = first;
        // The content's corner moves with the scroll; `sticky` is the left edge in view.
        let origin = ui.max_rect().min;
        let sticky = origin.x + vp.min.x;
        let row = |i: usize| Rect::from_min_size(pos2(origin.x, origin.y + m.top(i)), vec2(content_w, m.height(i)));
        let first_run = blame.run_of(first).unwrap_or(0);
        let last_run = blame.run_of(end - 1).unwrap_or(first_run);
        let num_right = sticky + GUTTER_W + PAD + num_w;
        let painter = ui.painter().clone();
        // Run shading across the whole row, then find hits and the cursor line.
        for i in first..end {
            let r = row(i);
            let shade = if blame.run_of(i).unwrap_or(0).is_multiple_of(2) { color::bg() } else { color::panel() };
            painter.rect_filled(r, 0.0, shade);
            if view.cursor == Some(i) {
                painter.rect_filled(r, 0.0, color::selected());
                let bar = Rect::from_min_size(pos2(sticky + GUTTER_W + 1.0, r.top()), vec2(3.0, style.row_h));
                painter.rect_filled(bar, 0.0, color::accent());
            } else if view.find.hits.binary_search(&i).is_ok() {
                painter.rect_filled(r, 0.0, color::warn().gamma_multiply(0.08));
            }
        }
        for (k, run) in blame.runs.iter().enumerate().take(last_run + 1).skip(first_run) {
            let Some(c) = blame.commits.get(run.commit) else { continue };
            let (from, to) = (run.start.max(first), (run.start + run.len).min(end));
            if from >= to {
                continue;
            }
            let gutter = Rect::from_min_max(pos2(sticky, row(from).top()), pos2(sticky + GUTTER_W, row(to - 1).bottom()));
            gutter::paint_age(&painter, gutter, data.age(run.commit));
            let resp = ui.interact(gutter, ui.id().with(("blame-run", k)), Sense::click());
            let label = format!("Select {} on the graph", short(c.oid));
            resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, &label));
            if resp.hovered() {
                painter.rect_filled(gutter, 0.0, color::hover());
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
            if run.start > 0 && run.start >= first {
                let r = row(run.start);
                painter.hline(r.x_range(), r.top() + 0.5, (1.0, color::border()));
            }
            // The author, date and commit on the first visual row, the summary on the one below
            // (of the next line or of the same wrapped line) while the run is that tall.
            let first_row = Rect::from_min_size(pos2(sticky, row(from).top()), vec2(GUTTER_W, style.row_h));
            let second = first_row.translate(vec2(0.0, style.row_h));
            gutter::text(ui, st, &fonts, c, first_row, (second.bottom() <= gutter.bottom() + 0.5).then_some(second));
            // The menu acts on the line under the pointer when the right-click happened.
            if resp.secondary_clicked()
                && let Some(p) = resp.interact_pointer_pos()
            {
                let line = m.row_containing(p.y - origin.y, n).clamp(from, to - 1);
                ui.ctx().data_mut(|d| d.insert_temp(menu_id, line));
            }
            let resp = resp.on_hover_text(gutter::tooltip(st, c));
            if resp.clicked() {
                msgs.push(Msg::Blame(BlameMsg::SelectCommit(c.oid)));
            }
            resp.context_menu(|ui| {
                let line = ui.ctx().data(|d| d.get_temp::<usize>(menu_id)).unwrap_or(from).clamp(run.start, run.start + run.len - 1);
                if let Some(t) = target(st, view, data, line) {
                    menu::line_menu(ui, msgs, &t);
                }
            });
        }
        painter.vline(sticky + GUTTER_W, row(first).top()..=row(end - 1).bottom(), (1.0, color::border()));
        for i in first..end {
            let r = row(i);
            let y = r.top() + style.row_h / 2.0;
            painter.text(pos2(num_right, y), Align2::RIGHT_CENTER, (i + 1).to_string(), style.font.clone(), color::faint());
            let code = Rect::from_min_max(pos2(sticky + GUTTER_W + 1.0, r.top()), r.max);
            let resp = ui.interact(code, ui.id().with(("blame-line", i)), Sense::click());
            if resp.clicked() || resp.secondary_clicked() {
                out.clicked = Some(i);
            }
            resp.context_menu(|ui| {
                if let Some(t) = target(st, view, data, i) {
                    menu::line_menu(ui, msgs, &t);
                }
            });
            let clip = Rect::from_min_max(pos2(sticky + code_off, r.top()), r.max).intersect(ui.clip_rect());
            let code_painter = painter.with_clip_rect(clip);
            let line = &blame.lines[i];
            let found = if view.find.query.is_empty() { Vec::new() } else { ranges(line, &view.find.query) };
            let marks = (!found.is_empty()).then_some((found.as_slice(), marks_col));
            let at = pos2(origin.x + code_off, y);
            paint_line(&code_painter, at, line, data.spans(i), marks, &style.font, color::text(), style.tab, wrap);
        }
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn top(line: usize) -> Option<BlameLanding> {
        Some(BlameLanding { line, center: false })
    }

    fn centre(line: usize) -> Option<BlameLanding> {
        Some(BlameLanding { line, center: true })
    }

    #[test]
    fn gutter_text_stays_on_the_scale() {
        assert_eq!(label_px(8.0), type_scale::CAPTION);
        assert_eq!(label_px(12.5), type_scale::SMALL);
    }

    #[test]
    fn landing_offsets() {
        let u = RowMetrics::Uniform(20.0);
        assert_eq!(offset(None, &u, 100, 400.0), None);
        assert_eq!(offset(top(10), &u, 100, 400.0), Some(200.0));
        assert_eq!(offset(centre(30), &u, 100, 400.0), Some(600.0 - 200.0 + 10.0));
        assert_eq!(offset(centre(2), &u, 100, 400.0), Some(0.0), "never above the start");
        assert_eq!(offset(top(500), &u, 100, 400.0), Some(99.0 * 20.0), "clamped to the last line");

        // Line 1 wraps to three visual rows (60 high).
        let o = [0.0, 20.0, 80.0, 100.0, 120.0];
        let v = RowMetrics::Varied(&o);
        assert_eq!(offset(top(2), &v, 4, 50.0), Some(80.0));
        assert_eq!(offset(centre(1), &v, 4, 40.0), Some(20.0 - 20.0 + 30.0), "the tall line's middle");
        assert_eq!(offset(top(9), &v, 4, 50.0), Some(100.0), "clamped");
    }
}
