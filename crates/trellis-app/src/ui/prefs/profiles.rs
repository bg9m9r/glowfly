//! Preferences > Profile: the profile list (current marker, switch, inline rename, delete), the
//! New profile form, and the selected profile's identity fields, then what else switches with
//! it (profile_setup.rs). The view edits only text buffers and which form is open; everything
//! else is a `Msg`.

use super::{help, primary_button, row, text_field};
use crate::msg::Msg;
use crate::profile::{IdentityField, ProfileMsg};
use crate::state::prefs::GitConfigValues;
use crate::state::profiles::{
    ADD_BUTTON, AddForm, COPY_ACCOUNTS, COPY_CURRENT, CURRENT_CHIP, CURRENT_HELP, DELETE_BUTTON, DELETE_DISABLED, KEY_HELP, LIST_HELP,
    LIST_LABEL, NAME_LABEL, NEW_LABEL, NEW_NAME_HINT, NO_IDENTITY, OTHER_HELP, ProfilesPage, RENAME_BUTTON, RenameForm, SWITCH_BUTTON,
    editing_caption, identity_line, key_hint,
};
use crate::ui::theme::color;
use crate::ui::theme::controls;
use crate::ui::widgets;
use eframe::egui::{self, Align, Align2, CornerRadius, FontId, Key, Layout, RichText, Sense, Stroke, Ui, UiBuilder, pos2, vec2};
use trellis_core::settings::{Profile, ProfileId, Settings};

/// The list and its rows are wider than a form field, to fit the buttons beside the identity.
const LIST_W: f32 = 640.0;
const ROW_H: f32 = 44.0;

pub fn profile_page(
    ui: &mut Ui,
    settings: &Settings,
    page: &mut ProfilesPage,
    git: &GitConfigValues,
    switching: Option<ProfileId>,
    msgs: &mut Vec<Msg>,
) {
    list(ui, settings, page, switching, msgs);
    add_form(ui, page, msgs);
    fields(ui, settings, page, git, msgs);
}

fn list(ui: &mut Ui, settings: &Settings, page: &mut ProfilesPage, switching: Option<ProfileId>, msgs: &mut Vec<Msg>) {
    row(ui, LIST_LABEL, |ui| {
        ui.set_max_width(LIST_W);
        ui.spacing_mut().item_spacing.y = 4.0;
        for p in &settings.profiles {
            let current = settings.active_profile == Some(p.id);
            profile_row(ui, p, current, page, switching, msgs);
        }
        ui.add_space(2.0);
        help(ui, LIST_HELP);
        if page.adding.is_none() {
            ui.add_space(4.0);
            if ui.button(ADD_BUTTON).clicked() {
                page.adding = Some(AddForm::new());
            }
        }
    });
}

/// One profile: marker, name (or its rename field), chip, identity line, and the buttons.
fn profile_row(ui: &mut Ui, p: &Profile, current: bool, page: &mut ProfilesPage, switching: Option<ProfileId>, msgs: &mut Vec<Msg>) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &p.name));
    if resp.clicked() {
        msgs.push(Msg::Profile(ProfileMsg::Select(p.id)));
    }
    let painter = ui.painter_at(rect);
    let fill = if page.selected == Some(p.id) {
        color::selected()
    } else if resp.hovered() {
        color::hover()
    } else {
        color::panel()
    };
    painter.rect_filled(rect, CornerRadius::same(6), fill);
    let marker = pos2(rect.left() + 16.0, rect.top() + 15.0);
    if current {
        painter.circle_filled(marker, 5.0, color::primary());
    } else {
        painter.circle_stroke(marker, 4.5, Stroke::new(1.5_f32, color::faint()));
    }
    let text_x = rect.left() + 30.0;
    let renaming = page.renaming.as_ref().is_some_and(|f| f.id == p.id);
    if !renaming {
        let name = painter.text(pos2(text_x, rect.top() + 15.0), Align2::LEFT_CENTER, &p.name, FontId::proportional(13.5), color::text());
        if current {
            widgets::chip(
                &painter,
                name.right() + 8.0,
                rect.top() + 15.0,
                CURRENT_CHIP,
                color::primary().gamma_multiply(0.25),
                color::text(),
            );
        }
    }
    let (line, line_color) = match identity_line(&p.author_name, &p.author_email) {
        Some(l) => (l, color::weak()),
        None => (NO_IDENTITY.to_owned(), color::faint()),
    };
    painter.text(pos2(text_x, rect.top() + 32.0), Align2::LEFT_CENTER, line, FontId::proportional(12.0), line_color);

    let buttons = rect.shrink2(vec2(10.0, 0.0));
    // Children that don't allocate: the row above already holds the space.
    let mut child = ui.new_child(UiBuilder::new().max_rect(buttons).layout(Layout::right_to_left(Align::Center)));
    {
        let ui = &mut child;
        let deletable = !current && switching != Some(p.id);
        let delete = ui.add_enabled(deletable, egui::Button::new(DELETE_BUTTON));
        let delete = if current { delete.on_disabled_hover_text(DELETE_DISABLED) } else { delete };
        if delete.clicked() {
            msgs.push(Msg::Profile(ProfileMsg::AskDelete(p.id)));
        }
        if ui.button(RENAME_BUTTON).clicked() {
            page.renaming = Some(RenameForm::new(p.id, &p.name));
        }
        if switching == Some(p.id) {
            ui.add(egui::Spinner::new().size(14.0));
        } else if !current && ui.add_enabled(switching.is_none(), egui::Button::new(SWITCH_BUTTON)).clicked() {
            msgs.push(Msg::Profile(ProfileMsg::Switch(p.id)));
        }
    }

    if renaming {
        let field = egui::Rect::from_min_size(pos2(text_x - 4.0, rect.top() + 3.0), vec2(240.0, 24.0));
        rename_field(&mut ui.new_child(UiBuilder::new().max_rect(field)), page, msgs);
        if let Some(e) = page.renaming.as_ref().and_then(|f| f.error.clone()) {
            ui.label(RichText::new(e).size(12.0).color(color::red()));
        }
    }
}

