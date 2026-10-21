//! The pane of a diff, File view or blame whose file contents a partial clone hasn't downloaded
//! yet (state/partial.rs): downloading, with Cancel; waiting for the trust prompt or a retry;
//! or why it can't.

use super::empty_state::{EmptyIcon, Glyph, PaneButton, block};
use super::pane_status::{self, Failure};
use super::theme::color;
use super::widgets::severity::Severity;
use crate::msg::{Msg, PartialMsg};
use crate::state::AppState;
use crate::state::partial::{ContentPhase, PendingContent, Wait};
use crate::ui::theme::type_scale;
use eframe::egui::{RichText, Ui};

/// Draws `p`'s pane, centred in what is left of `ui`.
pub fn show(ui: &mut Ui, st: &AppState, p: &PendingContent, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let title = |ui: &Ui, text: String| RichText::new(text).font(type_scale::strong(ui, type_scale::BODY)).color(color::text());
    let path = &p.path;
    let remote = &p.remote;
    let source = format!("This repository is a partial clone: file contents come from {remote} when they are first needed.");
    let phase = st.content_phase(p);
    let stopping = match &phase {
        ContentPhase::Fetching { stopping, .. } => *stopping,
        ContentPhase::FetchingAsJob => st.job_cancelling(),
        ContentPhase::Waiting(_) => false,
    };
    match &phase {
        ContentPhase::Fetching { .. } | ContentPhase::FetchingAsJob if stopping => {
            block(ui, Glyph::Spinner, RichText::new("Cancelling the download…").color(color::weak()), None, &[]);
        }
        ContentPhase::Fetching { .. } | ContentPhase::FetchingAsJob => {
            let list = [PaneButton { text: t.tr("common.cancel.button"), primary: false }];
            let head = title(ui, format!("Downloading the contents of {path}…"));
            if block(ui, Glyph::Spinner, head, Some(&source), &list).is_some() {
                msgs.push(Msg::Partial(PartialMsg::StopContent));
            }
        }
        ContentPhase::Waiting(Wait::Trust) => {
            let hint = format!(
                "This repository is a partial clone and isn't trusted yet. Downloading them from {remote} runs git in it, so \
                 Trellis asks first."
            );
            let list = [PaneButton { text: "Download", primary: true }];
            let head = title(ui, format!("The contents of {path} haven't been downloaded"));
            if block(ui, Glyph::Icon(EmptyIcon::Shield), head, Some(&hint), &list).is_some() {
                msgs.push(Msg::Partial(PartialMsg::DownloadContent));
            }
        }
        ContentPhase::Waiting(Wait::Cancelled) => {
            let list = [PaneButton { text: "Download", primary: true }];
            let head = title(ui, "The download was cancelled".to_owned());
            let hint = format!("The contents of {path} haven't been downloaded. {source}");
            if block(ui, Glyph::Icon(EmptyIcon::File), head, Some(&hint), &list).is_some() {
                msgs.push(Msg::Partial(PartialMsg::DownloadContent));
            }
        }
        ContentPhase::Waiting(Wait::Failed(why)) => {
            let head = format!("Couldn't download the contents of {path}");
            let retry = Some(Msg::Partial(PartialMsg::DownloadContent));
            pane_status::failed(ui, t, Failure { title: &head, error: why, retry }, msgs);
        }
        ContentPhase::Waiting(Wait::StillMissing) => {
            let hint = format!("{remote} finished the download without them. It may no longer have this version of the file.");
            let list = [PaneButton { text: t.tr("common.try_again.button"), primary: false }];
            let head = title(ui, format!("The contents of {path} still aren't there"));
            if block(ui, Glyph::Severity(Severity::Warning), head, Some(&hint), &list).is_some() {
                msgs.push(Msg::Partial(PartialMsg::DownloadContent));
            }
        }
        ContentPhase::Waiting(Wait::OldGit(why)) => {
            let head = title(ui, format!("The contents of {path} haven't been downloaded"));
            block(ui, Glyph::Severity(Severity::Info), head, Some(why), &[]);
        }
    }
}
