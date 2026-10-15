//! Right panel: staging and committing (WIP), or the selected commit's details.

use super::UiCache;
use super::common::{issue_chip, lane_color, menu_item, short, state_actions, status_color, status_label, widget_row_height};
use super::file_menu::{self, RowSource};
use super::graph::draw_avatar;
use super::theme::color;
use crate::keymap::{BindingId, hint};
use crate::msg::ConflictMsg;
use crate::msg::HistoryMsg;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::msg::StageMsg;
use crate::msg::WorkFileMsg;
use crate::state::lfs::{TrackContext, track_items};
use crate::state::{AppState, Selection};
use eframe::egui::{self, Align, Layout, RichText, Sense, vec2};
use git2::Oid;
use trellis_core::repo::{DiffTarget, FileEntry, Snapshot};
use trellis_core::undo::DISCARD_STASH_MESSAGE;

pub fn show(ctx: &egui::Context, st: &mut AppState, cache: &mut UiCache, msgs: &mut Vec<Msg>) {
    egui::SidePanel::right("detail")
        .resizable(true)
        .default_width(400.0)
        .max_width(640.0)
        .frame(egui::Frame::new().fill(color::panel()).inner_margin(egui::Margin::same(10)))
        .show(ctx, |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
            match st.selection {
                Selection::Wip => wip_panel(ui, st, cache, msgs),
                Selection::Commit(_) => commit_panel(ui, st, cache, msgs),
                Selection::Commits(_) => super::details_multi::panel(ui, st, msgs),
                Selection::None => {
                    ui.label(RichText::new("Select a commit to see its details.").color(color::weak()));
                    if st.snap.as_ref().is_some_and(|s| s.is_dirty()) {
                        ui.add_space(8.0);
                        if ui.button("View uncommitted changes").clicked() {
                            msgs.push(Msg::Repo(RepoMsg::SelectWip));
                        }
                    }
                }
            }
        });
}

fn wip_panel(ui: &mut egui::Ui, st: &mut AppState, cache: &mut UiCache, msgs: &mut Vec<Msg>) {
    let Some(snap) = &st.snap else { return };
    let open = OpenFile { diff: st.diff.as_ref().map(|(t, _)| t), conflict: st.conflict.as_ref().map(|c| c.path.as_str()) };

    // An operation in progress, or conflicts left by one that has none (a stash apply).
    let conflicted: Vec<&FileEntry> = snap.unstaged.iter().filter(|f| f.status == '!').collect();
    let in_progress = state_actions(snap.state);
    if in_progress.is_some() || !conflicted.is_empty() {
        egui::Frame::group(ui.style()).fill(color::banner()).show(ui, |ui| {
            if let Some((name, _)) = in_progress {
                ui.label(RichText::new(format!("{name} in progress")).strong());
            }
            if let Some(first) = conflicted.first() {
                ui.horizontal(|ui| {
                    let n = conflicted.len();
                    ui.label(format!("{n} conflicted file{}", if n == 1 { "" } else { "s" }));
                    if ui.button("View conflicts").clicked() {
                        msgs.push(Msg::Conflict(ConflictMsg::OpenConflict(first.path.clone())));
                    }
                });
            }
            let Some((_, actions)) = in_progress else { return };
            ui.horizontal(|ui| {
                for (label, a) in actions {
                    if ui.button(*label).clicked() {
                        msgs.push(Msg::git(label, a));
                    }
                }
            });
        });
        ui.add_space(6.0);
    }

    let list_h = ((ui.available_height() - super::commit_box::RESERVED_HEIGHT) / 2.0).max(80.0);
    let lfs = TrackContext::of(st);
    file_list(ui, snap, false, &open, list_h, lfs, msgs);
    ui.add_space(8.0);
    file_list(ui, snap, true, &open, list_h, lfs, msgs);
    super::commit_box::show(ui, st, cache, msgs);
}

