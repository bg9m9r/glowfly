//! The start page's repository area, below its cards and as wide as their row: Favorites, a
//! heading with + New group at its right (and Open all and Remove missing when they apply),
//! then the repository groups nested under it, indented, each with its header and a bar in its
//! colour down its rows (`repo_list_groups.rs`), then the ungrouped favorites (under "Other
//! favorites" once a group exists), then Recent with Remove missing and Clear. Every row is the
//! repository menu's row (`repo_list_row.rs`, `repo_row.rs`), all of them sharing columns, and
//! favorites drag into another order or group by their grip (`repo_list_drag.rs`); an empty
//! group's placeholder offers Add repositories… (`repo_group_picker.rs`). Under
//! Favorites, a line says what groups do while no group has a member, and how to add a favorite
//! while there is none.

use super::repo_list_drag::{self, DropSection};
use super::repo_list_groups::{self, header_row};
use super::repo_list_row::{self, Section};
use super::repo_menu_list;
use super::repo_row::{self, Columns, Row, Words};
use super::theme::color;
use super::welcome_cards::START_W;
use super::widgets::{Icon, ghost};
use crate::theme::sizes::ControlSize;
use crate::theme::type_scale;
use eframe::egui::{self, Align, Layout, RichText};
use glowfly_model::msg::{GroupMsg, Msg, RecentMsg};
use glowfly_model::state::AppState;
use glowfly_model::state::repo_groups::{any_colour, members, ungrouped};
use glowfly_model::state::repo_lists::{clear_recent_msg, favorites, is_missing, is_open, missing_counts, recent_rows, remove_missing_msg};
use std::path::PathBuf;

/// How far the groups and the other favorites sit in from the Favorites heading.
const INDENT: f32 = 16.0;
/// The space above Recent, and above each group.
const SECTION_GAP: f32 = 20.0;
const GROUP_GAP: f32 = 8.0;

/// The start page's repository sections, in a centred column below its cards. Nothing is drawn
/// while there is no favorite, group or recent entry.
pub fn welcome_sections(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let groups = &st.settings.repo_groups;
    let loose = ungrouped(&st.settings);
    let recent: Vec<PathBuf> = recent_rows(st).into_iter().cloned().collect();
    let favs = favorites(&st.settings).len();
    if favs == 0 && recent.is_empty() && groups.is_empty() {
        return;
    }
    ui.add_space(24.0);
    let width = ui.available_width().min(START_W);
    let top = ui.cursor().top();
    let full = ui.max_rect();
    let rect =
        egui::Rect::from_min_max(egui::pos2(full.center().x - width / 2.0, top), egui::pos2(full.center().x + width / 2.0, full.bottom()));
    let nested = !groups.is_empty();
    let draggable = favs >= 2 || (favs > 0 && nested);
    // The rows, made first so every row's columns are measured together.
    let mut key = 0;
    let grouped: Vec<Vec<(Row, Section)>> =
        (0..groups.len()).map(|g| repo_list_row::favorite_rows(st, &members(&st.settings, g), Some(g), &mut key, draggable)).collect();
    let others = repo_list_row::favorite_rows(st, &loose, None, &mut key, draggable);
    let recents = repo_list_row::recent_rows(st, &recent);
    let words = repo_menu_list::words(st);
    let measured: Vec<&Row> = grouped.iter().flatten().chain(&others).chain(&recents).map(|(r, _)| r).collect();
    let cols = Columns::measure(ui, &measured, &words, any_colour(&st.settings), true);
    let list = List { st, cols, words: &words };
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect).layout(Layout::top_down(Align::Min)), |ui| {
        egui::ScrollArea::vertical().max_height(ui.available_height()).auto_shrink([false, true]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            favorites_heading(ui, &list, &loose, msgs);
            let mut sections = Vec::new();
            let indent = egui::Margin { left: if nested { INDENT as i8 } else { 0 }, ..Default::default() };
            egui::Frame::new().inner_margin(indent).show(ui, |ui| {
                for (g, rows) in grouped.iter().enumerate() {
                    ui.add_space(GROUP_GAP);
                    repo_list_groups::header(ui, st, g, cols, msgs);
                    let drawn = list.section(ui, Some(g), rows, msgs);
                    if let Some((top, bottom)) = drawn.span {
                        let hue = groups[g].hue().map(color::repo_hue);
                        repo_list_groups::bar(ui, ui.max_rect().left(), top, bottom, hue);
                    }
                    sections.push(drawn.drop);
                }
                // While every favorite is in a group, the others show only as a drag's drop place.
                if !others.is_empty() || (nested && repo_list_drag::dragging(ui)) {
                    if nested {
                        ui.add_space(GROUP_GAP);
                        let title = st.settings.lang().tr("repos.favorites.others");
                        let (_, label_x) = repo_row::lead(ui, cols);
                        let left = |ui: &mut egui::Ui| {
                            ui.label(RichText::new(title).color(color::weak()));
                        };
                        header_row(ui, label_x, left, |ui| {
                            // An empty ⋯ slot, so its Open all ends where the groups' do.
                            ui.allocate_exact_size(
                                egui::vec2(ControlSize::Small.metrics().h, ControlSize::Small.metrics().h),
                                egui::Sense::hover(),
                            );
                            open_all_link(ui, st, &loose, msgs);
                        });
                    }
                    sections.push(list.section(ui, None, &others, msgs).drop);
                }
            });
            repo_list_drag::track(ui, &sections, msgs);
            if !recents.is_empty() {
                ui.add_space(SECTION_GAP);
                recent_heading(ui, st, cols, msgs);
                for row in &recents {
                    repo_list_row::show(ui, st, row, cols, &words, msgs);
                }
            }
        });
    });
}

