//! Opening a link clicked in a description or comment from a hosting service
//! (`Msg::OpenLink`): straight away, unless its visible text reads as an address on another
//! host than the link's, when a confirmation names the real host (in punycode, like the
//! address it shows) and opens the link only if the user goes ahead.

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
    st.modal = Some(Modal::Confirm {
        title: t.tr("markdown.link.mismatch.title").to_owned(),
        body,
        button: t.tr("markdown.link.mismatch.button").to_owned(),
        then: Box::new(Msg::OpenUrl(url)),
    });
}
