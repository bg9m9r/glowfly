//! Right panel: staging and committing (WIP, `details_wip.rs`), the selected commit's details,
//! several selected commits (`details_multi.rs`) or a commit compared with the working
//! directory (`details_workdir.rs`).

use super::UiCache;
use super::common::{issue_chip, lane_color, menu_item, short, status_color, status_label, widget_row_height};
use super::file_menu::{self, RowSource};
use super::graph::draw_avatar;
use super::theme::color;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::state::lfs::{TrackContext, track_items};
use crate::state::{AppState, Selection};
use eframe::egui::{self, Align, Layout, RichText, Sense, vec2};
use git2::Oid;
use trellis_core::repo::{DiffTarget, FileEntry};

pub fn show(ctx: &egui::Context, st: &mut AppState, cache: &mut UiCache, msgs: &mut Vec<Msg>) {
    egui::SidePanel::right("detail")
        .resizable(true)
        .default_width(400.0)
        .max_width(640.0)
        .frame(egui::Frame::new().fill(color::panel()).inner_margin(egui::Margin::same(10)))
        .show(ctx, |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
            match st.selection {
                Selection::Wip => super::details_wip::wip_panel(ui, st, cache, msgs),
                Selection::Commit(_) => commit_panel(ui, st, cache, msgs),
                Selection::Commits(_) => super::details_multi::panel(ui, st, msgs),
                Selection::VsWorkdir(_) => super::details_workdir::panel(ui, st, msgs),
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
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| super::signature_badge::show(ui, st, d, msgs));
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
    let diff_blocked = crate::state::tool_compare::diff_blocked(&st.settings.external_tools);
    let row_h = widget_row_height(ui);
    egui::ScrollArea::vertical().auto_shrink([false, false]).show_rows(ui, row_h, d.files.len(), |ui, range| {
        for f in &d.files[range] {
            let patch_blocked = (d.parents.len() > 1).then_some(crate::state::patch::PATCH_BLOCKED_MERGE);
            let actions =
                Some(RowActions { src: RowSource::Commit(d.oid), in_workdir: d.in_workdir(&f.path), lfs, diff_blocked, patch_blocked });
            changed_file_row(ui, f, DiffTarget::Commit(d.oid, f.path.clone()), open, d.oid, actions, msgs);
        }
    });
}

/// The file actions of a commit's (or comparison's) file row: where its change comes from,
/// whether the path is in the working tree, what Track in Git LFS needs, and why Open in
/// external diff tool is disabled.
#[derive(Clone, Copy)]
pub(super) struct RowActions<'a> {
    pub src: RowSource,
    pub in_workdir: bool,
    pub lfs: TrackContext<'a>,
    pub diff_blocked: Option<&'a str>,
    /// Why Create patch from this file… is unavailable (a merge commit).
    pub patch_blocked: Option<&'a str>,
}

/// A file changed by a commit (or between two): its status and path, which opens `target`.
/// Its menu opens the file's history as seen from `history_at`. With `actions` it also has the
/// file actions.
pub(super) fn changed_file_row(
    ui: &mut egui::Ui,
    f: &FileEntry,
    target: DiffTarget,
    open: Option<&DiffTarget>,
    history_at: Oid,
    actions: Option<RowActions<'_>>,
    msgs: &mut Vec<Msg>,
) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(status_label(f.status).to_string()).monospace().strong().color(status_color(f.status)));
        let r = ui.selectable_label(open == Some(&target), &f.path);
        super::path_warnings::mark(ui, f);
        if r.clicked() {
            msgs.push(Msg::Repo(RepoMsg::OpenDiff(target)));
        }
        r.context_menu(|ui| {
            // Blame needs the file at `history_at`: a commit compared with the working tree
            // lacks the files added since; another commit lacks the ones it deleted.
            let missing = match actions.map(|a| a.src) {
                Some(RowSource::ToWorkdir(o)) => matches!(f.status, 'A' | 'R').then(|| format!("{} doesn't have this file", short(o))),
                _ => (f.status == 'D').then(|| "This commit deletes the file".to_owned()),
            };
            file_menu::history_items(ui, msgs, &f.path, Some(history_at), missing.as_deref());
            menu_item(msgs, ui, "Copy path", Msg::Copy(f.path.clone()));
            if let Some(a) = actions {
                let items = if a.in_workdir { track_items(&f.path, a.lfs) } else { Vec::new() };
                file_menu::commit_file_items(ui, msgs, a.src, f, a.in_workdir, &items, a.diff_blocked, a.patch_blocked);
            }
        });
    });
}