/// What every section of the list shares.
struct List<'a> {
    st: &'a AppState,
    cols: Columns,
    words: &'a Words<'a>,
}

/// One drawn favorites section: its drop places, and the top and bottom of its rows.
struct Drawn {
    drop: DropSection,
    span: Option<(f32, f32)>,
}

impl List<'_> {
    /// The rows of one favorites section (`group`; None: the ungrouped ones), or its outlined
    /// placeholder while it has none.
    fn section(&self, ui: &mut egui::Ui, group: Option<usize>, rows: &[(Row, Section)], msgs: &mut Vec<Msg>) -> Drawn {
        let t = self.st.settings.lang();
        let mut drops = Vec::new();
        let mut span: Option<(f32, f32)> = None;
        for row in rows {
            let shown = repo_list_row::show(ui, self.st, row, self.cols, self.words, msgs);
            span = Some((span.map_or(shown.rect.top(), |s| s.0), shown.rect.bottom()));
            if let Some(grip) = shown.grip {
                drops.push((row.0.path.clone(), grip, shown.rect));
            }
        }
        let empty = rows.is_empty().then(|| {
            let text = if group.is_some() { t.tr("repo_group.empty") } else { t.tr("repos.favorites.drop_out") };
            // While a drag is on, the box is only a place to drop.
            let link = group.filter(|_| !repo_list_drag::dragging(ui)).map(|_| t.tr("repo_group.menu.add_repos"));
            let (r, add) = repo_list_groups::placeholder(ui, text, link, self.cols);
            if let (true, Some(g)) = (add, group) {
                msgs.push(Msg::Groups(GroupMsg::AddDialog(g)));
            }
            span = Some((r.top(), r.bottom()));
            r
        });
        Drawn { drop: DropSection { group, rows: drops, empty }, span }
    }
}

/// The Favorites heading, with + New group at its right once there is a favorite or a group,
/// Open all for the favorites while there are no groups, and Remove missing when only favorites
/// are missing; under it, a line while there are no favorites or no group has a member.
fn favorites_heading(ui: &mut egui::Ui, list: &List<'_>, loose: &[PathBuf], msgs: &mut Vec<Msg>) {
    let st = list.st;
    let t = st.settings.lang();
    let groups = &st.settings.repo_groups;
    let nested = !groups.is_empty();
    let none = favorites(&st.settings).is_empty();
    let (missing_recent, missing_favs) = missing_counts(st);
    let (_, label_x) = repo_row::lead(ui, list.cols);
    header_row(
        ui,
        label_x,
        |ui| heading(ui, t.tr("repos.favorites.header")),
        |ui| {
            if nested || !none {
                let label = t.tr("repos.groups.new.link");
                if ghost::ghost(ui, ControlSize::Small, Some(Icon::Plus), Some(label), label, true).clicked() {
                    msgs.push(Msg::Groups(GroupMsg::NewDialog { repo: None }));
                }
            }
            if !nested {
                open_all_link(ui, st, loose, msgs);
            }
            if missing_recent == 0 && missing_favs > 0 {
                remove_missing_link(ui, st, msgs);
            }
        },
    );
    if none {
        note(ui, label_x, t.tr("repos.favorites.empty"));
    } else if nested && groups.iter().all(|g| g.repos.is_empty()) {
        note(ui, label_x, t.tr("repos.groups.intro"));
    }
}

/// The Recent heading, with Clear and, while the last check missed any, Remove missing.
fn recent_heading(ui: &mut egui::Ui, st: &AppState, cols: Columns, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let (_, label_x) = repo_row::lead(ui, cols);
    header_row(
        ui,
        label_x,
        |ui| heading(ui, t.tr("repos.recent.header")),
        |ui| {
            if small_link(ui, t.tr("repos.clear.link")) {
                msgs.push(clear_recent_msg(st));
            }
            if missing_counts(st).0 > 0 {
                remove_missing_link(ui, st, msgs);
            }
        },
    );
}

/// Open all for `paths`, while one of them isn't in a tab.
fn open_all_link(ui: &mut egui::Ui, st: &AppState, paths: &[PathBuf], msgs: &mut Vec<Msg>) {
    let unopened = paths.iter().any(|p| !is_open(st, p) && !is_missing(st, p));
    if unopened && small_link(ui, st.settings.lang().tr("repos.open_all.link")) {
        msgs.push(Msg::Recent(RecentMsg::OpenAll(paths.to_vec())));
    }
}

/// A section's heading: body size, strong.
fn heading(ui: &mut egui::Ui, title: &str) {
    ui.label(RichText::new(title).font(type_scale::strong(ui, type_scale::BODY)).color(color::text()));
}

/// A weak line under the Favorites heading, where the rows' names start.
fn note(ui: &mut egui::Ui, label_x: f32, text: &str) {
    ui.horizontal(|ui| {
        ui.add_space(label_x);
        ui.add(egui::Label::new(RichText::new(text).size(type_scale::SMALL).color(color::weak())).wrap());
    });
    ui.add_space(4.0);
}

pub(super) fn small_link(ui: &mut egui::Ui, text: &str) -> bool {
    ui.add(egui::Link::new(RichText::new(text).size(type_scale::SMALL))).clicked()
}

fn remove_missing_link(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    if small_link(ui, st.settings.lang().tr("repos.remove_missing.link")) {
        msgs.extend(remove_missing_msg(st));
    }
}
