//! The blame view: every line of one file as it is in a commit, each with the commit that last
//! changed it. Consecutive lines from one commit form a run: the gutter names its author, date
//! and commit once (on the run's first line in view) and its summary on the next line, runs
//! alternate their shading, and a strip at the gutter's edge is stronger for newer commits.
//! Clicking a run's gutter selects its commit on the graph; right-clicking a line or a gutter
//! opens the line's menu (`menu.rs`). The header (`header.rs`) steps back, toggles whitespace
//! and moves, finds in the file and goes to a line.

mod gutter;
mod header;
pub(in crate::ui) mod menu;

use super::common::short;
use super::diff::{DiffStyle, paint_line};
use super::theme::color;
use crate::msg::BlameMsg;
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::blame::{BlameData, BlameLanding, BlameView};
use crate::state::blame_find::ranges;
use eframe::egui::{self, Align2, CursorIcon, FontId, Rect, RichText, Sense, Ui, WidgetInfo, WidgetType, pos2, vec2};
use gutter::{Fonts, GUTTER_W, PAD};
use menu::LineTarget;

pub fn show(ui: &mut Ui, st: &mut AppState, msgs: &mut Vec<Msg>) {
    let Some(view) = st.blame.as_mut() else { return };
    let scroll_to = view.scroll_to.take();
    ui.add_space(6.0);
    header::show(ui, &mut st.blame, &st.settings, st.snap.as_ref(), msgs);
    ui.separator();
    let note = |ui: &mut Ui, text: RichText| {
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            ui.add(egui::Label::new(text).wrap());
        });
    };
    let Some(view) = &st.blame else { return };
    if view.loading {
        note(ui, RichText::new("Loading…").color(color::weak()));
    } else if let Some(e) = &view.error {
        note(ui, RichText::new(e).color(color::red()));
    } else if let Some(data) = &view.data {
        if data.blame.lines.is_empty() {
            note(ui, RichText::new("This file is empty").color(color::weak()));
        } else {
            let out = body(ui, st, view, data, scroll_to, msgs);
            if let Some(view) = st.blame.as_mut() {
                view.top_line = out.top;
                if out.clicked.is_some() {
                    view.cursor = out.clicked;
                }
            }
        }
    }
}

/// What the body saw this frame: the first line in view and a code line clicked.
struct BodyOut {
    top: usize,
    clicked: Option<usize>,
}

/// The menu target of line `i`.
fn target<'a>(st: &AppState, view: &BlameView, data: &'a BlameData, i: usize) -> Option<LineTarget<'a>> {
    let blame = &data.blame;
    let commit = blame.commit_of_line(i)?;
    let before = !commit.oid.is_zero() && blame.origin_of_line(i).is_some_and(|o| o.previous.is_some());
    Some(LineTarget {
        line: i,
        text: blame.lines.get(i)?,
        commit,
        before,
        skipped: view.skipped.contains(&commit.oid),
        on_graph: st.snap.as_ref().is_some_and(|s| s.row_of.contains_key(&commit.oid)),
    })
}

/// Where the scroll area starts this frame for `land` (None to leave it).
fn offset(land: Option<BlameLanding>, row_h: f32, viewport_h: f32) -> Option<f32> {
    let l = land?;
    let y = l.line as f32 * row_h;
    Some(if l.center { (y - viewport_h / 2.0 + row_h / 2.0).max(0.0) } else { y })
}

