//! Open anyway with the runner: a folder whose `.git` file names a network place (one that
//! doesn't exist, so nothing is ever there) fails to open and asks; Open anyway gives the
//! approval to the opens before the load runs, so the load gets past the check and fails as an
//! ordinary open of a folder holding no repository does; forgetting it from Preferences makes
//! the next open ask again.

use super::tests::{drive, state};
use glowfly_model::msg::{Msg, RepoMsg, TrustMsg};
use glowfly_model::state::Modal;
use std::path::PathBuf;

/// A network place that is never there.
fn net_place() -> PathBuf {
    if cfg!(windows) { PathBuf::from(r"\\glowfly-test.invalid\share") } else { PathBuf::from("/net/glowfly-test.invalid") }
}

#[test]
fn open_anyway_lets_the_load_past_the_check() {
    let dir = glowfly_core::paths::temp_dir().join(format!("glowfly-test-flows-open-anyway-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let named = net_place().join("x/.git").display().to_string().replace('\\', "/");
    std::fs::write(dir.join(".git"), format!("gitdir: {named}\n")).unwrap();
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    assert!(st.snap.is_none());
    assert!(matches!(&st.modal, Some(Modal::Confirm { then, .. }) if matches!(&**then, Msg::Repo(RepoMsg::OpenAnyway { .. }))), "asks");
    let refused = st.log.last().expect("the failure is logged").text.clone();
    assert!(refused.contains("network place"), "{refused}");

    drive(&mut st, Msg::ConfirmAccepted);
    assert!(st.modal.is_none(), "it doesn't ask again");
    assert!(!st.settings.network_places_for(&dir).is_empty());
    let failed = st.log.last().expect("the open's failure").text.clone();
    assert!(!failed.contains("network place") && failed.contains("not a git repository"), "an ordinary failed open: {failed}");

    // Forgotten: the next open asks again.
    drive(&mut st, Msg::Trust(TrustMsg::ForgetNetworkPlaces(dir.clone())));
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    assert!(matches!(&st.modal, Some(Modal::Confirm { .. })), "asks again");
    let _ = std::fs::remove_dir_all(&dir);
}
