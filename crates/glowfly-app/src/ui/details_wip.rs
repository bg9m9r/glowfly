//! The WIP panel: the in-progress banner, the Conflicted, Resolved, Unstaged and Staged Files
//! lists with their rows' actions, and the commit box. The Unstaged and Staged Files headers are
//! in `details_wip_header.rs`; those two lists' keyboard focus and arrows are in `wip_keys.rs`.

#[path = "details_wip_header.rs"]
mod header;

use super::UiCache;
use super::common::{menu_item, status_color, status_label};
use super::file_menu::{self, ToolItem};
use super::theme::color;
use crate::keymap::{BindingId, Keymap};
use crate::msg::ConflictMsg;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::msg::StageMsg;
use crate::msg::WorkFileMsg;
use crate::state::AppState;
use crate::state::custom_commands::MenuCommands;
use crate::state::lfs::{TrackContext, track_items};
use crate::state::tool_compare::ToolAvail;
use crate::tools::ToolsMsg;
use crate::ui::theme::type_scale;
use eframe::egui::{self, RichText};
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{DiffTarget, FileEntry, Snapshot, WipLineCounts};
use glowfly_core::undo::DISCARD_STASH_MESSAGE;

pub(super) fn wip_panel(ui: &mut egui::Ui, st: &mut AppState, cache: &mut UiCache, msgs: &mut Vec<Msg>) {
    let Some(snap) = &st.snap else { return };
    // The file being opened is highlighted at once, so a click or a step answers immediately.
    let opening = st.view.diff_opening.as_ref().filter(|t| t.is_working_tree());
    let open = OpenFile { diff: opening.or(st.diff.as_ref().map(|(t, _)| t)), conflict: st.conflict.as_ref().map(|c| c.path.as_str()) };

    let conflicted: Vec<&FileEntry> = snap.unstaged.iter().filter(|f| f.status == '!').collect();
    super::details_banner::op_banner(ui, st, snap, &conflicted, msgs);
    super::path_warnings::banner(ui, st.settings.lang(), snap);

    // The lists shown share what the commit box leaves: Unstaged and Staged, plus Conflicted
    // and Resolved Files while there are any.
    let resolved = st.conflict_files.resolved_rows(snap);
    let lists = 2 + usize::from(!conflicted.is_empty()) + usize::from(!resolved.is_empty());
    let list_h = ((ui.available_height() - super::commit_box::RESERVED_HEIGHT) / lists as f32).max(60.0);
    let tools = ToolAvail::of(st);
    let lines = st.line_counts.wip();
    let commands = MenuCommands::for_state(st);
    let (lfs, keys, lang) = (TrackContext::of(st), &st.keymap, st.settings.lang());
    let ctx = RowContext { lfs, tools: &tools, keys, lang, lines, commands: &commands, keyed: false };
    let lists = super::conflict_files::Lists { conflicted: &conflicted, resolved: &resolved, height: list_h };
    super::conflict_files::show(ui, st, lists, &open, ctx, msgs);
    let reveal = st.view.file_reveal.as_ref();
    file_list(ui, snap, false, (&open, reveal), list_h, ctx, msgs);
    ui.add_space(8.0);
    file_list(ui, snap, true, (&open, reveal), list_h, ctx, msgs);
    super::commit_box::show(ui, st, cache, msgs);
}

/// What every working-tree row needs, built once per frame: Track in Git LFS's context and
/// the diff and merge tools' availability, the keymap for the header buttons' tips, the
/// language the rows are drawn in, and the working tree's line counts (once read).
#[derive(Clone, Copy)]
pub(super) struct RowContext<'a> {
    pub lfs: TrackContext<'a>,
    pub tools: &'a ToolAvail,
    pub keys: &'a Keymap,
    pub lang: Lang,
    pub lines: Option<&'a WipLineCounts>,
    /// The custom commands of the file menus.
    pub commands: &'a MenuCommands<'a>,
    /// The row's list takes the keyboard focus as a whole (Unstaged and Staged Files), so the
    /// row is no Tab stop of its own.
    pub keyed: bool,
}

/// What the central panel shows, so its file row is drawn selected.
pub(super) struct OpenFile<'a> {
    pub diff: Option<&'a DiffTarget>,
    pub conflict: Option<&'a str>,
}

/// The Discard all confirmation, naming Undo's current key.
pub(super) fn discard_all_text(t: Lang, keys: &Keymap) -> String {
    match keys.hint(BindingId::Undo) {
        Some(k) => t.trf("details.wip.discard_all.body_key", &[("stash", &DISCARD_STASH_MESSAGE), ("key", &k)]),
        None => t.trf("details.wip.discard_all.body", &[("stash", &DISCARD_STASH_MESSAGE)]),
    }
}

/// The file a step landed on, for its list to scroll to (`ViewState::file_reveal`).
type Reveal<'a> = Option<&'a (DiffTarget, u64)>;

