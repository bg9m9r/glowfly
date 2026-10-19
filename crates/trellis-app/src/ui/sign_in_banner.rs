//! The banner under the toolbar for remotes of the active repository that auto-fetch skips
//! because they need the user to sign in (or to confirm an SSH host key), with a fetch that can
//! ask and a way to hide the line (state/sign_in.rs, update/sign_in.rs).

use crate::msg::{CredentialMsg, Msg};
use crate::state::AppState;
use crate::state::sign_in::banner_line;
use crate::ui::theme::color;
use crate::ui::widgets::buttons;
use crate::ui::widgets::{self, Icon};
use eframe::egui::{self, RichText};

pub fn show(ctx: &egui::Context, st: &AppState, msgs: &mut Vec<Msg>) {
    let Some(snap) = st.snap.as_ref() else { return };
    let shown = st.sign_in.shown(&snap.workdir, &snap.remote_info);
    if shown.is_empty() {
        return;
    }
    egui::TopBottomPanel::top("sign_in_banner")
        .frame(egui::Frame::new().fill(color::banner()).inner_margin(egui::Margin::symmetric(12, 6)))
        .show(ctx, |ui| {
            for n in shown {
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let (workdir, remote) = (n.workdir.clone(), n.remote.clone());
                        if ui.add(buttons::secondary("Not now")).clicked() {
                            msgs.push(Msg::Credentials(CredentialMsg::DismissSignIn { workdir: workdir.clone(), remote: remote.clone() }));
                        }
                        if ui.add(buttons::primary(format!("Fetch {}", n.remote))).clicked() {
                            msgs.push(Msg::Credentials(CredentialMsg::SignInFetch { workdir, remote }));
                        }
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            let (rect, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                            widgets::paint_icon(ui.painter(), rect.center(), Icon::Key, color::text());
                            ui.add(egui::Label::new(RichText::new(banner_line(n)).color(color::text())).wrap());
                        });
                    });
                });
            }
        });
}
