//! The blame view: every line of one file as it is in a commit, each with the commit that last
//! changed it. Consecutive lines from one commit form a run: the gutter names its author, date
//! and commit once (on the run's first line in view) and its summary on the next line, and runs
//! alternate their shading. Clicking a run's gutter selects its commit on the graph.

use super::common::{paint_cut_line, short};
use super::diff::{DiffStyle, paint_line};
use super::theme::color;
use crate::msg::BlameMsg;
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::blame::{BlameData, BlameView};
use eframe::egui::{self, Align, Align2, CursorIcon, FontId, Layout, Rect, RichText, Sense, Ui, WidgetInfo, WidgetType, pos2, vec2};
use trellis_core::repo::BlameCommit;

/// Width of the author, date and commit gutter.
const GUTTER_W: f32 = 300.0;
/// Space between the gutter's edges and its text.
const PAD: f32 = 8.0;

pub fn show(ui: &mut Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let Some(view) = &st.blame else { return };
    ui.add_space(6.0);
    header(ui, view, msgs);
    ui.separator();
    let note = |ui: &mut Ui, text: RichText| {
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            ui.add(egui::Label::new(text).wrap());
        });
    };
    if view.loading {
        note(ui, RichText::new("Loading…").color(color::weak()));
    } else if let Some(e) = &view.error {
        note(ui, RichText::new(e).color(color::red()));
    } else if let Some(data) = &view.data {
        if data.blame.lines.is_empty() {
            note(ui, RichText::new("This file is empty").color(color::weak()));
        } else {
            body(ui, st, view, data, msgs);
        }
    }
}

/// "Blame", the path, the commit it is shown at, its size, and Close.
fn header(ui: &mut Ui, view: &BlameView, msgs: &mut Vec<Msg>) {
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label(RichText::new("Blame").color(color::weak()));
        ui.label(RichText::new(&view.path).strong().size(15.0).color(color::text()));
        let resolved = view.data.as_ref().map(|d| short(d.blame.at));
        let at = match (view.at, resolved) {
            (Some(oid), _) => format!("at {}", short(oid)),
            (None, Some(head)) => format!("at HEAD · {head}"),
            (None, None) => "at HEAD".to_owned(),
        };
        ui.label(RichText::new(at).color(color::weak()));
        if let Some(d) = &view.data {
            let (lines, commits) = (d.blame.lines.len(), d.blame.commits.len());
            let plural = |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
            let size = format!("{} · {}", plural(lines, "line", "lines"), plural(commits, "commit", "commits"));
            ui.label(RichText::new(size).color(color::weak()));
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.button("Close  (Esc)").clicked() {
                msgs.push(Msg::Blame(BlameMsg::Close));
            }
        });
    });
}

/// The lines, only those in view laid out.
fn body(ui: &mut Ui, st: &AppState, view: &BlameView, data: &BlameData, msgs: &mut Vec<Msg>) {
    let style = DiffStyle::new(&st.settings);
    let blame = &data.blame;
    let n = blame.lines.len();
    let digit_w = ui.fonts(|f| f.glyph_width(&style.font, '0'));
    let num_w = digit_w * n.to_string().len() as f32;
    let small = (style.font.size + 1.0).min(12.0);
    let fonts = Fonts { name: FontId::proportional(small), sha: FontId::monospace(small) };
    ui.spacing_mut().item_spacing.y = 0.0;
    egui::ScrollArea::vertical().id_salt(("blame", &view.path, view.at)).auto_shrink([false, false]).show_rows(
        ui,
        style.row_h,
        n,
        |ui, range| {
            if range.is_empty() {
                return;
            }
            let w = ui.available_width();
            let rows: Vec<Rect> = range.clone().map(|_| ui.allocate_exact_size(vec2(w, style.row_h), Sense::hover()).0).collect();
            let row = |i: usize| rows[i - range.start];
            let first_run = blame.run_of(range.start).unwrap_or(0);
            let last_run = blame.run_of(range.end - 1).unwrap_or(first_run);
            let left = rows[0].left();
            // Run shading across the whole row.
            for i in range.clone() {
                let shade = if blame.run_of(i).unwrap_or(0).is_multiple_of(2) { color::bg() } else { color::panel() };
                ui.painter().rect_filled(row(i), 0.0, shade);
            }
            for (k, run) in blame.runs.iter().enumerate().take(last_run + 1).skip(first_run) {
                let Some(c) = blame.commits.get(run.commit) else { continue };
                let (from, to) = (run.start.max(range.start), (run.start + run.len).min(range.end));
                if from >= to {
                    continue;
                }
                let gutter = Rect::from_min_max(pos2(left, row(from).top()), pos2(left + GUTTER_W, row(to - 1).bottom()));
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
                gutter_text(ui, st, &fonts, c, row(from), (from + 1 < to).then(|| row(from + 1)));
                let author = if c.author.is_empty() { "Unknown" } else { c.author.as_str() };
                let when = st.settings.format_verbose(c.time, st.view.utc_offset);
                let tip = format!("{author} <{}>\n{when}\n{}\n\nClick to select this commit on the graph", c.email, c.summary);
                if resp.on_hover_text(tip).clicked() {
                    msgs.push(Msg::Blame(BlameMsg::SelectCommit(c.oid)));
                }
            }
            ui.painter().vline(left + GUTTER_W, rows[0].top()..=rows[rows.len() - 1].bottom(), (1.0, color::border()));
            let num_right = left + GUTTER_W + PAD + num_w;
            let code_left = num_right + 12.0;
            for i in range.clone() {
                let r = row(i);
                let y = r.center().y;
                ui.painter().text(pos2(num_right, y), Align2::RIGHT_CENTER, (i + 1).to_string(), style.font.clone(), color::faint());
                let clip = Rect::from_min_max(pos2(code_left, r.top()), r.max);
                let painter = ui.painter().with_clip_rect(clip.intersect(ui.clip_rect()));
                paint_line(&painter, pos2(code_left, y), &blame.lines[i], data.spans(i), None, &style.font, color::text(), style.tab, None);
            }
        },
    );
}

struct Fonts {
    name: FontId,
    sha: FontId,
}

/// The run's commit on `first` (author, then date and short SHA at the right) and its summary on
/// `second`.
fn gutter_text(ui: &Ui, st: &AppState, fonts: &Fonts, c: &BlameCommit, first: Rect, second: Option<Rect>) {
    let (left, right) = (first.left() + PAD, first.left() + GUTTER_W - PAD);
    let y = first.center().y;
    let sha = ui.painter().text(pos2(right, y), Align2::RIGHT_CENTER, short(c.oid), fonts.sha.clone(), color::weak());
    let date = st.settings.format_time(c.time, st.view.now, st.view.utc_offset);
    let date = ui.painter().text(pos2(sha.left() - 10.0, y), Align2::RIGHT_CENTER, date, fonts.name.clone(), color::weak());
    let author = if c.author.is_empty() { "Unknown" } else { c.author.as_str() };
    paint_cut_line(ui, pos2(left, y), author, fonts.name.clone(), color::text(), date.left() - 10.0 - left);
    if let Some(r) = second {
        paint_cut_line(ui, pos2(left, r.center().y), &c.summary, fonts.name.clone(), color::faint(), right - left);
    }
}
