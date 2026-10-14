//! The file history view: the commits that changed one file on the left (newest first,
//! following renames), and the selected commit with its diff of the file on the right.

use super::common::{menu_item, short, status_color};
use super::file_menu;
use super::theme::color;
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::history::FileHistoryView;
use eframe::egui::text::{LayoutJob, TextWrapping};
use eframe::egui::{self, Align, Align2, Color32, CursorIcon, FontId, Key, Layout, Pos2, RichText, Sense, Ui, pos2, vec2};
use trellis_core::repo::{DiffTarget, HistoryEntry};

const ROW_H: f32 = 44.0;
const NOT_ON_GRAPH: &str = "Not visible on the graph: its branch is hidden or it is older than the commit limit";

pub fn show(ui: &mut Ui, st: &mut AppState, msgs: &mut Vec<Msg>) {
    let scroll_to = st.view.history_scroll_to.take();
    let Some(view) = &st.history else { return };
    ui.add_space(6.0);
    header(ui, view, msgs);
    ui.separator();
    egui::SidePanel::left("file-history-list")
        .resizable(true)
        .default_width(380.0)
        .width_range(260.0..=600.0)
        .frame(egui::Frame::new().fill(color::panel()))
        .show_inside(ui, |ui| list(ui, st, scroll_to, msgs));
    egui::CentralPanel::default().frame(egui::Frame::new().fill(color::bg())).show_inside(ui, |ui| selected(ui, st, msgs));
}

/// "File history", the path, "as of <sha>" when the walk didn't start at HEAD, and Close.
fn header(ui: &mut Ui, view: &FileHistoryView, msgs: &mut Vec<Msg>) {
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label(RichText::new("File history").color(color::weak()));
        ui.label(RichText::new(&view.path).strong().size(15.0).color(color::text()));
        if let Some(at) = view.as_of {
            ui.label(RichText::new(format!("as of {}", short(at))).color(color::weak()))
                .on_hover_text("The history as seen from this commit (it is not part of HEAD's history of this file)");
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.button("Close  (Esc)").clicked() {
                msgs.push(Msg::CloseFileHistory);
            }
        });
    });
}

/// One line of text cut to `max_w` with an ellipsis.
fn paint_line(ui: &Ui, pos: Pos2, text: &str, font: FontId, c: Color32, max_w: f32) -> f32 {
    if max_w <= 0.0 {
        return 0.0;
    }
    let mut job = LayoutJob::simple_singleline(text.to_owned(), font, c);
    job.wrap = TextWrapping::truncate_at_width(max_w);
    let galley = ui.fonts(|f| f.layout_job(job));
    let w = galley.size().x;
    ui.painter().galley(pos2(pos.x, pos.y - galley.size().y / 2.0), galley, c);
    w
}

fn list(ui: &mut Ui, st: &AppState, scroll_to: Option<usize>, msgs: &mut Vec<Msg>) {
    let Some(view) = &st.history else { return };
    keyboard(ui, view, msgs);
    egui::ScrollArea::vertical().id_salt("file-history").auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for (i, e) in view.entries.iter().enumerate() {
            let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
            if scroll_to == Some(i) {
                resp.scroll_to_me(Some(Align::Center));
            }
            if ui.is_rect_visible(rect) {
                let selected = view.selected == Some(i);
                if selected {
                    ui.painter().rect_filled(rect, 0.0, color::selected());
                    ui.painter().rect_filled(egui::Rect::from_min_size(rect.min, vec2(3.0, ROW_H)), 0.0, color::accent());
                } else if resp.hovered() {
                    ui.painter().rect_filled(rect, 0.0, color::hover());
                }
                row(ui, st, rect, e);
            }
            if resp.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
            if resp.clicked() {
                msgs.push(Msg::SelectHistoryEntry(i));
            }
            resp.context_menu(|ui| {
                menu_item(msgs, ui, "View on graph", Msg::ShowInGraph(e.oid));
                menu_item(msgs, ui, "Copy commit SHA", Msg::Copy(e.oid.to_string()));
                ui.separator();
                file_menu::restore_item(ui, msgs, e.oid, &e.path, e.status);
            });
            ui.painter().hline(rect.x_range(), rect.bottom() - 0.5, (1.0, color::border()));
        }
        ui.add_space(8.0);
        footer(ui, st, view, msgs);
    });
}

