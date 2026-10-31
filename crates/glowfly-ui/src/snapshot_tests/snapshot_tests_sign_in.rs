//! The sign-in banner under the toolbar for a remote auto-fetch skips, with its fetch and
//! "Not now" buttons. Shares the fixture repository and renderer with `snapshot_tests`.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use crate::msg::{CredentialMsg, Msg};
use crate::state::sign_in::SignInNeeded;
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use glowfly_core::auth_failure::AuthNeed;
use glowfly_core::repo::RemoteInfo;
use glowfly_core::settings::ThemeChoice;

#[test]
fn sign_in_banner() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "sign-in-banner");
    let snap = st.snap.as_mut().unwrap();
    snap.remote_info.retain(|r| r.name != "origin");
    let url = "https://github.com/team/app.git";
    snap.remote_info.push(RemoteInfo { name: "origin".into(), urls: vec![url.into()], ..RemoteInfo::default() });
    let workdir = snap.workdir.clone();
    st.sign_in.pause(SignInNeeded {
        workdir: workdir.clone(),
        remote: "origin".into(),
        host: Some("github.com".into()),
        need: AuthNeed::Https,
        dismissed: false,
    });
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("sign_in_banner");
    assert!(h.query_by_label_contains("origin (github.com) needs you to sign in").is_some());
    h.get_by_label("Fetch origin").click();
    h.run();
    let fetch = |m: &Msg| matches!(m, Msg::Credentials(CredentialMsg::SignInFetch { remote, .. }) if remote == "origin");
    assert!(h.state().1.iter().any(fetch));
    h.get_by_label("Not now").click();
    h.run();
    let dismiss =
        |m: &Msg| matches!(m, Msg::Credentials(CredentialMsg::DismissSignIn { workdir: w, remote }) if *w == workdir && remote == "origin");
    assert!(h.state().1.iter().any(dismiss));
    drop(h);
    remove_fixture(&dir);
}
