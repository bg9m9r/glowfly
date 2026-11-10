//! The WIP panel: the file lists in a scrolling area over the commit box, which stays at the
//! bottom (its height measured the frame before). While an operation is in progress the lists
//! sit in its numbered steps (`op_steps.rs`); else Unstaged, then Staged Files, under the card of
//! an operation without steps (`details_banner.rs`). A conflicted row's Mark resolved always sits
//! at its end, a row's Stage or Unstage while the row is hovered or current; the S and U keys
//! and the row's menu reach them too. Shift+click and Ctrl/Cmd+click pick several files of a
//! list (gaps allowed); a picked row's button and menu (`details_wip_picks.rs`) act on them all.
//! The Unstaged and Staged Files headers are in `details_wip_header.rs`, the lists themselves in
//! `details_wip_list.rs`; those two lists' keyboard focus and arrows are in `wip_keys.rs`.

/// Narrower than this, a row's action is an icon and its note is left out.
const NARROW_ROW: f32 = 300.0;

#[path = "details_wip_header.rs"]
pub(super) mod header;
#[path = "details_wip_list.rs"]
mod list;
#[path = "details_wip_picks.rs"]
pub(super) mod picks;
pub(super) use list::file_list;

use super::UiCache;
use super::commit_box::BoxStyle;
use super::common::{menu_item, status_color, status_label};
use super::file_menu::{self, ToolItem};
use super::op_steps::TopArgs;
use super::theme::color;
use super::widgets::{Icon, buttons};
use crate::theme::sizes::ControlSize;
use crate::widgets::row_menu::RowMenuExt;

use crate::theme::type_scale;
use eframe::egui::{self, RichText};
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{DiffTarget, FileEntry, Snapshot, WipLineCounts};
use glowfly_core::settings::FileListMode;
use glowfly_core::undo::DISCARD_STASH_MESSAGE;
use glowfly_model::keymap::{BindingId, Keymap};
use glowfly_model::msg::ConflictMsg;
use glowfly_model::msg::Msg;
use glowfly_model::msg::RepoMsg;
use glowfly_model::msg::StageMsg;
use glowfly_model::state::conflict_picks::{ConflictPicks, drawn_selected};
use glowfly_model::state::custom_commands::MenuCommands;
use glowfly_model::state::file_folds::FileFolds;
use glowfly_model::state::lfs::{TrackContext, track_items};
use glowfly_model::state::op_steps::op_kind;
use glowfly_model::state::tool_compare::ToolAvail;
use glowfly_model::state::wip_lists::{self, PickHow, Picked};
use glowfly_model::state::{AppState, ViewState};
use glowfly_model::tools::ToolsMsg;

