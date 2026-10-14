//! Which hosting services the app contacts, and with which credentials.

use super::tests::state;
use super::update;
use crate::effect::Effect;
use crate::msg::Msg;
use crate::state::{AppState, ForgeState, Status};
use std::time::Instant;
use trellis_core::forge::{Connection, Provider, Target};

fn target(provider: Provider, host: &str) -> Target {
    Target { provider, host: host.into(), owner: "o".into(), project: String::new(), repo: "r".into() }
}

fn with_host(t: &Target) -> AppState {
    let mut st = state();
    st.forge = Some(ForgeState::new(Some(t.clone()), "origin".into(), Some(t.clone())));
    st
}

/// The `connected` flag of each forge call `msg` starts.
fn forge_calls(st: &mut AppState, msg: Msg) -> Vec<bool> {
    update(st, msg, Instant::now())
        .into_iter()
        .filter_map(|e| match e {
            Effect::Forge { connected, .. } => Some(connected),
            _ => None,
        })
        .collect()
}

#[test]
fn guessed_self_hosted_servers_are_never_contacted() {
    for t in [target(Provider::GitHubEnterprise, "github.evil.example"), target(Provider::GitLabSelfManaged, "gitlab.evil.example")] {
        let mut st = with_host(&t);
        for interactive in [false, true] {
            assert!(forge_calls(&mut st, Msg::ForgeRefresh { interactive }).is_empty(), "{t:?}");
        }
        let fs = st.forge.as_ref().unwrap();
        assert!(matches!(&fs.host_status, Status::Error(e) if e.contains("not connected")), "{:?}", fs.host_status);
        assert_eq!(fs.loading, 0);
    }
}

#[test]
fn connected_self_hosted_servers_and_public_hosts_are_contacted() {
    let ghe = target(Provider::GitHubEnterprise, "github.acme.io");
    let mut st = with_host(&ghe);
    st.settings.connections.push(Connection { account: ghe.account(), user: "ada".into(), method: "access token".into() });
    assert_eq!(forge_calls(&mut st, Msg::ForgeRefresh { interactive: false }), [true]);

    let mut st = with_host(&target(Provider::GitHub, "github.com"));
    assert_eq!(forge_calls(&mut st, Msg::ForgeRefresh { interactive: false }), [false], "public, not connected");
}
