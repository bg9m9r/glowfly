//! The selected commit's details, title first: the branch whose line it sits on (a colour dot and
//! its name), the title (the message's first paragraph, as git treats it), the author with a relative date and the signature badge, the chips row
//! (`details_chips`: SHA, parents, committer), the body in a readable block, the forge issues
//! it mentions, then its changed files.

use super::UiCache;
use super::branch_dot::{self, DOT_ROOM};
use super::common::issue_chip;
use super::details::{RowActions, changed_file_row_at, file_reveal};
use super::file_menu::RowSource;
use super::file_menu::folder_menu::{self, FolderStage};
use super::graph::draw_avatar;
use super::list_row::file_tree::{ListRows, Shown, folder_row, mode_toggle};
use super::theme::branch_colours::line;
use super::theme::color;
use super::widgets::{self, buttons};
use crate::msg::{Msg, SelectionMsg};
use crate::state::AppState;
use crate::state::file_folds::FileListId;
use crate::state::lfs::TrackContext;
use crate::state::list_keys::FileList;
use crate::ui::theme::tokens::radius;
use crate::ui::theme::type_scale;
use eframe::egui::{self, Align, Label, Layout, Margin, RichText, ScrollArea, Sense, Ui, vec2};
use glowfly_core::repo::{CommitDetail, DiffTarget};
use std::borrow::Cow;

/// Below the title.
const TITLE_GAP: f32 = 12.0;
/// Below the author's row, before the chips.
const CHIPS_GAP: f32 = 8.0;
/// Before the body block.
const BODY_GAP: f32 = 12.0;
/// Before the changed files.
const FILES_GAP: f32 = 16.0;
/// The body block's inner margin and corner radius.
const BODY_PAD: i8 = 12;
const BODY_RADIUS: u8 = radius::MD;
/// The author's row: its height (the avatar's slot) and the avatar's radius.
const PERSON_H: f32 = 32.0;
const AVATAR_R: f32 = 15.0;

/// A message's title and body, as git splits them: the title is the first paragraph (every line
/// up to the first blank one, joined with spaces), the body what follows the blank line, without
/// its leading blank lines or trailing whitespace. A line of only whitespace counts as blank.
pub(super) fn split_message(msg: &str) -> (String, &str) {
    let mut title: Vec<&str> = Vec::new();
    let mut rest = msg;
    while !rest.is_empty() {
        let (line, next) = rest.split_once('\n').unwrap_or((rest, ""));
        rest = next;
        match line.trim() {
            "" if title.is_empty() => continue,
            "" => break,
            line => title.push(line),
        }
    }
    while !rest.is_empty() {
        let (line, next) = rest.split_once('\n').unwrap_or((rest, ""));
        if !line.trim().is_empty() {
            break;
        }
        rest = next;
    }
    (title.join(" "), rest.trim_end())
}

/// The body block's greatest height in a panel with `available` px left: longer bodies scroll.
fn body_max_height(available: f32) -> f32 {
    (available * 0.4).clamp(96.0, 320.0)
}

/// `px` of space below the last widget, counting the item spacing already there.
fn gap(ui: &mut Ui, px: f32) {
    ui.add_space((px - ui.spacing().item_spacing.y).max(0.0));
}

pub(super) fn panel(ui: &mut Ui, st: &AppState, cache: &mut UiCache, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let Some(d) = &st.session.detail else {
        return super::pane_status::loading_line(ui, t.tr("common.loading"));
    };
    let (title, body) = split_message(&d.message);
    branch_row(ui, st, d);
    if title.is_empty() {
        ui.label(RichText::new(t.tr("details.commit.no_message.label")).color(color::weak()));
    } else {
        let text = RichText::new(title).font(type_scale::strong(ui, type_scale::SUBHEAD)).color(color::strong());
        ui.add(Label::new(text).wrap().selectable(true));
    }
    gap(ui, TITLE_GAP);
    person_row(ui, st, cache, d, msgs);
    gap(ui, CHIPS_GAP);
    super::details_chips::row(ui, st, d, msgs);
    // Only graph rows are compared (a stash selected in the sidebar isn't one).
    if st.snap.as_ref().is_some_and(|s| s.row_of.contains_key(&d.oid)) {
        gap(ui, CHIPS_GAP);
        let compare =
            buttons::secondary(t.tr("details.commit.compare_workdir.button")).small().tip(t.tr("details.commit.compare_workdir.tip"));
        if ui.add(compare).clicked() {
            msgs.push(Msg::Selection(SelectionMsg::CompareWithWorkdir(d.oid)));
        }
    }
    if !body.is_empty() {
        gap(ui, BODY_GAP);
        body_block(ui, body);
    }
    if let Some(fs) = &st.session.forge {
        let keys = fs.issue_refs(&d.message);
        if !keys.is_empty() {
            ui.add_space(6.0);
            ui.label(RichText::new(fs.issues_header(t)).size(type_scale::CAPTION).color(color::weak()));
            for k in keys {
                issue_chip(ui, fs, &k, msgs);
            }
        }
    }
    gap(ui, FILES_GAP);
    ui.separator();
    files(ui, st, d, msgs);
}

