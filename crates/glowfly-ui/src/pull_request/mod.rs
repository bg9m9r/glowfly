//! The pull request panel, in place of the graph: a header with the title and links, then three
//! tabs. Overview has the details, the checks, the review and merge forms on the left (`info`,
//! `checks`, `review`, `merge`) and the conversation in the middle (`conversation`); Commits has
//! the commits on the left (`commits`) and the diff of a file of the one selected in the commit
//! panel in the middle; Files changed has the changed files on the left (`files`) and the open
//! file's diff in the middle, with its code comment threads (`threads`) in a pane below it,
//! led by the box of a new code comment on its lines (`line_comment`). Also here: the Create pull
//! request dialog (`create_dialog`, its From / Into repositories in `create_repos`) and the pull
//! request menu items of the branch menus and the menu bar (`menu_items`).

mod checks;
mod commits;
mod conversation;
pub(crate) mod create_dialog;
mod create_repos;
mod files;
mod info;
mod line_comment;
pub(crate) mod menu_items;
mod merge;
mod review;
mod threads;
// For the chip catalog golden (`snapshot_tests_chips.rs`).
#[cfg(test)]
pub(crate) use threads::chip as thread_chip;

use super::UiCache;
use super::theme::color;
use super::widgets::{self, Icon, underline_tab};
use crate::theme::type_scale;
use crate::widgets::chip::{Chip, ChipSize, Look};
use eframe::egui::{self, Align, Color32, Layout, Margin, RichText, Sense, Ui, vec2};
use glowfly_core::i18n::Lang;
use glowfly_core::repo::DiffTarget;
use glowfly_core::text::printable;
use glowfly_model::keymap::Keymap;
use glowfly_model::msg::{EditMsg, Msg};
use glowfly_model::pull_request::PullRequestMsg;
use glowfly_model::state::pull_request::commits::tab_label;
use glowfly_model::state::pull_request::line_map::on_local_file;
use glowfly_model::state::pull_request::{Files, PrTab, PrView, Tone};
use glowfly_model::state::{AppState, ViewState};

/// Draws the panel. What its forms and the new code comment's box write is drawn from one copy
/// (`PrView::fields`) and sent back as `EditMsg::PullRequest` when it changed.
pub fn show(ui: &mut Ui, st: &AppState, vs: &mut ViewState, cache: &mut UiCache, msgs: &mut Vec<Msg>) {
    let Some(view) = &st.session.pull_request.view else { return };
    let mut f = view.fields();
    // Also while the file being opened waits for contents a partial clone hasn't downloaded (in
    // the tab it opened in).
    let waiting = st.for_diff().is_some();
    let diff = st.session.diff.view.as_ref().map(|(t, _)| t);
    let shows_file = waiting || diff.is_some_and(|t| view.shows_file(t));
    let shows_commit = waiting || diff.is_some_and(|t| matches!(t, DiffTarget::Commit(..)) && view.shows(t));
    let tab = view.tab;
    ui.add_space(6.0);
    header(ui, st.settings.lang(), &st.keymap, view, msgs);
    // The tabs draw the line under them.
    tabs(ui, view, msgs);
    let room = ui.available_width();
    let side = crate::panel_size::inner_side(egui::SidePanel::left("pull-request-side"), glowfly_model::state::panel_layout::PR_SIDE, room)
        .frame(egui::Frame::new().fill(color::panel()).inner_margin(Margin::same(super::theme::spacing::PANEL_PAD)));
    let center = egui::CentralPanel::default().frame(egui::Frame::new().fill(color::bg()));
    match tab {
        PrTab::Overview => {
            side.show_inside(ui, |ui| {
                egui::ScrollArea::vertical().id_salt("pr-side").auto_shrink([false, false]).show(ui, |ui| {
                    let default_branch = st.session.forge.as_ref().and_then(|f| f.default_branch.clone());
                    let (lang, now) = (st.settings.lang(), st.cues.now);
                    info::show(ui, view);
                    checks::show(ui, view, lang, now, msgs);
                    review::show(ui, &st.keymap, view, &mut f.forms, msgs);
                    merge::show(ui, lang, view, &mut f.forms, st.snap.as_ref(), default_branch.as_deref(), msgs);
                });
            });
            center.show_inside(ui, |ui| conversation::show(ui, st, vs, &mut cache.markdown, &mut f.forms, msgs));
        }
        PrTab::Commits => {
            side.show_inside(ui, |ui| commits::show(ui, st, view, msgs));
            center.show_inside(ui, |ui| {
                if shows_commit {
                    super::diff::show(ui, st, vs, cache, msgs);
                } else {
                    weak_hint(ui, commits::HINT);
                }
            });
        }
        PrTab::Files => {
            side.show_inside(ui, |ui| files::show(ui, st, view, msgs));
            center.show_inside(ui, |ui| {
                if shows_file {
                    let path = st.session.diff.view.as_ref().map(|(t, _)| t.path().to_owned()).unwrap_or_default();
                    let commented =
                        on_local_file(view, &path) > 0 || glowfly_model::state::pull_request::line_comment::draft_on(view, &path);
                    if commented {
                        let room = ui.available_height();
                        let threads = egui::TopBottomPanel::bottom("pr-file-threads");
                        crate::panel_size::inner_bottom(threads, glowfly_model::state::panel_layout::PR_THREADS, room)
                            .frame(egui::Frame::new().fill(color::bg()).inner_margin(Margin::symmetric(12, 4)))
                            .show_inside(ui, |ui| threads::file_pane(ui, st, vs, &path, &mut cache.markdown, &mut f, msgs));
                    }
                    super::diff::show(ui, st, vs, cache, msgs);
                } else {
                    weak_hint(ui, "Select a file to see its changes.");
                }
            });
        }
    }
    if f != view.fields() {
        msgs.push(Msg::Edit(EditMsg::PullRequest(Box::new(f))));
    }
}

