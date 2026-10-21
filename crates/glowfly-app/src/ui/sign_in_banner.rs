//! The banner under the toolbar for remotes of the active repository that auto-fetch skips
//! because they need the user to sign in (or to confirm an SSH host key), with a fetch that can
//! ask and a way to hide the line (state/sign_in.rs, update/sign_in.rs). Each remote has its
//! own row.

use crate::msg::{CredentialMsg, Msg};
use crate::state::AppState;
use crate::state::sign_in::banner_line;
use crate::ui::theme::color;
use crate::ui::widgets::Icon;
use crate::ui::widgets::banner::{self, Banner, BannerKind};
use crate::ui::widgets::buttons;
use eframe::egui;

pub fn show(ctx: &egui::Context, st: &AppState, msgs: &mut Vec<Msg>) {
    let Some(snap) = st.snap.as_ref() else { return };
    let shown = st.sign_in.shown(&snap.workdir, &snap.remote_info);
    if shown.is_empty() {
        return;
    }
    egui::TopBottomPanel::top("sign_in_banner").frame(egui::Frame::new()).show(ctx, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for n in shown {
            let b = Banner::new(BannerKind::Info).icon(Icon::Key, color::info()).text(banner_line(n));
            banner::show(ui, b, |ui| {
                let (workdir, remote) = (n.workdir.clone(), n.remote.clone());
                if ui.add(buttons::secondary("Not now")).clicked() {
                    msgs.push(Msg::Credentials(CredentialMsg::DismissSignIn { workdir: workdir.clone(), remote: remote.clone() }));
                }
                if ui.add(buttons::primary(format!("Fetch {}", n.remote))).clicked() {
                    msgs.push(Msg::Credentials(CredentialMsg::SignInFetch { workdir, remote }));
                }
            });
        }
    });
}