/// The Unstaged or Staged Files list of the working tree. Conflicted files have their own list
/// (`conflict_files::show`), so Unstaged Files leaves them out. It scrolls to the file a step
/// landed on. The list takes the keyboard focus as one Tab stop, or by a click on a row; while
/// it has it, Up/Down step between its files and its current row (or, with none, the list) is
/// ringed.
fn file_list(
    ui: &mut egui::Ui,
    snap: &Snapshot,
    staged: bool,
    (open, want): (&OpenFile, Reveal),
    max_h: f32,
    ctx: RowContext,
    msgs: &mut Vec<Msg>,
) {
    // The English titles stay the lists' scroll ids, whatever the language.
    let (id, all) = if staged { ("Staged files", &snap.staged) } else { ("Unstaged files", &snap.unstaged) };
    // Built only while files are conflicted: a working tree can have thousands of changed files.
    let kept: Option<Vec<&FileEntry>> =
        (!staged && all.iter().any(|f| f.status == '!')).then(|| all.iter().filter(|f| f.status != '!').collect());
    let count = kept.as_ref().map_or(all.len(), Vec::len);
    let t = ctx.lang;
    let title = if staged {
        t.trf("details.wip.staged.header", &[("count", &count)])
    } else {
        t.trf("details.wip.unstaged.header", &[("count", &count)])
    };
    let header = header::header(ui, title.clone(), staged, count, ctx, msgs);
    if staged {
        header.context_menu(|ui| file_menu::staged_header_items(ui, t, msgs, snap));
    }
    let row_h = super::widgets::LIST_ROW_H;
    // The list's area, for its focus target: registered before the rows, after the header.
    let h = if count == 0 { row_h } else { (count as f32 * (row_h + ui.spacing().item_spacing.y)).min(max_h) };
    let focus =
        super::wip_keys::target(ui, staged, egui::Rect::from_min_size(ui.cursor().min, egui::vec2(ui.available_width(), h)), &title);
    super::wip_keys::arrows(ui, staged, msgs);
    let focused = focus.has_focus();
    let area = egui::ScrollArea::vertical().id_salt(id).max_height(max_h).auto_shrink([false, true]);
    if count == 0 {
        let shown = area.show(ui, |ui| ui.label(RichText::new(t.tr("details.wip.none")).color(color::faint())));
        if focused {
            super::widgets::file_row_focus(ui, shown_rect(&shown));
        }
        return;
    }
    let entry = |i: usize| kept.as_ref().map_or(&all[i], |k| k[i]);
    let index = |t: &DiffTarget| {
        let path = match t {
            DiffTarget::Staged(p) if staged => p,
            DiffTarget::Unstaged(p) if !staged => p,
            _ => return None,
        };
        (0..count).find(|&i| entry(i).path == *path)
    };
    // Found in the whole list, so a current row scrolled out of view doesn't ring the list.
    let current = open.diff.and_then(index);
    let ctx = RowContext { keyed: true, ..ctx };
    // Only the rows in view are drawn: a working tree can have thousands of changed files.
    let shown = area.show_rows(ui, row_h, count, |ui, range| {
        super::details::file_reveal::reveal(ui, id, want, index, &range, row_h);
        for i in range {
            let r = wip_file_row(ui, snap, entry(i), staged, open, ctx, None, msgs);
            if r.clicked() {
                super::wip_keys::request(ui.ctx(), staged);
            }
            if focused && current == Some(i) {
                super::widgets::file_row_focus(ui, r.rect);
            }
        }
    });
    if focused && current.is_none() {
        super::widgets::file_row_focus(ui, shown_rect(&shown));
    }
}

/// The part of a scroll area its content fills (the viewport can be taller).
fn shown_rect<R>(out: &egui::scroll_area::ScrollAreaOutput<R>) -> egui::Rect {
    let r = out.inner_rect;
    egui::Rect::from_min_size(r.min, egui::vec2(r.width(), out.content_size.y.min(r.height())))
}

