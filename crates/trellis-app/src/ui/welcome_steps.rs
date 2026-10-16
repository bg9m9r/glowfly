//! The welcome's step bodies (ui/welcome.rs draws the header and footer around them).

use super::theme::color;
use crate::msg::{Msg, PrefsMsg};
use crate::onboarding::{OnboardingMsg, StartChoice};
use crate::state::git_env::GitState;
use crate::state::onboarding::Welcome;
use crate::state::{PrefPage, host_text};
use eframe::egui::{self, RichText, vec2};
use trellis_core::forge::Connection;
use trellis_core::settings::Settings;

const DOWNLOAD_GIT: &str = "https://git-scm.com/downloads";

fn text(ui: &mut egui::Ui, s: &str) {
    ui.add(egui::Label::new(RichText::new(s).color(color::text())).wrap());
}

fn weak(ui: &mut egui::Ui, s: &str) {
    ui.add(egui::Label::new(RichText::new(s).color(color::weak())).wrap());
}

/// Step 1: the name and email for commits, filled from the global git config when it has them.
/// `keyboard` is false under a credential prompt, which needs the keys.
pub fn identity(ui: &mut egui::Ui, w: &mut Welcome, settings: &Settings, keyboard: bool) {
    text(
        ui,
        "Every commit carries a name and an email address. Trellis saves them in your global git config, so every \
         repository on this computer uses them.",
    );
    ui.add_space(8.0);
    let f = &mut w.identity;
    if f.reading {
        ui.horizontal(|ui| {
            ui.add(egui::Spinner::new());
            weak(ui, "Reading your git settings…");
        });
        return;
    }
    if let Some(e) = &f.read_error {
        weak(ui, &format!("Your git settings could not be read: {e}"));
    } else if f.git_knows() {
        weak(ui, "These come from your global git config. You can change them here.");
    } else {
        weak(ui, "Git doesn't know who you are yet.");
    }
    ui.add_space(6.0);
    ui.add_enabled_ui(keyboard && !f.saving, |ui| {
        egui::Grid::new("welcome-identity").num_columns(2).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
            ui.label("Name");
            ui.add(egui::TextEdit::singleline(&mut f.name).hint_text("Ada Lovelace").desired_width(320.0));
            ui.end_row();
            ui.label("Email");
            ui.add(egui::TextEdit::singleline(&mut f.email).hint_text("ada@example.com").desired_width(320.0));
            ui.end_row();
        });
    });
    if let Some(p) = settings.active_profile() {
        weak(ui, &format!("The change also goes to the {} profile.", p.name));
    }
    if let Some(e) = &f.error {
        ui.add_space(4.0);
        ui.add(egui::Label::new(RichText::new(e).color(color::red())).wrap());
    }
}

/// Step 2: why to sign in to a hosting service, and the accounts already connected.
pub fn connect(ui: &mut egui::Ui, connections: &[Connection], msgs: &mut Vec<Msg>) {
    text(
        ui,
        "Signing in to a hosting service such as GitHub, GitLab, Bitbucket or Azure DevOps brings its pull requests and \
         issues into Trellis, lets you clone from your account and publishes new repositories there.",
    );
    ui.add_space(4.0);
    weak(ui, "You can also do this later in Preferences > Integrations.");
    ui.add_space(10.0);
    let label = if connections.is_empty() {
        "Connect a service…"
    } else {
        ui.label(RichText::new("You're connected").strong().color(color::text()));
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
        "Connect another…"
    };
    if ui.button(label).clicked() {
        msgs.push(Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Integrations)));
    }
}

/// Step 3: whether git was found, from the check made at startup.
pub fn git(ui: &mut egui::Ui, git: &GitState, msgs: &mut Vec<Msg>) {
    text(ui, "Trellis runs the git installed on this computer, so your git settings, hooks and credentials apply.");
    ui.add_space(10.0);
    let Some(info) = &git.info else {
        ui.horizontal(|ui| {
            ui.add(egui::Spinner::new());
            weak(ui, "Looking for git…");
        });
        return;
    };
    match info.banner() {
        None => {
            let version = info.version.map(trellis_core::git_version::show).unwrap_or_default();
            ui.label(RichText::new(format!("Git {version} is ready")).strong().color(color::green()));
            if let Some(path) = &info.path {
                weak(ui, &trellis_core::native_path::shown(path));
            }
        }
        Some(problem) => {
            ui.add(egui::Label::new(RichText::new(problem).color(color::amber())).wrap());
            ui.add_space(4.0);
            if ui.link("Download git").clicked() {
                msgs.push(Msg::OpenUrl(DOWNLOAD_GIT.into()));
            }
        }
    }
}

/// Step 4: open, clone or start a repository, the tour, and where the look is changed.
pub fn start(ui: &mut egui::Ui, w: &mut Welcome, has_repo: bool, msgs: &mut Vec<Msg>) {
    text(ui, "Open a repository you already have, clone one from a hosting service, or start a new one.");
    ui.add_space(10.0);
    ui.vertical_centered(|ui| {
        let size = vec2(260.0, 34.0);
        let choices = [
            (StartChoice::Open, "Open a repository"),
            (StartChoice::Clone, "Clone a repository"),
            (StartChoice::Init, "Start a new repository"),
        ];
        for (choice, label) in choices {
            if ui.add(egui::Button::new(label).min_size(size)).clicked() {
                msgs.push(Msg::Onboarding(OnboardingMsg::Finish(Some(choice))));
            }
        }
        ui.add_space(6.0);
        if has_repo {
            if ui.button("Take the tour now").clicked() {
                msgs.push(Msg::Onboarding(OnboardingMsg::StartTour));
            }
        } else {
            ui.checkbox(&mut w.tour_after, "Show me around once it opens");
        }
    });
    ui.add_space(10.0);
    if ui.link("Change the theme and layout in Preferences > UI Customization").clicked() {
        msgs.push(Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Ui)));
    }
}
