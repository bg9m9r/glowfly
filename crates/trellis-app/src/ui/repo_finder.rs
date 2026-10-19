//! The Find repositories dialog (`Modal::FindRepos`): the project folders Trellis looks in, a
//! filter, and the repositories found under them, grouped by project folder. Each row stars the
//! repository or opens it; the scan itself runs in the runner (`ReposMsg`).

use super::theme::color;
use super::widgets::{self, Icon};
use crate::msg::{Msg, RecentMsg, RepoMsg, ReposMsg};
use crate::state::repo_lists::favorite_label;
use crate::state::repo_scan::{FinderRow, RepoScan, matches};
use crate::ui::theme::controls;
use eframe::egui::{self, Align, Layout, Response, RichText, Sense, vec2};
use std::path::PathBuf;
use trellis_core::i18n::Lang;
use trellis_core::native_path::shown;
use trellis_core::paths::same_spelling;

/// What the dialog lists: the repositories found, the scan's state and the project folders.
#[derive(Clone, Copy)]
pub struct FinderLists<'a> {
    pub rows: &'a [FinderRow],
    pub scan: &'a RepoScan,
    pub folders: &'a [PathBuf],
}

/// Draws the dialog; returns true when it should close. The filter keeps the keyboard focus
/// unless `focus` is false (a credential prompt is open above it).
pub fn show(ui: &mut egui::Ui, t: Lang, filter: &mut String, lists: FinderLists, focus: bool, msgs: &mut Vec<Msg>) -> bool {
    let mut close = false;
    let (scan, folders) = (lists.scan, lists.folders);
    ui.set_width(560.0);
    ui.heading(t.tr("finder.title"));
    ui.add_space(8.0);
    ui.label(RichText::new(t.tr("finder.folders.header")).strong());
    for f in folders {
        folder_row(ui, t, f, scan, msgs);
    }
    if folders.is_empty() {
        ui.label(RichText::new(t.tr("finder.no_folders")).color(color::weak()));
    }
    ui.horizontal(|ui| {
        if ui.button(t.tr("finder.add_folder.button")).clicked() {
            msgs.push(Msg::Repos(ReposMsg::AddFolder));
        }
        if !folders.is_empty() && look_again(ui, t).clicked() {
            msgs.push(Msg::Repos(ReposMsg::Rescan));
        }
        if scan.running {
            ui.add(egui::Spinner::new());
            ui.label(RichText::new(t.tr("finder.scanning")).color(color::weak()));
        }
    });
    if !folders.is_empty() {
        ui.add_space(10.0);
        let r = controls::field(ui, egui::TextEdit::singleline(filter).hint_text(t.tr("finder.filter.hint")).desired_width(f32::INFINITY));
        if focus {
            r.request_focus();
        }
        ui.add_space(6.0);
        results(ui, t, filter, &lists, msgs, &mut close);
    }
    ui.add_space(10.0);
    if ui.button(t.tr("common.close.button")).clicked() {
        close = true;
    }
    close
}

/// One project folder: its path, why the last scan couldn't read it, and ×.
fn folder_row(ui: &mut egui::Ui, t: Lang, f: &std::path::Path, scan: &RepoScan, msgs: &mut Vec<Msg>) {
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if widgets::small_icon(ui, Icon::Close, t.tr("finder.remove_folder.tip"), color::weak()).clicked() {
                msgs.push(Msg::Repos(ReposMsg::RemoveFolder(f.to_path_buf())));
            }
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                ui.label(shown(f));
                if let Some((_, why)) = scan.unreadable.iter().find(|(r, _)| same_spelling(r, f)) {
                    let note = t.trf("finder.unreadable", &[("error", why)]);
                    ui.add(egui::Label::new(RichText::new(note).small().color(color::weak())).truncate());
                }
            });
        });
    });
}

/// The repositories found that match `filter`, under a header per project folder.
fn results(ui: &mut egui::Ui, t: Lang, filter: &str, lists: &FinderLists, msgs: &mut Vec<Msg>, close: &mut bool) {
    let FinderLists { rows, scan, folders } = *lists;
    let weak = |ui: &mut egui::Ui, text: &str| ui.label(RichText::new(text).color(color::weak()));
    if rows.is_empty() {
        if !scan.running {
            weak(ui, t.tr("finder.nothing_found"));
        }
    } else {
        let matching: Vec<&FinderRow> = rows.iter().filter(|r| matches(&r.repo, filter)).collect();
        if matching.is_empty() {
            weak(ui, t.tr("finder.no_match"));
        }
        egui::ScrollArea::vertical().max_height(380.0).auto_shrink([false, true]).show(ui, |ui| {
            for root in folders {
                let group: Vec<&&FinderRow> = matching.iter().filter(|r| same_spelling(&r.repo.root, root)).collect();
                if group.is_empty() {
                    continue;
                }
                ui.add_space(4.0);
                weak(ui, &format!("{} ({})", shown(root), group.len()));
                for r in group {
                    if row(ui, t, r, msgs) {
                        *close = true;
                    }
                }
            }
        });
    }
    if scan.truncated {
        weak(ui, &t.trf("finder.truncated", &[("max", &trellis_core::repo_scan::MAX_FOLDERS)]));
    }
}

/// One repository found: star, its name (opens it), its path and an "open" tag. Returns true
/// when it was opened.
fn row(ui: &mut egui::Ui, t: Lang, r: &FinderRow, msgs: &mut Vec<Msg>) -> bool {
    let mut opened = false;
    let path = &r.repo.path;
    ui.horizontal(|ui| {
        let (icon, col) = if r.favorite { (Icon::StarFilled, color::amber()) } else { (Icon::Star, color::weak()) };
        if widgets::small_icon(ui, icon, favorite_label(t, r.favorite), col).clicked() {
            msgs.push(Msg::Recent(RecentMsg::SetFavorite { path: path.clone(), favorite: !r.favorite }));
        }
        let name = ui.add(egui::Label::new(RichText::new(&r.repo.name).color(color::accent())).selectable(false).sense(Sense::click()));
        if name.hovered() {
            ui.painter().hline(name.rect.x_range(), name.rect.bottom() - 1.0, egui::Stroke::new(1.0_f32, color::accent()));
        }
        if name.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            msgs.push(Msg::Repo(RepoMsg::Open(path.clone())));
            opened = true;
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if r.open {
                ui.label(RichText::new(t.tr("repos.open_tag.label")).small().color(color::weak()))
                    .on_hover_text(t.tr("repos.open_tag.tip"));
            }
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                let text = shown(path);
                let label = egui::Label::new(RichText::new(&text).small().color(color::weak())).truncate();
                ui.add(label).on_hover_text(text);
            });
        });
    });
    opened
}

/// The Look again button: a painted refresh arrow (the font lacks the glyph) and its text.
fn look_again(ui: &mut egui::Ui, t: Lang) -> Response {
    let text = t.tr("finder.look_again.button");
    let galley = egui::WidgetText::from(text).into_galley(ui, None, f32::INFINITY, egui::TextStyle::Button);
    let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + 36.0, ui.spacing().interact_size.y), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, text));
    let v = ui.style().interact(&resp);
    let p = ui.painter();
    p.rect(rect, v.corner_radius, v.weak_bg_fill, v.bg_stroke, egui::StrokeKind::Inside);
    widgets::paint_icon(p, egui::pos2(rect.left() + 14.0, rect.center().y), Icon::Refresh, v.text_color());
    p.galley(egui::pos2(rect.left() + 28.0, rect.center().y - galley.size().y / 2.0), galley, v.text_color());
    resp
}
