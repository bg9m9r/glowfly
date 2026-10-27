//! The WIP panel: the file lists in a scrolling area over the commit box, which stays at the
//! bottom (its height measured the frame before). While an operation is in progress the lists
//! sit in its numbered steps (`op_steps.rs`); else Unstaged, then Staged Files, under the card of
//! an operation without steps (`details_banner.rs`). A conflicted row's Mark resolved always sits
//! at its end, a row's Stage or Unstage while the row is hovered or current; the S and U keys
//! and the row's menu reach them too.
//! The Unstaged and Staged Files headers are in `details_wip_header.rs`, the lists themselves in
//! `details_wip_list.rs`; those two lists' keyboard focus and arrows are in `wip_keys.rs`.

/// Narrower than this, a row's action is an icon and its note is left out.
const NARROW_ROW: f32 = 300.0;

#[path = "details_wip_header.rs"]
pub(super) mod header;
#[path = "details_wip_list.rs"]
mod list;
pub(super) use list::file_list;

use super::UiCache;
use super::commit_box::BoxStyle;
use super::common::{menu_item, status_color, status_label};
use super::file_menu::{self, ToolItem};
use super::op_steps::TopArgs;
use super::theme::color;
use super::widgets::{Icon, buttons};
use crate::ui::widgets::row_menu::RowMenuExt;

use crate::keymap::{BindingId, Keymap};
use crate::msg::ConflictMsg;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::msg::StageMsg;
use crate::msg::WorkFileMsg;
use crate::state::AppState;
use crate::state::custom_commands::MenuCommands;
use crate::state::file_folds::FileFolds;
use crate::state::lfs::{TrackContext, track_items};
use crate::state::op_steps::op_kind;
use crate::state::tool_compare::ToolAvail;
use crate::tools::ToolsMsg;
use crate::ui::theme::type_scale;
use eframe::egui::{self, RichText};
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{DiffTarget, FileEntry, Snapshot, WipLineCounts};
use glowfly_core::settings::FileListMode;
use glowfly_core::undo::DISCARD_STASH_MESSAGE;

pub(super) fn wip_panel(ui: &mut egui::Ui, st: &mut AppState, cache: &mut UiCache, msgs: &mut Vec<Msg>) {
    let Some(snap) = &st.snap else { return };
    // The file being opened is highlighted at once, so a click or a step answers immediately.
    let opening = st.view.diff_opening.as_ref().filter(|t| t.is_working_tree());
    let open = OpenFile {
        diff: opening.or(st.session.diff.view.as_ref().map(|(t, _)| t)),
        conflict: st.session.conflict.view.as_ref().map(|c| c.path.as_str()),
    };

    let conflicted: Vec<&FileEntry> = snap.unstaged.iter().filter(|f| f.status == '!').collect();
    let resolved = st.session.conflict_files.resolved_rows(snap);
    let kind = op_kind(snap);
    let tools = ToolAvail::of(st);
    let lines = st.session.reads.line_counts.wip();
    let commands = MenuCommands::for_state(st);
    let (lfs, keys, lang) = (TrackContext::of(st), &st.keymap, st.settings.lang());
    let (mode, folds) = (st.settings.file_list_mode, &st.session.tab_view.file_folds);
    let ctx = RowContext { lfs, tools: &tools, keys, lang, lines, commands: &commands, keyed: false, depth: None, mode, folds };
    let reveal = st.view.file_reveal.as_ref();

    // The commit box (or step 3) is pinned at the bottom at the height it took last frame; the
    // lists scroll in the room above it.
    let bottom_id = ui.id().with("wip-bottom");
    let bottom_h = ui.data(|d| d.get_temp::<f32>(bottom_id)).unwrap_or(super::commit_box::RESERVED_HEIGHT);
    let top_h = (ui.available_height() - bottom_h).max(TOP_MIN);
    let size = egui::vec2(ui.available_width(), top_h);
    ui.allocate_ui_with_layout(size, egui::Layout::top_down(egui::Align::Min), |ui| {
        ui.set_min_size(size);
        egui::ScrollArea::vertical().id_salt("wip-top").auto_shrink([false, false]).show(ui, |ui| match kind {
            Some(kind) => {
                let a = TopArgs { kind, conflicted: &conflicted, resolved: &resolved, open: &open, reveal, ctx, height: top_h };
                super::op_steps::top(ui, st, snap, a, msgs);
            }
            None => {
                super::details_banner::op_banner(ui, st, snap, msgs);
                super::path_warnings::banner(ui, lang, snap);
                // The two lists share the room left by their headers.
                let list_h = ((top_h - 2.0 * header::HEADER_H - 24.0) / 2.0).max(60.0);
                file_list(ui, snap, false, (&open, reveal), list_h, ctx, msgs);
                ui.add_space(12.0);
                file_list(ui, snap, true, (&open, reveal), list_h, ctx, msgs);
            }
        });
    });
    let bottom = ui.scope(|ui| match kind {
        Some(kind) => super::op_steps::bottom(ui, st, cache, kind, msgs),
        None => super::commit_box::show(ui, st, cache, BoxStyle { titled: true, primary: true }, msgs),
    });
    ui.data_mut(|d| d.insert_temp(bottom_id, bottom.response.rect.height()));
}

