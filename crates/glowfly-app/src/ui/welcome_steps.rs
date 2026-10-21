//! The welcome's step bodies (ui/welcome.rs draws the header and footer around them), all
//! left-aligned on the content edge.

use super::theme::color;
use crate::msg::{Msg, PrefsMsg};
use crate::onboarding::OnboardingMsg;
use crate::state::git_env::GitState;
use crate::state::onboarding::{EMAIL_FIELD, NAME_FIELD, Welcome};
use crate::state::{PrefPage, host_text};
use crate::ui::theme::controls;
use crate::ui::theme::type_scale;
use crate::ui::widgets::buttons;
use crate::ui::widgets::field_group::FieldGroup;
use crate::ui::widgets::form_message::{self, Message};
use crate::ui::widgets::text_field::TextField;
use eframe::egui::{self, RichText};
use glowfly_core::forge::Connection;
use glowfly_core::i18n::Lang;
use glowfly_core::settings::Settings;

const DOWNLOAD_GIT: &str = "https://git-scm.com/downloads";
/// The identity fields' width (less when the content is narrower).
const FIELD_W: f32 = 360.0;

fn text(ui: &mut egui::Ui, s: &str) {
    ui.add(egui::Label::new(RichText::new(s).color(color::text())).wrap());
}

fn weak(ui: &mut egui::Ui, s: &str) {
    ui.add(egui::Label::new(RichText::new(s).color(color::weak())).wrap());
}

/// "Preferences > <page>", in the words the Preferences window shows.
fn prefs_path(t: Lang, page: PrefPage) -> String {
    format!("{} > {}", t.tr("prefs.nav.caption"), page.nav_label(t))
}

/// Step 1: the name and email for commits, filled from the global git config when it has them.
/// `keyboard` is false under a credential prompt, which needs the keys.
pub fn identity(ui: &mut egui::Ui, w: &mut Welcome, settings: &Settings, keyboard: bool) {
    let t = settings.lang();
    text(ui, t.tr("welcome.identity.intro"));
    ui.add_space(8.0);
    let f = &mut w.identity;
    if f.reading {
        ui.horizontal(|ui| {
            ui.add(egui::Spinner::new());
            weak(ui, t.tr("welcome.identity.reading"));
        });
        return;
    }
    if let Some(e) = &f.read_error {
        weak(ui, &t.trf("welcome.identity.read_error", &[("error", e)]));
    } else if f.git_knows() {
        weak(ui, t.tr("welcome.identity.from_git"));
    } else {
        weak(ui, t.tr("welcome.identity.unknown"));
    }
    ui.add_space(12.0);
    ui.add_enabled_ui(keyboard && !f.saving, |ui| {
        ui.scope(|ui| {
            ui.set_max_width(FIELD_W.min(ui.available_width()));
            // A label above each field and one line under it: its problem once it was edited or
            // Continue was pressed, worked out from the text as it is now, or else its help.
            let (name_p, email_p) = f.field_problems(t);
            let r = FieldGroup::new(t.tr("welcome.identity.name.label"))
                .help(t.tr("welcome.identity.name.help"))
                .message(&f.touched, NAME_FIELD, name_p.map(Message::error))
                .show(ui, |ui| ui.add(TextField::new(&mut f.name).hint("Ada Lovelace")));
            form_message::track(&mut f.touched, NAME_FIELD, &r);
            let r = FieldGroup::new(t.tr("welcome.identity.email.label"))
                .help(t.tr("welcome.identity.email.help"))
                .message(&f.touched, EMAIL_FIELD, email_p.map(Message::error))
                .show(ui, |ui| ui.add(TextField::new(&mut f.email).hint("ada@example.com")));
            form_message::track(&mut f.touched, EMAIL_FIELD, &r);
        });
    });
    if let Some(p) = settings.active_profile() {
        ui.add_space(8.0);
        weak(ui, &t.trf("welcome.identity.profile_note", &[("profile", &p.name)]));
    }
    // The answer to the last try: git's write error or a refusal.
    if let Some(e) = &f.error {
        ui.add_space(12.0);
        form_message::form(ui, Message::error(e.as_str()));
    }
}

/// Step 2: why to sign in to a hosting service, and the accounts already connected.
pub fn connect(ui: &mut egui::Ui, t: Lang, connections: &[Connection], msgs: &mut Vec<Msg>) {
    text(ui, t.tr("welcome.connect.intro"));
    ui.add_space(4.0);
    weak(ui, &t.trf("welcome.connect.later", &[("page", &prefs_path(t, PrefPage::Integrations))]));
    ui.add_space(10.0);
    let label = if connections.is_empty() {
        t.tr("welcome.connect.first.button")
    } else {
        ui.label(RichText::new(t.tr("welcome.connect.connected")).font(type_scale::strong(ui, type_scale::BODY)).color(color::text()));
        for c in connections {
            ui.horizontal(|ui| {
                ui.label(RichText::new(c.account.provider.name()).color(color::text()));
                ui.label(RichText::new(&c.user).color(color::text()));
                let host = host_text(&c.account);
                if c.account.provider.fixed_host() != Some(host.as_str()) {
                    ui.label(RichText::new(host).color(color::weak()));
                }
            });
        }
        ui.add_space(6.0);
        t.tr("welcome.connect.another.button")
    };
    if ui.add(buttons::secondary(label)).clicked() {
        msgs.push(Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Integrations)));
    }
}

/// Step 3: whether git was found, from the check made at startup.
pub fn git(ui: &mut egui::Ui, t: Lang, git: &GitState, msgs: &mut Vec<Msg>) {
    text(ui, t.tr("welcome.git.intro"));
    ui.add_space(10.0);
    let Some(info) = &git.info else {
        ui.horizontal(|ui| {
            ui.add(egui::Spinner::new());
            weak(ui, t.tr("welcome.git.looking"));
        });
        return;
    };
    match info.banner() {
        None => {
            let version = info.version.map(glowfly_core::git_version::show).unwrap_or_default();
            ui.label(
                RichText::new(t.trf("welcome.git.ready", &[("version", &version)]))
                    .font(type_scale::strong(ui, type_scale::BODY))
                    .color(color::added()),
            );
            if let Some(path) = &info.path {
                weak(ui, &glowfly_core::native_path::shown(path));
            }
        }
        Some(problem) => {
            form_message::form(ui, Message::warning(problem));
            ui.add_space(6.0);
            if ui.link(t.tr("welcome.git.download.button")).clicked() {
                msgs.push(Msg::OpenUrl(DOWNLOAD_GIT.into()));
            }
        }
    }
}

/// Step 4: open, clone or start a repository as cards, then the tour and where the look is
/// changed in one quiet row.
pub fn start(ui: &mut egui::Ui, t: Lang, w: &mut Welcome, has_repo: bool, msgs: &mut Vec<Msg>) {
    text(ui, t.tr("welcome.start.intro"));
    ui.add_space(16.0);
    super::welcome_cards::choices(ui, t, msgs);
    ui.add_space(16.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 16.0;
        if has_repo {
            if ui.link(t.tr("welcome.start.tour_now.link")).clicked() {
                msgs.push(Msg::Onboarding(OnboardingMsg::StartTour));
            }
        } else {
            controls::checkbox(ui, &mut w.tour_after, t.tr("welcome.start.tour_after.check"));
        }
        if ui.link(t.trf("welcome.start.look_link", &[("page", &prefs_path(t, PrefPage::Ui))])).clicked() {
            msgs.push(Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Ui)));
        }
    });
}
