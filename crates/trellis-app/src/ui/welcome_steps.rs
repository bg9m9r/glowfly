//! The welcome's step bodies (ui/welcome.rs draws the header and footer around them).

use super::theme::color;
use crate::msg::{Msg, PrefsMsg};
use crate::onboarding::{OnboardingMsg, StartChoice};
use crate::state::git_env::GitState;
use crate::state::onboarding::Welcome;
use crate::state::{PrefPage, host_text};
use eframe::egui::{self, RichText, vec2};
use trellis_core::forge::Connection;
use trellis_core::i18n::Lang;
use trellis_core::settings::Settings;

const DOWNLOAD_GIT: &str = "https://git-scm.com/downloads";

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
    ui.add_space(6.0);
    ui.add_enabled_ui(keyboard && !f.saving, |ui| {
        egui::Grid::new("welcome-identity").num_columns(2).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
            ui.label(t.tr("welcome.identity.name.label"));
            ui.add(egui::TextEdit::singleline(&mut f.name).hint_text("Ada Lovelace").desired_width(320.0));
            ui.end_row();
            ui.label(t.tr("welcome.identity.email.label"));
            ui.add(egui::TextEdit::singleline(&mut f.email).hint_text("ada@example.com").desired_width(320.0));
            ui.end_row();
        });
    });
    if let Some(p) = settings.active_profile() {
        weak(ui, &t.trf("welcome.identity.profile_note", &[("profile", &p.name)]));
    }
    if let Some(e) = &f.error {
        ui.add_space(4.0);
        ui.add(egui::Label::new(RichText::new(e).color(color::red())).wrap());
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
        ui.label(RichText::new(t.tr("welcome.connect.connected")).strong().color(color::text()));
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
    if ui.button(label).clicked() {
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
            let version = info.version.map(trellis_core::git_version::show).unwrap_or_default();
            ui.label(RichText::new(t.trf("welcome.git.ready", &[("version", &version)])).strong().color(color::green()));
            if let Some(path) = &info.path {
                weak(ui, &trellis_core::native_path::shown(path));
            }
        }
        Some(problem) => {
            ui.add(egui::Label::new(RichText::new(problem).color(color::amber())).wrap());
            ui.add_space(4.0);
            if ui.link(t.tr("welcome.git.download.button")).clicked() {
                msgs.push(Msg::OpenUrl(DOWNLOAD_GIT.into()));
            }
        }
    }
}

/// Step 4: open, clone or start a repository, the tour, and where the look is changed.
pub fn start(ui: &mut egui::Ui, t: Lang, w: &mut Welcome, has_repo: bool, msgs: &mut Vec<Msg>) {
    text(ui, t.tr("welcome.start.intro"));
    ui.add_space(10.0);
    ui.vertical_centered(|ui| {
        let size = vec2(260.0, 34.0);
        let choices = [
            (StartChoice::Open, t.tr("common.open_repo.button")),
            (StartChoice::Clone, t.tr("common.clone_repo.button")),
            (StartChoice::Init, t.tr("common.init_repo.button")),
        ];
        for (choice, label) in choices {
            if ui.add(egui::Button::new(label).min_size(size)).clicked() {
                msgs.push(Msg::Onboarding(OnboardingMsg::Finish(Some(choice))));
            }
        }
        ui.add_space(6.0);
        if has_repo {
            if ui.button(t.tr("welcome.start.tour_now.button")).clicked() {
                msgs.push(Msg::Onboarding(OnboardingMsg::StartTour));
            }
        } else {
            ui.checkbox(&mut w.tour_after, t.tr("welcome.start.tour_after.check"));
        }
    });
    ui.add_space(10.0);
    if ui.link(t.trf("welcome.start.look_link", &[("page", &prefs_path(t, PrefPage::Ui))])).clicked() {
        msgs.push(Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Ui)));
    }
}