/// A row of the Unstaged, Staged or Conflicted Files list: status, Stage / Unstage / Mark
/// resolved, the path (opens its diff, or the conflict view), `note` (weak, or in text when
/// it is good news) and its menu. A conflicted file's status is the coral needs-you badge.
#[allow(clippy::too_many_arguments)]
pub(super) fn wip_file_row(
    ui: &mut egui::Ui,
    snap: &Snapshot,
    f: &FileEntry,
    staged: bool,
    open: &OpenFile,
    ctx: RowContext,
    note: Option<(String, bool)>,
    msgs: &mut Vec<Msg>,
) -> egui::Response {
    let target = || if staged { DiffTarget::Staged(f.path.clone()) } else { DiffTarget::Unstaged(f.path.clone()) };
    // A conflicted file opens in the conflict view; marking it resolved checks for markers first.
    let conflicted = !staged && f.status == '!';
    let toggle = || match (staged, conflicted) {
        (true, _) => Msg::Stage(StageMsg::UnstageFile(f.path.clone())),
        (false, true) => Msg::Conflict(ConflictMsg::MarkResolved(vec![f.path.clone()])),
        (false, false) => Msg::Stage(StageMsg::StageFile(f.path.clone())),
    };
    let t = ctx.lang;
    // Conflicted rows never show counts; bars of both lists share one scale.
    let lines = ctx.lines.filter(|_| !conflicted);
    let count = lines.and_then(|w| if staged { w.staged.get(&f.path) } else { w.unstaged.get(&f.path) });
    let action = match (staged, conflicted) {
        (true, _) => t.tr("details.wip.unstage.button"),
        (false, true) => t.tr("details.wip.mark_resolved.button"),
        (false, false) => t.tr("details.wip.stage.button"),
    };
    let selected = match open.diff {
        _ if conflicted => open.conflict == Some(f.path.as_str()),
        Some(DiffTarget::Staged(p)) => staged && *p == f.path,
        Some(DiffTarget::Unstaged(p)) => !staged && *p == f.path,
        _ => false,
    };
    let row = |ui: &mut egui::Ui| {
        if conflicted {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
            super::needs_you::badge(ui.painter(), rect);
        } else {
            ui.label(
                RichText::new(status_label(f.status).to_string()).font(type_scale::mono(type_scale::BODY)).color(status_color(f.status)),
            );
        }
        // A name that isn't UTF-8 can't be named to git safely (`update` refuses it too).
        let usable = !matches!(f.hazard, Some(glowfly_core::repo::Hazard::NotUtf8(_)));
        if ui.add_enabled(usable, egui::Button::new(action).small()).clicked() {
            msgs.push(toggle());
        }
        super::widgets::row_text(ui, RichText::new(&f.path).monospace(), selected);
        super::path_warnings::mark(ui, ctx.lang, f);
        super::lfs_badge::badge(ui, f);
        if let Some((text, green)) = note {
            ui.label(RichText::new(text).size(type_scale::SMALL).color(if green { color::text() } else { color::weak() }));
        }
    };
    let r = match (count.zip(lines), ctx.keyed) {
        (Some((n, w)), false) => {
            super::widgets::file_row_with_end(ui, selected, &f.path, |ui| super::line_counts::row_end(ui, n, w.largest()), row)
        }
        (Some((n, w)), true) => {
            super::widgets::keyed_file_row_with_end(ui, selected, &f.path, |ui| super::line_counts::row_end(ui, n, w.largest()), row)
        }
        (None, false) => super::widgets::file_row(ui, selected, &f.path, row),
        (None, true) => super::widgets::keyed_file_row(ui, selected, &f.path, row),
    }
    .response;
    if r.clicked() {
        msgs.push(if conflicted {
            Msg::Conflict(ConflictMsg::OpenConflict(f.path.clone()))
        } else {
            Msg::Repo(RepoMsg::OpenDiff(target()))
        });
    }
    r.context_menu(|ui| {
        if conflicted {
            menu_item(msgs, ui, t.tr("details.wip.open_conflict"), Msg::Conflict(ConflictMsg::OpenConflict(f.path.clone())));
        }
        menu_item(msgs, ui, action, toggle());
        // Checking out the index version of a conflicted path means nothing.
        if !staged && !conflicted {
            if f.status == '?' {
                menu_item(msgs, ui, t.tr("details.wip.discard.item"), Msg::WorkFile(WorkFileMsg::DiscardUntracked(f.path.clone())));
            } else {
                menu_item(
                    msgs,
                    ui,
                    t.tr("details.wip.discard.item"),
                    Msg::confirm_danger(
                        t.tr("details.wip.discard.title"),
                        t.trf("details.wip.discard.body", &[("path", &f.path)]),
                        t.tr("details.wip.discard.button"),
                        Msg::Stage(StageMsg::DiscardFile(f.path.clone())),
                    ),
                );
            }
        }
        file_menu::stash_file_item(ui, t, msgs, snap, f);
        file_menu::patch_file_item(ui, t, msgs, f, staged);
        if f.status != '?' {
            file_menu::history_items(ui, t, msgs, &f.path, None, (f.status == 'A').then_some(t.tr("details.wip.not_committed")));
        }
        menu_item(msgs, ui, t.tr("common.copy_path"), Msg::Copy(f.path.clone()));
        let unstages = crate::state::files::unstages_on_delete(snap, &f.path);
        let missing = snap.missing_from_workdir(&f.path);
        let items = if missing { Vec::new() } else { track_items(t, &f.path, ctx.lfs) };
        let tool = if conflicted {
            let msg = Msg::Tools(ToolsMsg::ResolveInMergeTool(f.path.clone()));
            let label = t.tr("tools.resolve_in_merge_tool");
            ToolItem { label, msg, blocked: ctx.tools.merge.as_deref(), setup: ctx.tools.setup.merge }
        } else {
            file_menu::diff_item(t, target(), ctx.tools.setup.diff)
        };
        file_menu::wip_file_items(ui, t, msgs, f, missing, unstages, &items, ctx.tools.setup.editor, tool, ctx.commands);
    });
    r
}