/// A tab center's hint while nothing is open in it, wrapped to the center's width.
fn weak_hint(ui: &mut Ui, text: &str) {
    egui::Frame::new().inner_margin(Margin { left: 12, right: 12, top: 12, bottom: 0 }).show(ui, |ui| {
        ui.add(egui::Label::new(RichText::new(text).color(color::weak())).wrap());
    });
}

/// Overview, Commits and Files changed (with their counts once known), and a note while the
/// page reloads.
fn tabs(ui: &mut Ui, v: &PrView, msgs: &mut Vec<Msg>) {
    ui.add_space(2.0);
    let files = match &v.files {
        Files::Loaded(c) => format!("Files changed  {}", c.comparison.files.len()),
        _ => "Files changed".to_owned(),
    };
    let tabs = [(PrTab::Overview, "Overview".to_owned()), (PrTab::Commits, tab_label(v)), (PrTab::Files, files)];
    let reloading = |ui: &mut Ui| {
        if v.reloading {
            ui.add_space(8.0);
            ui.label(RichText::new("Updating…").color(color::weak()));
        }
    };
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        if let Some(tab) = underline_tab::row(ui, v.tab, &tabs, reloading) {
            msgs.push(Msg::PullRequest(PullRequestMsg::Tab(tab)));
        }
    });
}

/// Whether Command+Enter (Ctrl on Windows and Linux) was pressed in the focused box `field`:
/// it posts the box's text (`BindingId::SendPrText`), and is consumed so nothing else sees it.
pub(super) fn send_key(ui: &mut Ui, field: &egui::Response) -> bool {
    field.has_focus() && ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter))
}

/// The color of a tone.
pub(super) fn tone_color(t: Tone) -> Color32 {
    match t {
        Tone::Good => color::added(),
        Tone::Bad => color::removed(),
        Tone::Warn => color::warn(),
        Tone::Neutral => color::weak(),
    }
}

/// A wrapped label in `color`.
pub(crate) fn note(ui: &mut Ui, text: impl Into<String>, color: Color32) {
    ui.add(egui::Label::new(RichText::new(text.into()).color(color)).wrap());
}

/// A section heading in the side panel.
pub(crate) fn heading(ui: &mut Ui, text: &str) {
    ui.add_space(10.0);
    ui.separator();
    ui.label(type_scale::section_text(ui, text));
}

/// The icon, label, title and Draft chip, the links (in a ⋯ menu when they would squeeze the
/// title, `panel_header`) and Close; below, the author and branches.
fn header(ui: &mut Ui, t: Lang, keys: &Keymap, v: &PrView, msgs: &mut Vec<Msg>) {
    let pr = &v.pr;
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(8.0);
            if widgets::ghost::close_view(ui, keys, t, t.tr("common.close_panel.name")).clicked() {
                msgs.push(Msg::PullRequest(PullRequestMsg::Close));
            }
            const DRAFT: &str = "Draft";
            let draft = Chip::new(ChipSize::Badge, DRAFT).look(Look::new(color::raised(), color::weak()));
            let chip_w = if pr.draft { draft.width(ui.ctx()) } else { 0.0 };
            let no_link = format!("The service sent no link for this {}.", v.noun);
            let lead_w = crate::panel_header::text_w(ui, &v.label) + chip_w;
            let open_label = t.tr("panel.header.open_in_browser");
            let links = crate::panel_header::Links { url: &pr.url, open_label, no_link: &no_link, lead_w };
            crate::panel_header::show(ui, t, keys, links, msgs);
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                ui.add_space(8.0);
                let (rect, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                widgets::paint_icon(ui.painter(), rect.center(), Icon::PullRequest, color::progress());
                ui.label(RichText::new(&v.label).color(color::weak()));
                ui.scope(|ui| {
                    ui.set_max_width((ui.available_width() - chip_w - 8.0).max(40.0));
                    let title = RichText::new(printable(&pr.title)).font(type_scale::strong(ui, type_scale::SUBHEAD)).color(color::text());
                    ui.add(egui::Label::new(title).truncate());
                });
                if pr.draft {
                    draft.show(ui);
                }
            });
        });
    });
    ui.horizontal(|ui| {
        ui.add_space(32.0);
        let weak = |t: String| RichText::new(t).color(color::weak());
        ui.label(weak(format!("by {} · {}", printable(&pr.author), printable(&pr.source))));
        // Painted: the bundled fonts have no arrow glyph.
        let (rect, _) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
        widgets::paint_icon(ui.painter(), rect.center(), Icon::Into, color::weak());
        ui.add(egui::Label::new(weak(printable(&pr.target))).truncate());
    });
}