/// The least height the lists keep above the pinned commit box.
const TOP_MIN: f32 = 120.0;

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
    /// The row's level in its list's tree; None in List mode and in lists never shown as one.
    pub depth: Option<usize>,
    /// How the Unstaged and Staged Files lists show their files.
    pub mode: FileListMode,
    /// The folders collapsed in those lists' trees.
    pub folds: &'a FileFolds,
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
pub(super) type Reveal<'a> = Option<&'a (DiffTarget, u64)>;

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
    // The row's action sits at its end: always on a conflicted row (Mark resolved), else while the
    // row is hovered or current, in place of its line counts. The S and U keys, the conflict
    // view's Save and mark resolved and the row's menu reach it too.
    let at = egui::Rect::from_min_size(ui.cursor().min, egui::vec2(ui.available_width(), super::widgets::LIST_ROW_H));
    let show_action = conflicted || selected || ui.rect_contains_pointer(at);
    // A name that isn't UTF-8 can't be named to git safely (`update` refuses it too).
    let usable = !matches!(f.hazard, Some(glowfly_core::repo::Hazard::NotUtf8(_)));
    // In a narrow panel the action is a check mark named by it, and the conflict count waits
    // for the conflict view, so the file's name keeps its room.
    let narrow = at.width() < NARROW_ROW;
    let end = |ui: &mut egui::Ui| {
        let button = if narrow { buttons::icon_only(Icon::Check, action) } else { buttons::quiet(action).small() };
        if show_action {
            if ui.add(button.enabled(usable)).clicked() {
                msgs.push(toggle());
            }
        } else if let Some((n, w)) = count.zip(lines) {
            super::line_counts::row_end(ui, n, w.largest());
        }
        if let Some((text, green)) = note.as_ref().filter(|_| !narrow) {
            ui.label(RichText::new(text).size(type_scale::SMALL).color(if *green { color::text() } else { color::weak() }));
        }
    };
    let row = |ui: &mut egui::Ui| {
        super::list_row::file_tree::indent(ui, ctx.depth);
        if conflicted {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
            super::needs_you::badge(ui.painter(), rect);
        } else {
            ui.label(
                RichText::new(status_label(f.status).to_string()).font(type_scale::mono(type_scale::BODY)).color(status_color(f.status)),
            );
        }
        super::list_row::file_tree::file_text(ui, &f.path, ctx.depth, selected, 0.0);
        super::path_warnings::mark(ui, ctx.lang, f);
        super::lfs_badge::badge(ui, f);
    };
    let r = if ctx.keyed {
        super::widgets::keyed_file_row_with_end(ui, selected, &f.path, end, row)
    } else {
        super::widgets::file_row_with_end(ui, selected, &f.path, end, row)
    }
    .response;
    if r.clicked() {
        msgs.push(if conflicted {
            Msg::Conflict(ConflictMsg::OpenConflict(f.path.clone()))
        } else {
            Msg::Repo(RepoMsg::OpenDiff(target()))
        });
    }
    r.row_menu(|ui| {
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