/// The inline rename field: Enter or leaving it commits, Esc cancels.
fn rename_field(ui: &mut Ui, page: &mut ProfilesPage, msgs: &mut Vec<Msg>) {
    let Some(form) = page.renaming.as_mut() else { return };
    let r = controls::field(ui, egui::TextEdit::singleline(&mut form.name).desired_width(240.0).margin(vec2(6.0, 3.0)));
    if form.focus {
        r.request_focus();
        form.focus = false;
    }
    if r.lost_focus() {
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            page.renaming = None;
        } else {
            msgs.push(Msg::Profile(ProfileMsg::Rename { id: form.id, name: form.name.clone() }));
        }
    }
}

fn add_form(ui: &mut Ui, page: &mut ProfilesPage, msgs: &mut Vec<Msg>) {
    let Some(form) = page.adding.as_mut() else { return };
    let mut cancel = false;
    row(ui, NEW_LABEL, |ui| {
        ui.label(RichText::new(NAME_LABEL).size(12.0).color(color::weak()));
        let name = text_field(ui, &mut form.name, NEW_NAME_HINT);
        let blank = form.name.trim().is_empty();
        let enter = name.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
        if let Some(e) = &form.error {
            ui.label(RichText::new(e).size(12.0).color(color::red()));
        }
        controls::checkbox(ui, &mut form.copy_current, COPY_CURRENT);
        controls::checkbox(ui, &mut form.copy_accounts, COPY_ACCOUNTS);
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if primary_button(ui, "Add", !blank).clicked() || (enter && !blank) {
                msgs.push(Msg::Profile(ProfileMsg::Add {
                    name: form.name.clone(),
                    copy_current: form.copy_current,
                    copy_accounts: form.copy_accounts,
                }));
            }
            cancel = ui.button("Cancel").clicked();
        });
    });
    if cancel {
        page.adding = None;
    }
}

fn fields(ui: &mut Ui, settings: &Settings, page: &mut ProfilesPage, git: &GitConfigValues, msgs: &mut Vec<Msg>) {
    let Some(p) = page.selected.and_then(|id| settings.profile(id)) else { return };
    let current = settings.active_profile == Some(p.id);
    let id = p.id;
    row(ui, "", |ui| {
        ui.label(RichText::new(editing_caption(&p.name, current)).size(14.0).strong().color(color::text()));
    });
    let mut set =
        |field: IdentityField, value: &str| msgs.push(Msg::Profile(ProfileMsg::SetIdentity { id, field, value: value.to_owned() }));
    row(ui, "Author Name", |ui| {
        if text_field(ui, &mut page.author_name, "Your Name").lost_focus() {
            set(IdentityField::AuthorName, &page.author_name);
        }
    });
    row(ui, "Author Email", |ui| {
        if text_field(ui, &mut page.author_email, "you@example.com").lost_focus() {
            set(IdentityField::AuthorEmail, &page.author_email);
        }
        help(ui, if current { CURRENT_HELP } else { OTHER_HELP });
    });
    row(ui, "Signing Key", |ui| {
        let hint = key_hint(p.signing_key.as_deref(), &git.signing_key);
        if text_field(ui, &mut page.signing_key, &hint).lost_focus() {
            set(IdentityField::SigningKey, &page.signing_key);
        }
        help(ui, KEY_HELP);
    });
    super::profile_setup::setup_rows(ui, settings, p, current);
}