/// The branch whose line the commit sits on in the graph: its dot and name, small and weak.
/// Left out for unnamed lines and commits not in the graph (stashes among them).
fn branch_row(ui: &mut Ui, st: &AppState, d: &CommitDetail) {
    let Some((name, swatch)) = st.snap.as_ref().and_then(|s| s.commit_branch(d.oid)) else { return };
    let tip = st.settings.lang().tr("details.commit.branch.tip");
    let r = ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let text = RichText::new(name).size(type_scale::SMALL).color(color::weak());
        let h = ui.text_style_height(&egui::TextStyle::Body);
        let (dot, _) = ui.allocate_exact_size(vec2(DOT_ROOM, h), Sense::hover());
        branch_dot::paint(ui.painter(), dot, 0.0, line(swatch));
        widgets::cut_label(ui, text, ui.available_width()).0
    });
    let _ = r.inner.on_hover_text(tip);
    gap(ui, 6.0);
}

/// The author's avatar, name and relative date (the whole name, email and date on hover), with
/// the signature badge at the row's right end. The name is cut first in a narrow panel.
fn person_row(ui: &mut Ui, st: &AppState, cache: &mut UiCache, d: &CommitDetail, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let name = if d.author.is_empty() { &d.email } else { &d.author };
    let when = format!(" · {}", glowfly_core::dates::relative(t, st.view.now - d.author_time));
    let tip = format!(
        "{}\n{}",
        super::details_chips::identity(&d.author, &d.email),
        st.settings.format_verbose(d.author_time, st.view.utc_offset)
    );
    ui.allocate_ui_with_layout(vec2(ui.available_width(), PERSON_H), Layout::left_to_right(Align::Center), |ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(PERSON_H, PERSON_H), Sense::hover());
        let av = if super::avatars::shown(st) { Some(&mut cache.avatars) } else { None };
        let ring = st.snap.as_ref().and_then(|s| s.commit_swatch(d.oid)).map_or_else(color::faint, line);
        draw_avatar(ui.painter(), rect.center(), AVATAR_R, &d.author, &d.email, ring, av);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            super::signature_badge::show(ui, st, d, msgs);
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                let date = RichText::new(when.as_str()).size(type_scale::SMALL).color(color::weak());
                let date_w = ui.painter().layout_no_wrap(when.clone(), type_scale::font(type_scale::SMALL), color::weak()).size().x;
                let text = RichText::new(name).font(type_scale::font(type_scale::BODY)).color(color::text());
                let (n, _) = widgets::cut_label(ui, text, ui.available_width() - date_w - 1.0);
                let w = ui.label(date);
                let _ = ui.interact(n.rect.union(w.rect), ui.id().with("commit-author"), Sense::hover()).on_hover_text(tip);
            });
        });
    });
}

/// The body in a block on the background, wrapped and selectable, scrolling past its cap.
fn body_block(ui: &mut Ui, body: &str) {
    let max_h = body_max_height(ui.available_height());
    let body: Cow<str> = if body.contains('\r') { Cow::Owned(body.replace('\r', "")) } else { Cow::Borrowed(body) };
    egui::Frame::new().fill(color::bg()).corner_radius(BODY_RADIUS).inner_margin(Margin::same(BODY_PAD)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ScrollArea::vertical().id_salt("commit-body").max_height(max_h).show(ui, |ui| {
            let text = RichText::new(body.as_ref()).font(type_scale::font(type_scale::BODY)).color(color::text());
            ui.add(Label::new(text).wrap().selectable(true));
        });
    });
}

