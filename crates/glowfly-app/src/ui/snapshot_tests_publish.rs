//! The Publish to GitHub dialog after the service refused the name: the refusal in plain words,
//! with the service's own text under a collapsed Details.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use crate::msg::{Msg, PublishMsg};
use crate::state::{Modal, publish::PublishForm};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use glowfly_core::forge::{Account, Provider};
use glowfly_core::settings::ThemeChoice;
use std::time::Instant;

const REFUSAL: &str = "HTTP 422: the server said “Repository creation failed. name already exists on this account”";

#[test]
fn publish_to_github_dialog() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "publish");
    st.modal = Some(Modal::Publish(Box::new(PublishForm {
        workdir: dir.clone(),
        branch: "main".into(),
        accounts: vec![Account { provider: Provider::GitHub, host: "github.com".into(), org: String::new() }],
        account: 0,
        user: Some("ada".into()),
        owners: vec!["ada".into(), "acme".into()],
        owner: "ada".into(),
        name: "demo".into(),
        description: "A small demo project".into(),
        private: true,
        loading: false,
        creating: true,
        error: None,
        error_detail: None,
        touched: Default::default(),
    })));
    let published = PublishMsg::Published { workdir: dir.clone(), branch: "main".into(), result: Err(REFUSAL.into()) };
    crate::update::update(&mut st, Msg::Publish(published), Instant::now());
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("publish_dialog");
    assert!(h.query_by_label("A repository called demo already exists on ada. Choose another name.").is_some());
    assert!(h.query_by_label_contains("Repository creation failed").is_none(), "Details starts closed");
    h.get_by_label("Details").click();
    h.run();
    assert!(h.query_by_label_contains("Repository creation failed").is_some(), "the service's words");
    remove_fixture(&dir);
}
