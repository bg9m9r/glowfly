//! Opening a link clicked in a description or comment from a hosting service
//! (`Msg::OpenLink`): straight away, unless its visible text reads as an address on another
//! host than the link's, when a confirmation names the real host (in punycode, like the
//! address it shows) and opens the link only if the user goes ahead; Cancel is its default.

use crate::effect::{Effect, OsEffect};
use crate::msg::Msg;
use crate::state::{AppState, Modal};
use glowfly_core::forge::{link_mismatch, shown_url};

pub(super) fn open(st: &mut AppState, text: &str, url: String, fx: &mut Vec<Effect>) {
    let Some((shown, host)) = link_mismatch(text, &url) else {
        fx.push(Effect::Os(OsEffect::OpenUrl(url)));
        return;
    };
    let t = st.settings.lang();
    let body = t.trf("markdown.link.mismatch.body", &[("shown", &shown), ("host", &host), ("url", &shown_url(&url))]);
    // The danger form: Cancel is the default, so a reflexive Enter doesn't open the other site.
    let (title, button) = (t.tr("markdown.link.mismatch.title"), t.tr("markdown.link.mismatch.button"));
    st.modal = Some(Modal::danger(title, body, button, Msg::OpenUrl(url)));
}