/// The "N changed files" header with the List / Tree toggle and the total on its right, then
/// the file rows (as a tree in Tree mode, its folds kept for every commit). The list takes the
/// keyboard focus as a whole (`file_list_keys.rs`).
fn files(ui: &mut Ui, st: &AppState, d: &CommitDetail, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    // The file being opened is highlighted at once, so a click or a key answers immediately.
    let open = crate::state::wip_lists::cursor(st);
    let n = d.files.len();
    let counts = st.session.reads.line_counts.commit(d.oid);
    let mode = st.settings.file_list_mode;
    let heading = t.trn("details.commit.changed_files", n, &[]);
    super::details_wip::header::frame(ui, |ui| {
        mode_toggle(ui, t, mode, msgs);
        if let Some(c) = counts.filter(|c| c.complete) {
            super::line_counts::total(ui, c.total);
        }
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            ui.label(type_scale::section_text(ui, heading.clone()));
        });
    });
    let rows = ListRows::of(
        ui,
        mode,
        n,
        || d.files.iter().map(|f| f.path.as_str()).collect(),
        st.session.tab_view.file_folds.closed(FileListId::Commit),
    );
    let lfs = TrackContext::of(st);
    let tools = crate::state::tool_compare::ToolSetup::of(t, &st.settings.external_tools);
    let commands = crate::state::custom_commands::MenuCommands::for_state(st);
    let row_h = widgets::LIST_ROW_H;
    let index = |t: &DiffTarget| {
        let i = d.files.iter().position(|f| *t == DiffTarget::Commit(d.oid, f.path.clone()))?;
        rows.row_of_file(i)
    };
    let current = open.and_then(index);
    let focus = super::file_list_keys::target(ui, FileList::Commit, rows.len(), ui.available_height(), &heading);
    let shown = ScrollArea::vertical().auto_shrink([false, false]).show_rows(ui, row_h, rows.len(), |ui, range| {
        file_reveal::reveal(ui, "commit-files", st.view.file_reveal.as_ref(), index, &range, row_h);
        for row in range {
            let r = match rows.get(row) {
                Shown::File { index, depth } => {
                    let f = &d.files[index];
                    let patch_blocked = (d.parents.len() > 1).then(|| crate::state::selection::CommitBlock::PatchMerge.text(t));
                    let src = RowSource::Commit(d.oid);
                    let lines = counts.and_then(|c| c.get(&f.path).map(|n| (n, c.largest)));
                    let in_workdir = d.in_workdir(&f.path);
                    let actions = RowActions { src, history_at: d.oid, in_workdir, lfs, tools, patch_blocked, lines, commands: &commands };
                    changed_file_row_at(ui, t, f, (DiffTarget::Commit(d.oid, f.path.clone()), open), actions, depth, msgs)
                }
                Shown::Folder(folder) => {
                    let menu = |ui: &mut Ui, msgs: &mut Vec<Msg>, path: &str| {
                        folder_menu::items(ui, t, msgs, path, Some(d.oid), FolderStage::None)
                    };
                    folder_row(ui, folder, true, msgs, FileListId::Commit, menu)
                }
            };
            focus.row(ui, &r, current == Some(row));
        }
    });
    focus.rest(ui, &shown, current.is_some());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_title_is_the_first_paragraph() {
        assert_eq!(split_message("One line"), ("One line".to_owned(), ""));
        assert_eq!(split_message("Title\n\nBody line\n"), ("Title".to_owned(), "Body line"));
        assert_eq!(split_message("Two-line\ntitle\n\nBody"), ("Two-line title".to_owned(), "Body"));
        assert_eq!(split_message(""), (String::new(), ""));
    }

    #[test]
    fn crlf_and_blank_lines_are_trimmed() {
        assert_eq!(split_message("T\r\n\r\nB\r\n"), ("T".to_owned(), "B"));
        assert_eq!(split_message("T\n\n\n\n  indented body\n\n"), ("T".to_owned(), "  indented body"), "leading blank lines go");
        assert_eq!(split_message("T\n   \t\nB"), ("T".to_owned(), "B"), "a line of whitespace is the blank separator");
        assert_eq!(split_message("\n\nT\n\nB"), ("T".to_owned(), "B"), "leading blank lines before the title go");
    }

    #[test]
    fn the_body_block_is_capped() {
        assert_eq!(body_max_height(100.0), 96.0);
        assert_eq!(body_max_height(500.0), 200.0);
        assert_eq!(body_max_height(2000.0), 320.0);
    }
}
