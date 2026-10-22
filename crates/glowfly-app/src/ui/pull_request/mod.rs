//! The pull request panel, in place of the graph: a header with the title and links, then three
//! tabs. Overview has the details, the checks, the review and merge forms on the left (`info`,
//! `checks`, `review`, `merge`) and the conversation in the middle (`conversation`); Commits has
//! the commits on the left (`commits`) and the diff of a file of the one selected in the commit
//! panel in the middle; Files changed has the changed files on the left (`files`) and the open
//! file's diff in the middle, with its code comment threads (`threads`) in a pane below it.

mod checks;
mod commits;
mod conversation;
mod files;
mod info;
mod merge;
mod review;
mod threads;

use super::UiCache;
use super::theme::color;
use super::widgets::{self, Icon};
use crate::keymap::Keymap;
use crate::msg::Msg;
use crate::pull_request::PullRequestMsg;
use crate::state::AppState;
use crate::state::pull_request::{Files, PrTab, PrView, Tone};
use crate::state::pull_request_commits::tab_label;
use crate::state::pull_request_threads::on_file;
use crate::ui::theme::type_scale;
use crate::ui::widgets::buttons;
use eframe::egui::{self, Align, Color32, Layout, Margin, RichText, Sense, Ui, vec2};
use glowfly_core::i18n::Lang;
use glowfly_core::repo::DiffTarget;
use glowfly_core::text::printable;

