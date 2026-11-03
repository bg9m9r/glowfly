//! The question a link in a description asks when its text names another host than the one it
//! goes to (`Msg::OpenLink`): the real host and address in punycode, Open link (a danger
//! button, Cancel the default) to go ahead.
//! Shares the fixture repository and renderer with `snapshot_tests`.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use crate::msg::Msg;
use crate::state::Modal;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;

fn prompt(name: &str, lang: &str, text: &str, url: &str) {
    let (mut st, dir) = with_repo(ThemeChoice::Light, &name.replace('_', "-"));
    st.settings.language = lang.into();
    crate::update::update(&mut st, Msg::OpenLink { text: text.into(), url: url.into() }, std::time::Instant::now());
    assert!(matches!(&st.modal, Some(Modal::ConfirmDanger { .. })), "a mismatched link asks first");
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot(name);
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn link_prompt_other_host() {
    prompt("link_prompt_other_host", "en", "https://github.com/acme/app", "https://evil.example/login");
}

#[test]
fn link_prompt_look_alike_host() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "link-prompt-look-alike");
    st.settings.language = "en".into();
    let url = "https://g\u{456}thub.com/acme";
    crate::update::update(&mut st, Msg::OpenLink { text: "github.com/acme".into(), url: url.into() }, std::time::Instant::now());
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("link_prompt_look_alike_host");
    assert!(h.get_by_role_and_label(Role::Button, "Cancel").is_focused(), "Cancel is the default, not the other site");
    h.get_by_label("Open link").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::ConfirmAccepted)), "the button accepts");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn link_prompt_german() {
    prompt("link_prompt_german", "de", "https://github.com/acme/app", "https://evil.example/login");
}

#[test]
fn link_prompt_spanish() {
    prompt("link_prompt_spanish", "es", "https://github.com/acme/app", "https://evil.example/login");
}
