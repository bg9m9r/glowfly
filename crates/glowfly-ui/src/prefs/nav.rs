//! Preferences' navigation column: Back, the "Search preferences" field, then the pages, and
//! the repository's pages under one "This repository · name" line. While a query is typed only
//! the pages it matches show, each with its count of matching rows; Enter opens the first match
//! and reveals its row (reveal.rs), and Esc clears the query. Ctrl+F focuses the field.

use super::caption;
use super::index::{self, PageMatch};
use crate::theme::sizes::ControlSize;
use crate::theme::{color, controls, type_scale};
use crate::widgets::{self, Icon};
use eframe::egui::{self, Id, Key, RichText, Ui};
use glowfly_core::i18n::Lang;
use glowfly_model::msg::{Msg, PrefsMsg};
use glowfly_model::state::PrefPage;

/// The column's width, and the field's inset from its sides.
const NAV_W: f32 = 250.0;
const INSET: f32 = 12.0;

/// The pages for every repository, then those of the open one, with their icons.
const PAGES: &[(PrefPage, Icon)] = &[
    (PrefPage::General, Icon::Gear),
    (PrefPage::Git, Icon::Branch),
    (PrefPage::Profile, Icon::User),
    (PrefPage::Integrations, Icon::Plug),
    (PrefPage::Ui, Icon::Brush),
    (PrefPage::Accessibility, Icon::Access),
    (PrefPage::Language, Icon::Globe),
    (PrefPage::DiffViewer, Icon::Code),
    (PrefPage::Keyboard, Icon::Keyboard),
    (PrefPage::Tools, Icon::Tools),
    (PrefPage::Terminal, Icon::Terminal),
    (PrefPage::Signing, Icon::Key),
    (PrefPage::Ssh, Icon::Lock),
    (PrefPage::Trust, Icon::Shield),
];
const REPO_PAGES: &[(PrefPage, Icon)] = &[
    (PrefPage::RepoIdentity, Icon::Identity),
    (PrefPage::RepoHooks, Icon::Hook),
    (PrefPage::RepoTracker, Icon::Activity),
    (PrefPage::RepoLfs, Icon::Storage),
    (PrefPage::RepoGitFlow, Icon::Flow),
    (PrefPage::RepoSparse, Icon::Sparse),
];

/// What the column shows besides the query it edits.
pub struct NavView<'a> {
    pub page: PrefPage,
    pub repo_name: Option<&'a str>,
    pub t: Lang,
}

pub fn show(ctx: &egui::Context, v: &NavView, query: &mut String, focus: &mut bool, msgs: &mut Vec<Msg>) {
    let t = v.t;
    egui::SidePanel::left("prefs-nav").exact_width(NAV_W).resizable(false).frame(egui::Frame::new().fill(color::panel())).show(ctx, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.add_space(6.0);
        let back = if v.repo_name.is_some() { t.tr("prefs.nav.back_to_repo") } else { t.tr("prefs.nav.back") };
        let r = widgets::nav_row(ui, Some(Icon::Back), back, false);
        if r.on_hover_text("Esc").clicked() {
            msgs.push(Msg::Prefs(PrefsMsg::ClosePrefs));
        }
        ui.add_space(10.0);
        let pages: Vec<PrefPage> = PAGES.iter().chain(if v.repo_name.is_some() { REPO_PAGES } else { &[] }).map(|(p, _)| *p).collect();
        let found = index::search(t, query, &pages);
        search_field(ui, v, query, focus, found.first(), msgs);
        ui.add_space(12.0);
        caption(ui, t.tr("prefs.nav.caption"));
        let searching = !query.trim().is_empty();
        if searching && found.is_empty() {
            no_match(ui, t, query.trim());
            return;
        }
        let shown = |p: PrefPage| !searching || found.iter().any(|m| m.page == p);
        let count = |p: PrefPage| found.iter().find(|m| m.page == p).map(|m| m.rows).filter(|&n| n > 0);
        // Every row goes through update, which asks first when the page has unsaved edits.
        let mut go = |ui: &mut Ui, pg: PrefPage, icon: Icon| {
            if shown(pg) && widgets::nav_row_count(ui, Some(icon), pg.nav_label(t), v.page == pg, count(pg)).clicked() {
                msgs.push(Msg::Prefs(PrefsMsg::Navigate(pg)));
            }
        };
        for &(pg, icon) in PAGES {
            go(ui, pg, icon);
        }
        let Some(name) = v.repo_name else { return };
        if REPO_PAGES.iter().any(|(p, _)| shown(*p)) {
            ui.add_space(20.0);
            repo_line(ui, t, name);
            for &(pg, icon) in REPO_PAGES {
                go(ui, pg, icon);
            }
        }
    });
}

/// The field; Enter opens `first` and reveals its first matching row.
fn search_field(ui: &mut Ui, v: &NavView, query: &mut String, focus: &mut bool, first: Option<&PageMatch>, msgs: &mut Vec<Msg>) {
    let (t, current) = (v.t, v.page);
    ui.horizontal(|ui| {
        ui.add_space(INSET);
        let edit = egui::TextEdit::singleline(query).id(Id::new("prefs-search")).hint_text(t.tr("prefs.search.hint"));
        let r = ui.add(controls::SizedField::new(ControlSize::Regular, edit).width(NAV_W - 2.0 * INSET));
        crate::access::name_if_unnamed(&r, t.tr("prefs.search.hint"));
        if std::mem::take(focus) {
            r.request_focus();
        }
        let (enter, esc) = ui.input(|i| (i.key_pressed(Key::Enter), i.key_pressed(Key::Escape)));
        if r.lost_focus() && esc && !query.is_empty() {
            // Esc clears the query first; on an empty field it leaves it, as in any field.
            query.clear();
            r.request_focus();
        }
        if r.lost_focus()
            && enter
            && let Some(m) = first
        {
            msgs.push(Msg::Prefs(PrefsMsg::Navigate(m.page)));
            if let Some(label) = m.first {
                super::reveal::set(ui.ctx(), t.tr(label), m.page, current);
            }
        }
    });
}

/// "Nothing matches “query”."
fn no_match(ui: &mut Ui, t: Lang, query: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.add_space(INSET);
        ui.set_max_width(NAV_W - 2.0 * INSET);
        let text = t.trf("prefs.search.none", &[("query", &query)]);
        ui.add(egui::Label::new(RichText::new(text).size(type_scale::SMALL).color(color::weak())).wrap());
    });
}

/// "This repository · demo": one 12 px semibold weak line, cut with "…" (the whole name on
/// hover) when the name is long.
fn repo_line(ui: &mut Ui, t: Lang, name: &str) {
    ui.horizontal(|ui| {
        ui.add_space(INSET);
        let text = t.trf("prefs.nav.this_repo", &[("name", &name)]);
        let rich = RichText::new(text).font(type_scale::strong(ui, type_scale::SMALL)).color(color::weak());
        widgets::truncate_label(ui, rich, NAV_W - 2.0 * INSET);
    });
    ui.add_space(4.0);
}