pub fn show(ui: &mut Ui, st: &mut AppState, cache: &mut UiCache, msgs: &mut Vec<Msg>) {
    let Some(view) = &st.pull_request else { return };
    // Also while the file being opened waits for contents a partial clone hasn't downloaded (in
    // the tab it opened in).
    let waiting = st.for_diff().is_some();
    let diff = st.diff.as_ref().map(|(t, _)| t);
    let shows_file = waiting || diff.is_some_and(|t| view.shows_file(t));
    let shows_commit = waiting || diff.is_some_and(|t| matches!(t, DiffTarget::Commit(..)) && view.shows(t));
    let tab = view.tab;
    ui.add_space(6.0);
    header(ui, st.settings.lang(), &st.keymap, view, msgs);
    tabs(ui, view, msgs);
    ui.separator();
    let room = ui.available_width();
    let side = crate::ui::panel_size::inner_side(egui::SidePanel::left("pull-request-side"), crate::state::panel_layout::PR_SIDE, room)
        .frame(egui::Frame::new().fill(color::panel()).inner_margin(Margin::same(super::theme::spacing::PANEL_PAD)));
    let center = egui::CentralPanel::default().frame(egui::Frame::new().fill(color::bg()));
    match tab {
        PrTab::Overview => {
            side.show_inside(ui, |ui| {
                egui::ScrollArea::vertical().id_salt("pr-side").auto_shrink([false, false]).show(ui, |ui| {
                    let default_branch = st.forge.as_ref().and_then(|f| f.default_branch.clone());
                    let (lang, now) = (st.settings.lang(), st.view.now);
                    let Some(view) = st.pull_request.as_mut() else { return };
                    info::show(ui, view);
                    checks::show(ui, view, lang, now, msgs);
                    review::show(ui, &st.keymap, view, msgs);
                    merge::show(ui, st.settings.lang(), view, st.snap.as_ref(), default_branch.as_deref(), msgs);
                });
            });
            center.show_inside(ui, |ui| conversation::show(ui, st, &mut cache.markdown, msgs));
        }
        PrTab::Commits => {
            side.show_inside(ui, |ui| {
                let Some(view) = &st.pull_request else { return };
                commits::show(ui, st, view, msgs);
            });
            center.show_inside(ui, |ui| {
                if shows_commit {
                    super::diff::show(ui, st, cache, msgs);
                } else {
                    weak_hint(ui, commits::HINT);
                }
            });
        }
        PrTab::Files => {
            side.show_inside(ui, |ui| {
                let Some(view) = &st.pull_request else { return };
                files::show(ui, st, view, msgs);
            });
            center.show_inside(ui, |ui| {
                if shows_file {
                    let path = st.diff.as_ref().map(|(t, _)| t.path().to_owned()).unwrap_or_default();
                    let commented = st.pull_request.as_ref().and_then(|v| v.loaded()).is_some_and(|p| on_file(p, &path) > 0);
                    if commented {
                        let room = ui.available_height();
                        let threads = egui::TopBottomPanel::bottom("pr-file-threads");
                        crate::ui::panel_size::inner_bottom(threads, crate::state::panel_layout::PR_THREADS, room)
                            .frame(egui::Frame::new().fill(color::bg()).inner_margin(Margin::symmetric(12, 4)))
                            .show_inside(ui, |ui| threads::file_pane(ui, st, &path, &mut cache.markdown, msgs));
                    }
                    super::diff::show(ui, st, cache, msgs);
                } else {
                    weak_hint(ui, "Select a file to see its changes.");
                }
            });
        }
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
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        let files = match &v.files {
            Files::Loaded(c) => format!("Files changed  {}", c.comparison.files.len()),
            _ => "Files changed".to_owned(),
        };
        let tabs = [(PrTab::Overview, "Overview".to_owned()), (PrTab::Commits, tab_label(v)), (PrTab::Files, files)];
        for (tab, label) in tabs {
            if ui.selectable_label(v.tab == tab, label).clicked() && v.tab != tab {
                msgs.push(Msg::PullRequest(PullRequestMsg::Tab(tab)));
            }
        }
        if v.reloading {
            ui.add_space(8.0);
            ui.label(RichText::new("Updating…").color(color::weak()));
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
pub(in crate::ui) fn note(ui: &mut Ui, text: impl Into<String>, color: Color32) {
    ui.add(egui::Label::new(RichText::new(text.into()).color(color)).wrap());
}

/// A section heading in the side panel.
pub(in crate::ui) fn heading(ui: &mut Ui, text: &str) {
    ui.add_space(10.0);
    ui.separator();
    ui.label(type_scale::section_text(ui, text));
}

/// The icon, label, title and Draft chip, the links and Close; below, the author and branches.
fn header(ui: &mut Ui, t: Lang, keys: &Keymap, v: &PrView, msgs: &mut Vec<Msg>) {
    let pr = &v.pr;
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(8.0);
            if widgets::ghost::close_view(ui, keys, t, t.tr("common.close_panel.name")).clicked() {
                msgs.push(Msg::PullRequest(PullRequestMsg::Close));
            }
            let no_link = format!("The service sent no link for this {}.", v.noun);
            let has_link = !pr.url.is_empty();
            let why = (!has_link).then_some(no_link.as_str());
            if ui.add(buttons::secondary("Copy link").enabled(has_link).why(why)).clicked() {
                msgs.push(Msg::Copy(pr.url.clone()));
            }
            if ui.add(buttons::secondary("Open in browser").enabled(has_link).why(why)).clicked() {
                msgs.push(Msg::OpenUrl(pr.url.clone()));
            }
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                ui.add_space(8.0);
                let (rect, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                widgets::paint_icon(ui.painter(), rect.center(), Icon::PullRequest, color::progress());
                ui.label(RichText::new(&v.label).color(color::weak()));
                const DRAFT: &str = "Draft";
                let chip_w = if pr.draft {
                    ui.painter().layout_no_wrap(DRAFT.to_owned(), type_scale::font(type_scale::CAPTION), color::weak()).size().x + 12.0
                } else {
                    0.0
                };
                ui.scope(|ui| {
                    ui.set_max_width((ui.available_width() - chip_w - 8.0).max(40.0));
                    let title = RichText::new(printable(&pr.title)).font(type_scale::strong(ui, type_scale::SUBHEAD)).color(color::text());
                    ui.add(egui::Label::new(title).truncate());
                });
                if pr.draft {
                    let (rect, _) = ui.allocate_exact_size(vec2(chip_w, 20.0), Sense::hover());
                    widgets::chip(ui.painter(), rect.left(), rect.center().y, DRAFT, color::raised(), color::weak());
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
