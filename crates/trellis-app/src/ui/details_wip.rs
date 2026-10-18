//! The WIP panel: the in-progress banner, the Conflicted, Resolved, Unstaged and Staged Files
//! lists with their rows' actions, and the commit box.

use super::UiCache;
use super::common::{menu_item, status_color, status_label, widget_row_height};
use super::file_menu::{self, ToolItem};
use super::theme::color;
use crate::keymap::{BindingId, Keymap};
use crate::msg::ConflictMsg;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::msg::StageMsg;
use crate::msg::WorkFileMsg;
use crate::state::AppState;
use crate::state::lfs::{TrackContext, track_items};
use crate::state::tool_compare::{RESOLVE_IN_MERGE_TOOL, ToolAvail};
use crate::tools::ToolsMsg;
use eframe::egui::{self, Align, Layout, RichText, Sense};
use trellis_core::repo::{DiffTarget, FileEntry, Snapshot};
use trellis_core::undo::DISCARD_STASH_MESSAGE;

pub(super) fn wip_panel(ui: &mut egui::Ui, st: &mut AppState, cache: &mut UiCache, msgs: &mut Vec<Msg>) {
    let Some(snap) = &st.snap else { return };
    let open = OpenFile { diff: st.diff.as_ref().map(|(t, _)| t), conflict: st.conflict.as_ref().map(|c| c.path.as_str()) };

    let conflicted: Vec<&FileEntry> = snap.unstaged.iter().filter(|f| f.status == '!').collect();
    super::details_banner::op_banner(ui, st, snap, &conflicted, msgs);
    super::path_warnings::banner(ui, snap);

    // The lists shown share what the commit box leaves: Unstaged and Staged, plus Conflicted
    // and Resolved Files while there are any.
    let resolved = st.conflict_files.resolved_rows(snap);
    let lists = 2 + usize::from(!conflicted.is_empty()) + usize::from(!resolved.is_empty());
    let list_h = ((ui.available_height() - super::commit_box::RESERVED_HEIGHT) / lists as f32).max(60.0);
    let tools = ToolAvail::of(st);
    let ctx = RowContext { lfs: TrackContext::of(st), tools: &tools, keys: &st.keymap };
    let lists = super::conflict_files::Lists { conflicted: &conflicted, resolved: &resolved, height: list_h };
    super::conflict_files::show(ui, st, lists, &open, ctx, msgs);
    file_list(ui, snap, false, &open, list_h, ctx, msgs);
    ui.add_space(8.0);
    file_list(ui, snap, true, &open, list_h, ctx, msgs);
    super::commit_box::show(ui, st, cache, msgs);
}

/// What every working-tree row needs, built once per frame: Track in Git LFS's context and
/// the diff and merge tools' availability, plus the keymap for the header buttons' tips.
#[derive(Clone, Copy)]
pub(super) struct RowContext<'a> {
    pub lfs: TrackContext<'a>,
    pub tools: &'a ToolAvail,
    pub keys: &'a Keymap,
}

/// What the central panel shows, so its file row is drawn selected.
pub(super) struct OpenFile<'a> {
    pub diff: Option<&'a DiffTarget>,
    pub conflict: Option<&'a str>,
}

/// The Discard all confirmation, naming Undo's current key.
pub(super) fn discard_all_text(keys: &Keymap) -> String {
    format!(
        "All uncommitted changes will be moved into a stash named \"{DISCARD_STASH_MESSAGE}\". Undo{} or applying that stash brings \
         them back; dropping the stash deletes them permanently.",
        keys.paren(BindingId::Undo)
    )
}

/// The Unstaged or Staged Files list of the working tree. Conflicted files have their own list
/// (`conflict_files::show`), so Unstaged Files leaves them out.
fn file_list(ui: &mut egui::Ui, snap: &Snapshot, staged: bool, open: &OpenFile, max_h: f32, ctx: RowContext, msgs: &mut Vec<Msg>) {
    let (title, all) = if staged { ("Staged Files", &snap.staged) } else { ("Unstaged Files", &snap.unstaged) };
    // Built only while files are conflicted: a working tree can have thousands of changed files.
    let kept: Option<Vec<&FileEntry>> =
        (!staged && all.iter().any(|f| f.status == '!')).then(|| all.iter().filter(|f| f.status != '!').collect());
    let count = kept.as_ref().map_or(all.len(), Vec::len);
    let keys = ctx.keys;
    // The Staged header's background opens its menu; registered before the header's buttons, so
    // they keep their own clicks.
    let sense = if staged { Sense::click() } else { Sense::hover() };
    let header = ui.scope_builder(egui::UiBuilder::new().sense(sense), |ui| {
        egui::Frame::new().fill(color::toolbar()).inner_margin(egui::Margin::symmetric(8, 5)).corner_radius(3).show(ui, |ui| {
            ui.horizontal(|ui| {
                // Not selectable, so a right-click on the title reaches the header's menu.
                let text = RichText::new(format!("{title} ({count})")).strong().color(color::text());
                ui.add(egui::Label::new(text).selectable(false));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if staged {
                        let tip = keys.tip("Unstage every file", BindingId::UnstageAll);
                        if count > 0 && ui.small_button("Unstage all").on_hover_text(tip).clicked() {
                            msgs.push(Msg::Stage(StageMsg::UnstageAll));
                        }
                        return;
                    }
                    if count > 0 {
                        let tip = keys.tip("Stage every changed file", BindingId::StageAll);
                        if ui.small_button("Stage all").on_hover_text(tip).clicked() {
                            msgs.push(Msg::Stage(StageMsg::StageAll));
                        }
                        if ui.small_button("Discard all").clicked() {
                            // Discarding goes through a stash so it can be recovered.
                            msgs.push(Msg::confirm(
                                "Discard all changes",
                                discard_all_text(keys),
                                "Discard all",
                                Msg::git("Discard all changes", &["stash", "push", "-u", "-m", DISCARD_STASH_MESSAGE]),
                            ));
                        }
                    }
                    // Drawn last, so it sits leftmost; shown even when nothing has changed.
                    if ui.small_button("New file…").clicked() {
                        msgs.push(Msg::WorkFile(WorkFileMsg::NewFileDialog(String::new())));
                    }
                });
            });
        });
    });
    if staged {
        header.response.context_menu(|ui| file_menu::staged_header_items(ui, msgs, snap));
    }
    let area = egui::ScrollArea::vertical().id_salt(title).max_height(max_h).auto_shrink([false, true]);
    if count == 0 {
        area.show(ui, |ui| ui.label(RichText::new("None").color(color::faint())));
        return;
    }
    // Only the rows in view are drawn: a working tree can have thousands of changed files.
    area.show_rows(ui, widget_row_height(ui), count, |ui, range| {
        for i in range {
            let f = kept.as_ref().map_or(&all[i], |k| k[i]);
            wip_file_row(ui, snap, f, staged, open, ctx, None, msgs);
        }
    });
}

