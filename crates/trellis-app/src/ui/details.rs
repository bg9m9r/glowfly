//! Right panel: staging and committing (WIP, `details_wip.rs`), the selected commit's details
//! (`details_commit.rs`), several selected commits (`details_multi.rs`) or a commit compared
//! with the working directory (`details_workdir.rs`); and the changed-file row they share with
//! the pull request's files.

use super::UiCache;
use super::common::{menu_item, short};
use super::file_menu::{self, RowSource};
use super::panel_size;
use super::theme::{color, spacing};
use super::widgets;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::state::lfs::{TrackContext, track_items};
use crate::state::panel_layout::Fit;
use crate::state::{AppState, Selection};
use eframe::egui::{self, RichText};
use git2::Oid;
use trellis_core::i18n::Lang;
use trellis_core::repo::{DiffTarget, FileEntry, LineCount};
use trellis_core::settings::SizedPanel;

// Declared here rather than beside the other views: the file lists that use it are these panels.
#[path = "file_reveal.rs"]
pub(super) mod file_reveal;

/// Draws the panel at `fit`'s width (`panel_size::side_fits`).
pub fn show(ctx: &egui::Context, st: &mut AppState, cache: &mut UiCache, fit: Fit, msgs: &mut Vec<Msg>) {
    let id = egui::Id::new(panel_size::DETAILS_ID);
    panel_size::pin(ctx, id, fit.size, st.view.panel_drag == Some(SizedPanel::Details));
    let resp = panel_size::side(egui::SidePanel::right(id), fit)
        .frame(egui::Frame::new().fill(color::panel()).inner_margin(egui::Margin::same(super::theme::spacing::PANEL_PAD)))
        .show(ctx, |ui| {
            panel_size::fill(ui);
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
            match st.selection {
                Selection::Wip => super::details_wip::wip_panel(ui, st, cache, msgs),
                Selection::Commit(_) => super::details_commit::panel(ui, st, cache, msgs),
                Selection::Commits(_) => super::details_multi::panel(ui, st, msgs),
                Selection::VsWorkdir(_) => super::details_workdir::panel(ui, st, msgs),
                Selection::None => {
                    let t = st.settings.lang();
                    ui.label(RichText::new(t.tr("details.none")).color(color::weak()));
                    if st.snap.as_ref().is_some_and(|s| s.is_dirty()) {
                        ui.add_space(8.0);
                        if ui.button(t.tr("details.view_wip.button")).clicked() {
                            msgs.push(Msg::Repo(RepoMsg::SelectWip));
                        }
                    }
                }
            }
        });
    panel_size::track(ctx, st, SizedPanel::Details, id, resp.response.rect.width(), msgs);
}

/// What a file row draws and offers: where its change comes from, the commit its history and
/// blame are seen from, whether the path is in the working tree, what Track in Git LFS needs,
/// which external programs aren't set yet (their entries say "Set up…"), and its line counts.
#[derive(Clone, Copy)]
pub(super) struct RowActions<'a> {
    pub src: RowSource,
    pub history_at: Oid,
    pub in_workdir: bool,
    pub lfs: TrackContext<'a>,
    pub tools: crate::state::tool_compare::ToolSetup,
    /// Why Create patch from this file… is unavailable (a merge commit).
    pub patch_blocked: Option<&'a str>,
    /// The file's lines added and removed, and the largest change in its list (the bar's
    /// scale); None draws no counts.
    pub lines: Option<(LineCount, usize)>,
}

/// A file changed by a commit (or between two), which opens `target`: its status chip, its path
/// (cut first), its `+N −M` right after it, its marks, and the line-count bar at the right end.
/// Its menu opens the file's history as seen from `actions.history_at`, and has the file
/// actions.
pub(super) fn changed_file_row(
    ui: &mut egui::Ui,
    t: Lang,
    f: &FileEntry,
    target: DiffTarget,
    open: Option<&DiffTarget>,
    actions: RowActions<'_>,
    msgs: &mut Vec<Msg>,
) {
    let selected = open == Some(&target);
    let lines = actions.lines;
    let row = |ui: &mut egui::Ui| {
        super::file_status::status_chip(ui, f.status);
        // The counts keep their width; the path is cut before them.
        let reserve = lines.map_or(0.0, |(n, _)| super::line_counts::inline_width(ui.ctx(), n) + spacing::GRID);
        widgets::row_text_reserving(ui, RichText::new(&f.path).monospace(), selected, reserve);
        if let Some((n, _)) = lines {
            super::line_counts::inline(ui, n);
        }
        super::path_warnings::mark(ui, t, f);
        super::lfs_badge::badge(ui, f);
    };
    let r = match lines {
        Some((n, largest)) => widgets::file_row_with_end(ui, selected, &f.path, |ui| super::line_counts::bar_end(ui, n, largest), row),
        None => widgets::file_row(ui, selected, &f.path, row),
    }
    .response;
    if r.clicked() {
        msgs.push(Msg::Repo(RepoMsg::OpenDiff(target)));
    }
    r.context_menu(|ui| {
        // Blame needs the file at `history_at`: a commit compared with the working tree
        // lacks the files added since; another commit lacks the ones it deleted.
        let a = actions;
        let missing = match a.src {
            RowSource::ToWorkdir(o) => matches!(f.status, 'A' | 'R').then(|| t.trf("details.file.not_in_commit", &[("sha", &short(o))])),
            _ => (f.status == 'D').then(|| t.tr("details.file.deleted_here").to_owned()),
        };
        file_menu::history_items(ui, t, msgs, &f.path, Some(a.history_at), missing.as_deref());
        menu_item(msgs, ui, t.tr("common.copy_path"), Msg::Copy(f.path.clone()));
        let items = if a.in_workdir { track_items(t, &f.path, a.lfs) } else { Vec::new() };
        file_menu::commit_file_items(ui, t, msgs, a.src, f, a.in_workdir, &items, a.tools, a.patch_blocked);
    });
}