/// The lines, only those in view laid out.
fn body(ui: &mut Ui, st: &AppState, view: &BlameView, data: &BlameData, land: Option<BlameLanding>, msgs: &mut Vec<Msg>) -> BodyOut {
    let style = DiffStyle::new(&st.settings);
    let blame = &data.blame;
    let n = blame.lines.len();
    let digit_w = ui.fonts(|f| f.glyph_width(&style.font, '0'));
    let num_w = digit_w * n.to_string().len() as f32;
    let small = (style.font.size + 1.0).min(12.0);
    let fonts = Fonts { name: FontId::proportional(small), sha: FontId::monospace(small) };
    ui.spacing_mut().item_spacing.y = 0.0;
    let mut out = BodyOut { top: view.top_line, clicked: None };
    let mut area = egui::ScrollArea::vertical().id_salt(("blame", &view.path, view.at)).auto_shrink([false, false]);
    if let Some(y) = offset(land, style.row_h, ui.available_height()) {
        area = area.vertical_scroll_offset(y);
    }
    let marks_col = color::amber().gamma_multiply(0.45);
    let menu_id = ui.id().with("blame-gutter-menu-line");
    area.show_rows(ui, style.row_h, n, |ui, range| {
        if range.is_empty() {
            return;
        }
        out.top = range.start;
        let w = ui.available_width();
        let rows: Vec<Rect> = range.clone().map(|_| ui.allocate_exact_size(vec2(w, style.row_h), Sense::hover()).0).collect();
        let row = |i: usize| rows[i - range.start];
        let first_run = blame.run_of(range.start).unwrap_or(0);
        let last_run = blame.run_of(range.end - 1).unwrap_or(first_run);
        let left = rows[0].left();
        let num_right = left + GUTTER_W + PAD + num_w;
        let code_left = num_right + 12.0;
        // Run shading across the whole row, then find hits and the cursor line.
        for i in range.clone() {
            let shade = if blame.run_of(i).unwrap_or(0).is_multiple_of(2) { color::bg() } else { color::panel() };
            ui.painter().rect_filled(row(i), 0.0, shade);
            if view.cursor == Some(i) {
                ui.painter().rect_filled(row(i), 0.0, color::selected());
                let bar = Rect::from_min_size(pos2(left + GUTTER_W + 1.0, row(i).top()), vec2(3.0, style.row_h));
                ui.painter().rect_filled(bar, 0.0, color::accent());
            } else if view.find.hits.binary_search(&i).is_ok() {
                ui.painter().rect_filled(row(i), 0.0, color::amber().gamma_multiply(0.08));
            }
        }
        for (k, run) in blame.runs.iter().enumerate().take(last_run + 1).skip(first_run) {
            let Some(c) = blame.commits.get(run.commit) else { continue };
            let (from, to) = (run.start.max(range.start), (run.start + run.len).min(range.end));
            if from >= to {
                continue;
            }
            let gutter = Rect::from_min_max(pos2(left, row(from).top()), pos2(left + GUTTER_W, row(to - 1).bottom()));
            gutter::paint_age(ui.painter(), gutter, data.age(run.commit));
            let resp = ui.interact(gutter, ui.id().with(("blame-run", k)), Sense::click());
            let label = format!("Select {} on the graph", short(c.oid));
            resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, &label));
            if resp.hovered() {
                ui.painter().rect_filled(gutter, 0.0, color::hover());
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
            if run.start > 0 && run.start >= range.start {
                ui.painter().hline(row(run.start).x_range(), row(run.start).top() + 0.5, (1.0, color::border()));
            }
            gutter::text(ui, st, &fonts, c, row(from), (from + 1 < to).then(|| row(from + 1)));
            // The menu acts on the line under the pointer when the right-click happened.
            if resp.secondary_clicked()
                && let Some(p) = resp.interact_pointer_pos()
            {
                let line = (from + ((p.y - row(from).top()) / style.row_h).max(0.0) as usize).min(to - 1);
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
        ui.painter().vline(left + GUTTER_W, rows[0].top()..=rows[rows.len() - 1].bottom(), (1.0, color::border()));
        for i in range.clone() {
            let r = row(i);
            let y = r.center().y;
            ui.painter().text(pos2(num_right, y), Align2::RIGHT_CENTER, (i + 1).to_string(), style.font.clone(), color::faint());
            let code = Rect::from_min_max(pos2(left + GUTTER_W + 1.0, r.top()), r.max);
            let resp = ui.interact(code, ui.id().with(("blame-line", i)), Sense::click());
            if resp.clicked() || resp.secondary_clicked() {
                out.clicked = Some(i);
            }
            resp.context_menu(|ui| {
                if let Some(t) = target(st, view, data, i) {
                    menu::line_menu(ui, msgs, &t);
                }
            });
            let clip = Rect::from_min_max(pos2(code_left, r.top()), r.max);
            let painter = ui.painter().with_clip_rect(clip.intersect(ui.clip_rect()));
            let found = if view.find.query.is_empty() { Vec::new() } else { ranges(&blame.lines[i], &view.find.query) };
            let marks = (!found.is_empty()).then_some((found.as_slice(), marks_col));
            paint_line(&painter, pos2(code_left, y), &blame.lines[i], data.spans(i), marks, &style.font, color::text(), style.tab, None);
        }
    });
    out
}