/// Line 1: short SHA and summary. Line 2: status, author (and the old path of a rename), with
/// the date on the right.
fn row(ui: &Ui, st: &AppState, rect: egui::Rect, e: &HistoryEntry) {
    let mono = FontId::monospace(12.0);
    let body = FontId::proportional(13.0);
    let small = FontId::proportional(12.0);
    let (left, right) = (rect.left() + 12.0, rect.right() - 10.0);
    let y1 = rect.top() + 13.0;
    let y2 = rect.top() + 31.0;

    let sha_w = paint_line(ui, pos2(left, y1), &short(e.oid), mono.clone(), color::weak(), 80.0);
    paint_line(ui, pos2(left + sha_w + 8.0, y1), &e.summary, body, color::text(), right - left - sha_w - 8.0);

    let date = st.settings.format_time(e.time, st.view.now, st.view.utc_offset);
    let date_w = ui.fonts(|f| f.layout_no_wrap(date.clone(), small.clone(), color::weak()).size().x);
    ui.painter().text(pos2(right, y2), Align2::RIGHT_CENTER, date, small.clone(), color::weak());
    let status_w = paint_line(ui, pos2(left, y2), &e.status.to_string(), mono, status_color(e.status), 20.0);
    let mut x = left + status_w.max(8.0) + 8.0;
    let limit = right - date_w - 12.0;
    let author = if e.author.is_empty() { "Unknown" } else { e.author.as_str() };
    x += paint_line(ui, pos2(x, y2), author, small.clone(), color::text(), limit - x);
    if let Some(old) = &e.old_path {
        x += 10.0;
        paint_line(ui, pos2(x, y2), &format!("renamed from {old}"), small, color::weak(), limit - x);
    }
}

fn footer(ui: &mut Ui, st: &AppState, view: &FileHistoryView, msgs: &mut Vec<Msg>) {
    egui::Frame::new().inner_margin(egui::Margin { left: 12, right: 10, top: 0, bottom: 0 }).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 6.0;
        if view.loading {
            ui.label(RichText::new("Loading…").color(color::weak()));
        } else if let Some(e) = &view.error {
            ui.add(egui::Label::new(RichText::new(e).color(color::red())).wrap());
        } else if view.complete() {
            if view.entries.is_empty() {
                ui.label(RichText::new("No commits change this file").color(color::weak()));
            } else {
                ui.label(RichText::new("Reached the first commit").color(color::faint()));
            }
        } else {
            // Stopped at the commit limit.
            if let Some((oid, time)) = view.searched_until {
                let date = st.settings.format_time(time, st.view.now, st.view.utc_offset);
                ui.label(RichText::new(format!("Searched back to {date} ({})", short(oid))).color(color::weak()));
            }
            if ui.button("Search further").clicked() {
                msgs.push(Msg::MoreFileHistory);
            }
        }
    });
    ui.add_space(8.0);
}

/// Up and Down move the selection while no text field has focus.
fn keyboard(ui: &Ui, view: &FileHistoryView, msgs: &mut Vec<Msg>) {
    if view.entries.is_empty() || ui.ctx().memory(|m| m.focused().is_some()) {
        return;
    }
    let (up, down) =
        ui.input(|i| (i.modifiers.is_none() && i.key_pressed(Key::ArrowUp), i.modifiers.is_none() && i.key_pressed(Key::ArrowDown)));
    let next = match (view.selected, up, down) {
        (Some(s), true, false) if s > 0 => s - 1,
        (Some(s), false, true) if s + 1 < view.entries.len() => s + 1,
        (None, _, true) | (None, true, _) => 0,
        _ => return,
    };
    msgs.push(Msg::SelectHistoryEntry(next));
}

/// The selected commit: summary, author and date, links, then its diff of the file.
fn selected(ui: &mut Ui, st: &mut AppState, msgs: &mut Vec<Msg>) {
    let Some(view) = &st.history else { return };
    let Some(e) = view.selected_entry().cloned() else {
        if !view.loading && !view.entries.is_empty() {
            ui.add_space(10.0);
            ui.label(RichText::new("Select a commit to see its changes to the file.").color(color::weak()));
        }
        return;
    };
    let visible = st.snap.as_ref().is_some_and(|s| s.row_of.contains_key(&e.oid));
    egui::Frame::new().inner_margin(egui::Margin::symmetric(10, 8)).show(ui, |ui| {
        ui.add(egui::Label::new(RichText::new(&e.summary).strong().size(14.0).color(color::text())).truncate());
        let author = if e.author.is_empty() { "Unknown" } else { e.author.as_str() };
        let when = st.settings.format_verbose(e.time, st.view.utc_offset);
        ui.label(RichText::new(format!("{author}, {when}")).color(color::weak()));
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("commit {}", short(e.oid))).monospace().color(color::weak()));
            let link = ui.add_enabled(visible, egui::Link::new("View on graph")).on_disabled_hover_text(NOT_ON_GRAPH);
            if link.clicked() {
                msgs.push(Msg::ShowInGraph(e.oid));
            }
            if ui.small_button("Copy SHA").clicked() {
                msgs.push(Msg::Copy(e.oid.to_string()));
            }
        });
    });
    ui.separator();
    let target = DiffTarget::Commit(e.oid, e.path.clone());
    if st.diff.as_ref().is_some_and(|(t, _)| *t == target) {
        super::diff::body(ui, st, msgs);
    } else {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            ui.label(RichText::new("Loading…").color(color::weak()));
        });
    }
}