pub(super) fn wip_panel(ui: &mut egui::Ui, st: &AppState, view: &mut ViewState, cache: &mut UiCache, msgs: &mut Vec<Msg>) {
    super::wip_keys::requested(ui.ctx(), view);
    let Some(snap) = &st.snap else { return };
    // The file being opened is highlighted at once, so a click or a step answers immediately.
    let opening = st.cues.diff_opening.as_ref().filter(|t| t.is_working_tree());
    let picks = [false, true].map(|staged| wip_lists::picked(st, staged));
    let conflict_picks = [false, true].map(|resolved| glowfly_model::state::conflict_picks::picked(st, resolved));
    let resolve_why = conflict_picks[0].as_ref().and_then(|p| picks::resolve_blocked(st, st.settings.lang(), &p.paths));
    let open = OpenFile {
        diff: opening.or(st.session.diff.view.as_ref().map(|(t, _)| t)),
        conflict: st.session.conflict.view.as_ref().map(|c| c.path.as_str()),
        picks: [picks[0].as_ref(), picks[1].as_ref()],
        conflict_picks: [conflict_picks[0].as_ref(), conflict_picks[1].as_ref()],
        conflict_held: [false, true].map(|resolved| glowfly_model::state::conflict_picks::held(st, resolved)),
        resolve_why: resolve_why.as_deref(),
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
    let reveal = st.cues.file_reveal.as_ref();

    // The commit box (or step 3) is pinned at the bottom at the height it took last frame; the
    // lists scroll in the room above it.
    let bottom_id = ui.id().with("wip-bottom");
    let bottom_h = ui.data(|d| d.get_temp::<f32>(bottom_id)).unwrap_or_else(|| {
        let why = glowfly_model::state::commit::commit_blocked(st, false);
        super::commit_box::reserved_height(crate::widgets::reason::lines(ui, why.as_deref(), ui.available_width()))
    });
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
        Some(kind) => super::op_steps::bottom(ui, st, view, cache, kind, msgs),
        None => super::commit_box::show(ui, st, view, cache, BoxStyle { titled: true, primary: true }, msgs),
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
    /// The files picked with Shift or Ctrl/Cmd in Unstaged, then Staged Files, when several.
    pub picks: [Option<&'a Picked>; 2],
    /// The same in Conflicted, then Resolved Files.
    pub conflict_picks: [Option<&'a Picked>; 2],
    /// The picks Conflicted, then Resolved Files hold, however many (for the rows drawn selected).
    pub conflict_held: [Option<&'a ConflictPicks>; 2],
    /// Why Mark resolved is off for the Conflicted Files picks, if it is.
    pub resolve_why: Option<&'a str>,
}

/// The Discard all confirmation, naming Undo's current key.
pub(super) fn discard_all_text(t: Lang, keys: &Keymap) -> String {
    match keys.hint(BindingId::Undo) {
        Some(k) => t.trf("details.wip.discard_all.body_key", &[("stash", &DISCARD_STASH_MESSAGE), ("key", &k)]),
        None => t.trf("details.wip.discard_all.body", &[("stash", &DISCARD_STASH_MESSAGE)]),
    }
}

/// How a click held picks files: Shift picks a range from the anchor, Ctrl (Cmd on macOS) adds
/// or removes one file; None for a plain click.
pub(super) fn pick_how(ui: &egui::Ui) -> Option<PickHow> {
    let m = ui.input(|i| i.modifiers);
    if m.shift { Some(PickHow::Range) } else { m.command.then_some(PickHow::Toggle) }
}

/// The file a step landed on, for its list to scroll to (`ViewCues::file_reveal`).
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
    // One of the files picked with Shift or Ctrl/Cmd in its list (Conflicted Files has its own
    // picks): its button, menu and keys act on every pick.
    let list_picks = if conflicted { open.conflict_picks[0] } else { open.picks[usize::from(staged)] };
    let picked = list_picks.is_some_and(|p| p.set.contains(&f.path));
    let many = list_picks.filter(|_| picked);
    let toggle = || match (staged, conflicted, many) {
        (true, _, Some(p)) => Msg::Stage(StageMsg::UnstagePaths(p.paths.clone())),
        (false, false, Some(p)) => Msg::Stage(StageMsg::StagePaths(p.paths.clone())),
        (false, true, Some(p)) => Msg::Conflict(ConflictMsg::MarkResolved(p.paths.clone())),
        (true, _, _) => Msg::Stage(StageMsg::UnstageFile(f.path.clone())),
        (false, true, _) => Msg::Conflict(ConflictMsg::MarkResolved(vec![f.path.clone()])),
        (false, false, _) => Msg::Stage(StageMsg::StageFile(f.path.clone())),
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
    let current = match open.diff {
        _ if conflicted => open.conflict == Some(f.path.as_str()),
        Some(DiffTarget::Staged(p)) => staged && *p == f.path,
        Some(DiffTarget::Unstaged(p)) => !staged && *p == f.path,
        _ => false,
    };
    // A conflicted pick opens nothing, so the open file may be out of the picks: then it isn't
    // drawn selected, as the picks' actions leave it out.
    let selected = if conflicted { drawn_selected(open.conflict_held[0], &f.path, current) } else { current || picked };
    // The row's action sits at its end: always on a conflicted row (Mark resolved), else while the
    // row is hovered or current, in place of its line counts. The S and U keys, the conflict
    // view's Mark resolved and the row's menu reach it too.
    let at = egui::Rect::from_min_size(ui.cursor().min, egui::vec2(ui.available_width(), super::widgets::LIST_ROW_H));
    let show_action = conflicted || current || ui.rect_contains_pointer(at);
    // A name that isn't UTF-8 can't be named to git safely (`update` refuses it too).
    let usable = !matches!(f.hazard, Some(glowfly_core::repo::Hazard::NotUtf8(_)));
    // In a narrow panel the action is a check mark named by it, and the conflict count waits
    // for the conflict view, so the file's name keeps its room.
    let narrow = at.width() < NARROW_ROW;
    let end = |ui: &mut egui::Ui| {
        let button = if narrow { buttons::icon_only(Icon::Check, action) } else { buttons::quiet(action).size(ControlSize::Small) };
        if show_action {
            let why = many.and(open.resolve_why).filter(|_| conflicted);
            let b = ui.add(button.enabled(usable && why.is_none()).why(why));
            let b = match many {
                _ if why.is_some() => b,
                Some(p) if staged => b.on_hover_text(t.trn("details.wip.picks.unstage.tip", p.paths.len(), &[])),
                Some(p) if conflicted => b.on_hover_text(t.trn("details.wip.picks.resolve.tip", p.paths.len(), &[])),
                Some(p) => b.on_hover_text(t.trn("details.wip.picks.stage.tip", p.paths.len(), &[])),
                None => b,
            };
            if b.clicked() {
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
            let letter = ui.label(
                RichText::new(status_label(f.status).to_string()).font(type_scale::mono(type_scale::BODY)).color(status_color(f.status)),
            );
            super::file_status::name_status(&letter, ctx.lang, f.status);
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
        msgs.push(match pick_how(ui) {
            // A conflicted pick doesn't open the file: the conflict view may hold unsaved choices.
            Some(how) if conflicted => Msg::Conflict(ConflictMsg::Pick { resolved: false, path: f.path.clone(), how }),
            Some(how) => Msg::Repo(RepoMsg::PickWipFile { staged, path: f.path.clone(), how }),
            None if conflicted => Msg::Conflict(ConflictMsg::OpenConflict(f.path.clone())),
            None => Msg::Repo(RepoMsg::OpenDiff(target())),
        });
    }
    // Shift+F10 opens the selected file's menu while its list has the keyboard.
    let keyboard = current
        && if conflicted {
            super::file_list_keys::focused(ui.ctx()) == Some(glowfly_model::state::list_keys::FileList::Conflicted)
        } else {
            super::wip_keys::focused_list(ui.ctx()) == Some(staged)
        };
    r.row_menu_for(keyboard, |ui| {
        match many {
            Some(p) if conflicted => return picks::conflicted_menu(ui, t, msgs, &p.paths, open.resolve_why),
            Some(p) => return picks::menu(ui, t, msgs, staged, &p.paths, &picks::PickBlocks::of(t, snap, &p.paths)),
            None => {}
        }
        if conflicted {
            menu_item(msgs, ui, t.tr("details.wip.open_conflict"), Msg::Conflict(ConflictMsg::OpenConflict(f.path.clone())));
        }
        menu_item(msgs, ui, action, toggle());
        // Checking out the index version of a conflicted path means nothing. Both ask first.
        let discard = (!staged && !conflicted).then(|| glowfly_model::state::files::discard_msg(t, f));
        file_menu::stash_file_item(ui, t, msgs, snap, f);
        file_menu::patch_file_item(ui, t, msgs, f, staged);
        if f.status != '?' {
            file_menu::history_items(ui, t, msgs, &f.path, None, (f.status == 'A').then_some(t.tr("details.wip.not_committed")));
        }
        menu_item(msgs, ui, t.tr("common.copy_path"), Msg::Copy(f.path.clone()));
        let unstages = glowfly_model::state::files::unstages_on_delete(snap, &f.path);
        let missing = snap.missing_from_workdir(&f.path);
        let items = if missing { Vec::new() } else { track_items(t, &f.path, ctx.lfs) };
        let tool = if conflicted {
            let msg = Msg::Tools(ToolsMsg::ResolveInMergeTool(f.path.clone()));
            let label = t.tr("tools.resolve_in_merge_tool");
            ToolItem { label, msg, blocked: ctx.tools.merge.as_deref(), setup: ctx.tools.setup.merge }
        } else {
            file_menu::diff_item(t, target(), ctx.tools.setup.diff)
        };
        file_menu::wip_file_items(ui, t, msgs, f, missing, unstages, &items, ctx.tools.setup.editor, tool, ctx.commands, discard);
    });
    r
}
