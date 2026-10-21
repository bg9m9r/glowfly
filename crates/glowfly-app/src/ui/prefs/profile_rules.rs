//! Preferences > Profile's Folders row (the selected profile's folder rules with Remove, Add
//! folder… and Use for this repository) and Open repository row (where the open repository's
//! identity comes from, with Use <profile> here when its own identity comes before a rule).
//! Everything is a `Msg`; the texts are in state/profile_rules.rs and state/repo_identity.rs.

use super::{help, row};
use crate::msg::Msg;
use crate::profile::ProfileMsg;
use crate::state::profile_rules::{
    ADD_FOLDER_BUTTON, ALREADY_HERE, FOLDERS_LABEL, HERE_LABEL, NO_RULES, REMOVE_BUTTON, RULES_HELP, USE_REPO_BUTTON, add_blocked,
    has_repository, rule_line,
};
use crate::state::profiles::ProfilesPage;
use crate::state::repo_identity::{here_line, not_applied, not_applied_line, shadow_line, shadowed, use_here_label};
use crate::ui::theme::color;
use crate::ui::theme::tokens::radius;
use crate::ui::theme::type_scale;
use crate::ui::widgets::buttons;
use crate::ui::widgets::form_message::{self, Message};
use crate::ui::widgets::{self, Icon};
use eframe::egui::{self, Align, Align2, CornerRadius, Layout, RichText, Sense, Ui, UiBuilder, pos2, vec2};
use glowfly_core::repo_identity::RepoIdentity;
use glowfly_core::settings::{FolderRule, Profile, Settings};
use std::path::Path;

const ROW_H: f32 = 40.0;
/// The widest a disabled button's reason gets beside it.
const REASON_W: f32 = 280.0;

/// The open repository, for the Profile page.
pub struct RepoHere<'a> {
    pub name: &'a str,
    pub git_dir: &'a Path,
    /// Its common git dir (what Use for this repository adds).
    pub common: &'a Path,
    /// Its identity as git last reported it; None while the first read runs.
    pub identity: Option<&'a RepoIdentity>,
    /// Whether its rule already got its one repair this session.
    pub healed: bool,
}

/// The Open repository row, above the profile list.
pub fn here_row(ui: &mut Ui, settings: &Settings, here: &RepoHere<'_>, width: f32, msgs: &mut Vec<Msg>) {
    let Some(id) = here.identity else { return };
    row(ui, HERE_LABEL, |ui| {
        ui.set_max_width(width);
        let line = here_line(here.name, id, settings, here.git_dir);
        ui.add(egui::Label::new(RichText::new(line).font(type_scale::font(type_scale::BODY)).color(color::text())).wrap());
        if let Some(p) = shadowed(id, settings).and_then(|p| settings.profile(p)) {
            warning(ui, &shadow_line(&p.name));
            ui.add_space(2.0);
            if ui.add(buttons::secondary(use_here_label(&p.name))).clicked() {
                msgs.push(Msg::Profile(ProfileMsg::AskUseHere(p.id)));
            }
        }
        if not_applied(id, settings, here.healed)
            && let Some(p) = id.expected.and_then(|p| settings.profile(p))
        {
            warning(ui, &not_applied_line(&p.name));
        }
    });
}

/// A warning mark and a weak line beside it.
fn warning(ui: &mut Ui, text: &str) {
    ui.horizontal_top(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(16.0, 18.0), Sense::hover());
        widgets::paint_icon_sized(ui.painter(), r.center(), Icon::Warning, color::warn(), 16.0);
        ui.add(egui::Label::new(RichText::new(text).size(type_scale::SMALL).color(color::weak())).wrap());
    });
}

/// The Folders row of the selected profile `p`, after its Signing key row.
pub fn rules_rows(ui: &mut Ui, p: &Profile, page: &ProfilesPage, here: Option<&RepoHere<'_>>, width: f32, msgs: &mut Vec<Msg>) {
    row(ui, FOLDERS_LABEL, |ui| {
        ui.set_max_width(width);
        ui.spacing_mut().item_spacing.y = 4.0;
        if p.folders.is_empty() {
            ui.add(egui::Label::new(RichText::new(NO_RULES).size(type_scale::SMALL).color(color::faint())).wrap());
        }
        for rule in &p.folders {
            rule_row(ui, p, rule, msgs);
        }
        if let Some(e) = &page.rules_error {
            form_message::line(ui, Message::error(e.as_str()));
        }
        ui.add_space(4.0);
        let blocked = add_blocked(p);
        ui.horizontal(|ui| {
            if ui.add(buttons::secondary(ADD_FOLDER_BUTTON).enabled(blocked.is_none()).why(blocked)).clicked() {
                msgs.push(Msg::Profile(ProfileMsg::AddFolder(p.id)));
            }
            if let Some(here) = here {
                let why = blocked.or(has_repository(p, here.common).then_some(ALREADY_HERE));
                if ui.add(buttons::secondary(USE_REPO_BUTTON).enabled(why.is_none()).why(why)).clicked() {
                    msgs.push(Msg::Profile(ProfileMsg::AddThisRepo(p.id)));
                }
            }
            if let Some(why) = blocked.or(here.filter(|h| has_repository(p, h.common)).map(|_| ALREADY_HERE)) {
                widgets::reason::beside(ui, why, REASON_W);
            }
        });
        ui.add_space(2.0);
        help(ui, RULES_HELP);
    });
}

/// One rule: the folder mark, its path over its kind, and Remove.
fn rule_row(ui: &mut Ui, p: &Profile, rule: &FolderRule, msgs: &mut Vec<Msg>) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, CornerRadius::same(radius::MD), color::panel());
    widgets::paint_icon_sized(&painter, pos2(rect.left() + 18.0, rect.center().y), Icon::Folder, color::weak(), 16.0);
    let (path, kind) = rule_line(rule);
    let text_x = rect.left() + 36.0;
    // Room up to the Remove button.
    let text_w = (rect.width() - 36.0 - 96.0).max(0.0);
    let cut = widgets::paint_cut_text(
        &painter,
        pos2(text_x, rect.top() + 13.0),
        &path,
        type_scale::font(type_scale::BODY),
        color::text(),
        text_w,
    );
    painter.text(pos2(text_x, rect.top() + 29.0), Align2::LEFT_CENTER, kind, type_scale::font(type_scale::SMALL), color::weak());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &path));
    if cut.hovered(ui.ctx().pointer_hover_pos()) {
        resp.on_hover_text(&path);
    }
    // A child that doesn't allocate: the row above already holds the space.
    let mut child = ui.new_child(UiBuilder::new().max_rect(rect.shrink2(vec2(8.0, 0.0))).layout(Layout::right_to_left(Align::Center)));
    if child.add(buttons::secondary(REMOVE_BUTTON)).clicked() {
        msgs.push(Msg::Profile(ProfileMsg::RemoveFolder { id: p.id, path: rule.path.clone() }));
    }
}