fn commit_panel(ui: &mut egui::Ui, st: &AppState, cache: &mut UiCache, msgs: &mut Vec<Msg>) {
    let Some(d) = &st.detail else {
        ui.label(RichText::new("Loading…").color(color::weak()));
        return;
    };
    let open = st.diff.as_ref().map(|(t, _)| t);
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("commit {}", short(d.oid))).monospace().color(color::weak()));
        if ui.small_button("Copy SHA").clicked() {
            msgs.push(Msg::Copy(d.oid.to_string()));
        }
    });
    let (summary, body) = d.message.split_once('\n').unwrap_or((&d.message, ""));
    ui.add(egui::Label::new(RichText::new(summary).strong().size(15.0).color(color::text())).wrap());
    if !body.trim().is_empty() {
        egui::ScrollArea::vertical().id_salt("commit-body").max_height(180.0).show(ui, |ui| {
            ui.add(egui::Label::new(RichText::new(body.trim()).color(color::weak())).wrap());
        });
    }

    if let Some(fs) = &st.forge {
        let keys = fs.issue_refs(&d.message);
        if !keys.is_empty() {
            ui.add_space(6.0);
            ui.label(RichText::new(format!("{}S", fs.issue_noun().to_uppercase())).size(11.0).color(color::weak()));
            for k in keys {
                issue_chip(ui, fs, &k, msgs);
            }
        }
    }

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::hover());
        let av = if st.settings.use_initials { None } else { Some(&mut cache.avatars) };
        draw_avatar(ui.painter(), rect.center(), 15.0, &d.author, &d.email, lane_color(0), av);
        ui.vertical(|ui| {
            ui.label(RichText::new(format!("{} <{}>", d.author, d.email)).color(color::text()));
            let mut when = st.settings.format_verbose(d.time, st.view.utc_offset);
            if d.committer != d.author {
                when.push_str(&format!(" · committed by {}", d.committer));
            }
            ui.label(RichText::new(when).color(color::weak()));
        });
    });
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("parents").color(color::weak()));
        for p in &d.parents {
            if ui.link(RichText::new(short(*p)).monospace()).clicked() {
                msgs.push(Msg::Repo(RepoMsg::JumpTo(*p)));
            }
        }
    });
    ui.separator();
    let n = d.files.len();
    ui.label(RichText::new(format!("{n} changed file{}", if n == 1 { "" } else { "s" })).strong().color(color::text()));
    let lfs = TrackContext::of(st);
    let row_h = widget_row_height(ui);
    egui::ScrollArea::vertical().auto_shrink([false, false]).show_rows(ui, row_h, d.files.len(), |ui, range| {
        for f in &d.files[range] {
            let actions = Some((RowSource::Commit(d.oid), d.in_workdir(&f.path), lfs));
            changed_file_row(ui, f, DiffTarget::Commit(d.oid, f.path.clone()), open, d.oid, actions, msgs);
        }
    });
}

/// A file changed by a commit (or between two): its status and path, which opens `target`.
/// Its menu opens the file's history as seen from `history_at`. With `actions` (where the change
/// comes from, whether the path is in the working tree, and what Track in Git LFS needs) it also
/// has the file actions.
pub(super) fn changed_file_row(
    ui: &mut egui::Ui,
    f: &FileEntry,
    target: DiffTarget,
    open: Option<&DiffTarget>,
    history_at: Oid,
    actions: Option<(RowSource, bool, TrackContext<'_>)>,
    msgs: &mut Vec<Msg>,
) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(status_label(f.status).to_string()).monospace().strong().color(status_color(f.status)));
        let r = ui.selectable_label(open == Some(&target), &f.path);
        if r.clicked() {
            msgs.push(Msg::Repo(RepoMsg::OpenDiff(target)));
        }
        r.context_menu(|ui| {
            menu_item(msgs, ui, "File history", Msg::History(HistoryMsg::OpenFileHistory { path: f.path.clone(), at: Some(history_at) }));
            menu_item(msgs, ui, "Copy path", Msg::Copy(f.path.clone()));
            if let Some((src, in_wd, lfs)) = actions {
                let items = if in_wd { track_items(&f.path, lfs) } else { Vec::new() };
                file_menu::commit_file_items(ui, msgs, src, f, in_wd, &items);
            }
        });
    });
}

/// What the central panel shows, so its file row is drawn selected.
struct OpenFile<'a> {
    diff: Option<&'a DiffTarget>,
    conflict: Option<&'a str>,
}