/// A row of the Unstaged, Staged or Conflicted Files list: status, Stage / Unstage / Mark
/// resolved, the path (opens its diff, or the conflict view), `note` (weak, or green) and its
/// menu.
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
) {
    let target = || if staged { DiffTarget::Staged(f.path.clone()) } else { DiffTarget::Unstaged(f.path.clone()) };
    // A conflicted file opens in the conflict view; marking it resolved checks for markers first.
    let conflicted = !staged && f.status == '!';
    let toggle = || match (staged, conflicted) {
        (true, _) => Msg::Stage(StageMsg::UnstageFile(f.path.clone())),
        (false, true) => Msg::Conflict(ConflictMsg::MarkResolved(vec![f.path.clone()])),
        (false, false) => Msg::Stage(StageMsg::StageFile(f.path.clone())),
    };
    ui.horizontal(|ui| {
        ui.label(RichText::new(status_label(f.status).to_string()).monospace().strong().color(status_color(f.status)));
        let action = match (staged, conflicted) {
            (true, _) => "Unstage",
            (false, true) => "Mark resolved",
            (false, false) => "Stage",
        };
        // A name that isn't UTF-8 can't be named to git safely (`update` refuses it too).
        let usable = !matches!(f.hazard, Some(trellis_core::repo::Hazard::NotUtf8(_)));
        if ui.add_enabled(usable, egui::Button::new(action).small()).clicked() {
            msgs.push(toggle());
        }
        let selected = match open.diff {
            _ if conflicted => open.conflict == Some(f.path.as_str()),
            Some(DiffTarget::Staged(p)) => staged && *p == f.path,
            Some(DiffTarget::Unstaged(p)) => !staged && *p == f.path,
            _ => false,
        };
        let r = ui.selectable_label(selected, &f.path);
        super::path_warnings::mark(ui, f);
        if let Some((text, green)) = note {
            ui.label(RichText::new(text).small().color(if green { color::green() } else { color::weak() }));
        }
        if r.clicked() {
            msgs.push(if conflicted {
                Msg::Conflict(ConflictMsg::OpenConflict(f.path.clone()))
            } else {
                Msg::Repo(RepoMsg::OpenDiff(target()))
            });
        }
        r.context_menu(|ui| {
            if conflicted {
                menu_item(msgs, ui, "Open conflict", Msg::Conflict(ConflictMsg::OpenConflict(f.path.clone())));
            }
            menu_item(msgs, ui, action, toggle());
            // Checking out the index version of a conflicted path means nothing.
            if !staged && !conflicted {
                if f.status == '?' {
                    menu_item(msgs, ui, "Discard changes", Msg::WorkFile(WorkFileMsg::DiscardUntracked(f.path.clone())));
                } else {
                    menu_item(
                        msgs,
                        ui,
                        "Discard changes",
                        Msg::confirm(
                            "Discard changes",
                            format!("Discard changes to {}? You can bring them back with Undo.", f.path),
                            "Discard",
                            Msg::Stage(StageMsg::DiscardFile(f.path.clone())),
                        ),
                    );
                }
            }
            file_menu::stash_file_item(ui, msgs, snap, f);
            file_menu::patch_file_item(ui, msgs, f, staged);
            if f.status != '?' {
                file_menu::history_items(ui, msgs, &f.path, None, (f.status == 'A').then_some("Not committed yet"));
            }
            menu_item(msgs, ui, "Copy path", Msg::Copy(f.path.clone()));
            let unstages = crate::state::files::unstages_on_delete(snap, &f.path);
            let missing = snap.missing_from_workdir(&f.path);
            let items = if missing { Vec::new() } else { track_items(&f.path, ctx.lfs) };
            let tool = if conflicted {
                let msg = Msg::Tools(ToolsMsg::ResolveInMergeTool(f.path.clone()));
                ToolItem { label: RESOLVE_IN_MERGE_TOOL, msg, blocked: ctx.tools.merge.as_deref() }
            } else {
                file_menu::diff_item(target(), ctx.tools.diff)
            };
            file_menu::wip_file_items(ui, msgs, f, missing, unstages, &items, tool);
        });
    });
}
