//! `Msg::OpenLink`: a link whose text names its own host opens; one whose text names another
//! host asks first, naming the real host in punycode, and opens only when accepted.

use super::tests::state;
use super::update;
use crate::effect::{Effect, OsEffect};
use crate::msg::Msg;
use crate::state::Modal;
use std::time::Instant;

fn opened(fx: &[Effect]) -> Option<&str> {
    match fx {
        [Effect::Os(OsEffect::OpenUrl(u))] => Some(u),
        _ => None,
    }
}

#[test]
fn a_link_whose_text_matches_its_host_opens_at_once() {
    let mut st = state();
    for (text, url) in [("docs", "https://github.com/x"), ("www.github.com", "https://github.com/")] {
        let fx = update(&mut st, Msg::OpenLink { text: text.into(), url: url.into() }, Instant::now());
        assert_eq!(opened(&fx), Some(url), "{text}");
        assert!(st.modal.is_none());
    }
}

#[test]
fn a_link_whose_text_names_another_host_asks_first() {
    let mut st = state();
    let url = "https://evil.example/login";
    let fx = update(&mut st, Msg::OpenLink { text: "https://github.com/acme/app".into(), url: url.into() }, Instant::now());
    assert!(fx.is_empty(), "nothing opens before the user answers");
    let Some(Modal::ConfirmDanger { title, body, button, ack: None, .. }) = &st.modal else { panic!("no danger confirmation") };
    assert_eq!(title, "Open a different site?");
    assert_eq!(
        body,
        "The link text shows github.com, but the link goes to evil.example.\n\nhttps://evil.example/login\n\nOnly open it if you trust evil.example."
    );
    assert_eq!(button, "Open link");
    let fx = update(&mut st, Msg::ConfirmAccepted, Instant::now());
    assert_eq!(opened(&fx), Some(url));
}

#[test]
fn a_look_alike_host_is_named_in_punycode() {
    let mut st = state();
    let fx = update(&mut st, Msg::OpenLink { text: "github.com/acme".into(), url: "https://g\u{456}thub.com/acme".into() }, Instant::now());
    assert!(fx.is_empty());
    let Some(Modal::ConfirmDanger { body, .. }) = &st.modal else { panic!("no confirmation") };
    assert!(body.starts_with("The link text shows github.com, but the link goes to xn--gthub-n2e.com."), "{body}");
    assert!(body.contains("\n\nhttps://xn--gthub-n2e.com/acme\n\n"), "{body}");
    assert!(!body.contains('\u{456}'), "{body}");
}