/// The Unstaged or Staged Files list of the working tree.
fn file_list(ui: &mut egui::Ui, snap: &Snapshot, staged: bool, open: &OpenFile, max_h: f32, lfs: TrackContext, msgs: &mut Vec<Msg>) {
    let (title, files) = if staged { ("Staged Files", &snap.staged) } else { ("Unstaged Files", &snap.unstaged) };
    egui::Frame::new().fill(color::toolbar()).inner_margin(egui::Margin::symmetric(8, 5)).corner_radius(3).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{title} ({})", files.len())).strong().color(color::text()));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if staged {
                    let tip = format!("Unstage every file ({})", hint(BindingId::UnstageAll));
                    if !files.is_empty() && ui.small_button("Unstage all").on_hover_text(tip).clicked() {
                        msgs.push(Msg::Stage(StageMsg::UnstageAll));
                    }
                    return;
                }
                if !files.is_empty() {
                    let tip = format!("Stage every changed file ({})", hint(BindingId::StageAll));
                    if ui.small_button("Stage all").on_hover_text(tip).clicked() {
                        msgs.push(Msg::Stage(StageMsg::StageAll));
                    }
                    if ui.small_button("Discard all").clicked() {
                        // Discarding goes through a stash so it can be recovered.
                        msgs.push(Msg::confirm(
                            "Discard all changes",
                            format!(
                                "All uncommitted changes will be moved into a stash named \"{DISCARD_STASH_MESSAGE}\". Undo (Ctrl+Z) \
                                 or applying that stash brings them back; dropping the stash deletes them permanently."
                            ),
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
    let area = egui::ScrollArea::vertical().id_salt(title).max_height(max_h).auto_shrink([false, true]);
    if files.is_empty() {
        area.show(ui, |ui| ui.label(RichText::new("None").color(color::faint())));
        return;
    }
    // Only the rows in view are drawn: a working tree can have thousands of changed files.
    area.show_rows(ui, widget_row_height(ui), files.len(), |ui, range| {
        for f in &files[range] {
            wip_file_row(ui, snap, f, staged, open, lfs, msgs);
        }
    });
}

/// A row of the Unstaged or Staged Files list: status, Stage / Unstage / Mark resolved, the
/// path (opens its diff, or the conflict view) and its menu.
fn wip_file_row(ui: &mut egui::Ui, snap: &Snapshot, f: &FileEntry, staged: bool, open: &OpenFile, lfs: TrackContext, msgs: &mut Vec<Msg>) {
    let target = || if staged { DiffTarget::Staged(f.path.clone()) } else { DiffTarget::Unstaged(f.path.clone()) };
    let toggle =
        || if staged { Msg::Stage(StageMsg::UnstageFile(f.path.clone())) } else { Msg::Stage(StageMsg::StageFile(f.path.clone())) };
    // A conflicted file opens in the conflict view; staging it marks it resolved.
    let conflicted = !staged && f.status == '!';
    ui.horizontal(|ui| {
        ui.label(RichText::new(status_label(f.status).to_string()).monospace().strong().color(status_color(f.status)));
        let action = match (staged, conflicted) {
            (true, _) => "Unstage",
            (false, true) => "Mark resolved",
            (false, false) => "Stage",
        };
        if ui.small_button(action).clicked() {
            msgs.push(toggle());
        }
        let selected = match open.diff {
            _ if conflicted => open.conflict == Some(f.path.as_str()),
            Some(DiffTarget::Staged(p)) => staged && *p == f.path,
            Some(DiffTarget::Unstaged(p)) => !staged && *p == f.path,
            _ => false,
        };
        let r = ui.selectable_label(selected, &f.path);
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
            if f.status != '?' {
                menu_item(msgs, ui, "File history", Msg::History(HistoryMsg::OpenFileHistory { path: f.path.clone(), at: None }));
            }
            menu_item(msgs, ui, "Copy path", Msg::Copy(f.path.clone()));
            let unstages = crate::state::files::unstages_on_delete(snap, &f.path);
            let missing = snap.missing_from_workdir(&f.path);
            let items = if missing { Vec::new() } else { track_items(&f.path, lfs) };
            file_menu::wip_file_items(ui, msgs, f, missing, unstages, &items);
        });
    });
}
