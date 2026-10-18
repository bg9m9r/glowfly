//! Which dialogs are forms, information or confirm dialogs, and what a form's draft counts.

use super::DialogKind;
use crate::conflict_check::CheckRequest;
use crate::msg::Msg;
use crate::state::clone::CloneForm;
use crate::state::conflict_check::ConflictReport;
use crate::state::forks::{ForkCreate, ForkStep};
use crate::state::init::InitForm;
use crate::state::publish::PublishForm;
use crate::state::trust::TrustPrompt;
use crate::state::worktree_manage::RemoveForm;
use crate::state::{Modal, clone_service::ServicePicker};
use git2::Oid;
use std::path::PathBuf;
use trellis_core::conflict_check::{MergeTarget, TargetKind};
use trellis_core::forge::{Provider, Target};
use trellis_core::repo::{ChangeCounts, ChangesRead, WorktreeInfo};

fn folder(name: &str) -> PathBuf {
    trellis_core::paths::temp_dir().join(name)
}

fn worktree() -> WorktreeInfo {
    WorktreeInfo {
        name: Some("feature".into()),
        path: folder("app-feature"),
        branch: Some("feature".into()),
        head: None,
        current: false,
        locked: None,
        missing: false,
        prunable: false,
        folder: None,
        changes: ChangeCounts::default(),
        read: ChangesRead::NotCounted,
    }
}

fn target() -> Target {
    Target { provider: Provider::GitHub, host: "github.com".into(), owner: "ada".into(), project: String::new(), repo: "app".into() }
}

fn publish(owners: &[&str], owner: &str) -> Modal {
    Modal::Publish(Box::new(PublishForm {
        workdir: folder("app"),
        branch: "main".into(),
        accounts: Vec::new(),
        account: 0,
        user: None,
        owners: owners.iter().map(|o| o.to_string()).collect(),
        owner: owner.into(),
        name: "app".into(),
        description: String::new(),
        private: true,
        loading: false,
        creating: false,
        error: None,
        touched: Default::default(),
    }))
}

#[test]
fn forms_info_and_confirms_are_told_apart() {
    let forms = [
        Modal::NewBranch { start: None, name: String::new(), checkout: true, touched: Default::default() },
        Modal::Clone(CloneForm::new(String::new())),
        Modal::RenameBranch { old: "feature".into(), name: "feature".into(), touched: Default::default() },
        Modal::CreatePr {
            source: "feature".into(),
            target: "main".into(),
            targets: vec!["main".into()],
            title: String::new(),
            description: String::new(),
            draft: false,
            items: String::new(),
        },
    ];
    let request = CheckRequest {
        branch: "feature".into(),
        ours: Oid::zero(),
        target: MergeTarget { name: "main".into(), oid: Oid::zero(), kind: TargetKind::DefaultBranch },
    };
    let info = [
        Modal::About,
        Modal::Shortcuts { filter: "stage".into() },
        Modal::ConflictReport(ConflictReport { request, result: Err("failed".into()) }),
    ];
    let trust = TrustPrompt {
        name: "app".into(),
        workdir: folder("app"),
        real: folder("app"),
        scan: Default::default(),
        refused: false,
        held: None,
        gained: None,
    };
    let fork = ForkCreate {
        workdir: folder("app"),
        remote: "origin".into(),
        target: target(),
        step: ForkStep::Checking,
        name: "ada".into(),
        fetch_after: true,
        error: None,
        touched: Default::default(),
    };
    let confirms = [
        Modal::Confirm { title: "T".into(), body: "B".into(), button: "Go".into(), then: Box::new(Msg::ClearLog) },
        Modal::danger("T", "B", "Go", Msg::ClearLog),
        Modal::Trust(trust),
        Modal::RemoveWorktree(RemoveForm::new(&worktree())),
        Modal::CreateFork(Box::new(fork)),
    ];
    for m in &forms {
        assert_eq!(m.kind(), DialogKind::Form);
        assert!(m.draft().is_some(), "a form has a draft");
    }
    for m in &info {
        assert_eq!(m.kind(), DialogKind::Info);
        assert_eq!(m.draft(), None);
    }
    for m in &confirms {
        assert_eq!(m.kind(), DialogKind::Confirm);
        assert_eq!(m.draft(), None);
    }
}

#[test]
fn draft_leaves_out_what_loads_later() {
    // A holder filled in from git's user.name isn't something the user typed.
    let empty = InitForm::new(String::new(), String::new());
    let mut suggested = empty.clone();
    suggested.holder = "Ada".into();
    suggested.holder_suggested = "Ada".into();
    let draft = |f: &InitForm| Modal::Init(f.clone()).draft();
    assert_eq!(draft(&suggested), draft(&empty));
    let mut typed = suggested.clone();
    typed.holder = "Ada Lovelace".into();
    assert_ne!(draft(&typed), draft(&empty), "a holder the user typed counts");

    // The parent folder's disk check and the service's list arrive after opening.
    let clone = CloneForm::new("/work".into());
    let mut loaded = clone.clone();
    loaded.parent_checked = Some(("/work".into(), None));
    loaded.service = ServicePicker { loading: true, ..ServicePicker::default() };
    let draft = |f: &CloneForm| Modal::Clone(f.clone()).draft();
    assert_eq!(draft(&loaded), draft(&clone));
    let mut typed = clone.clone();
    typed.url = "https://example.com/r.git".into();
    assert_ne!(draft(&typed), draft(&clone));

    // The first owner is filled in when the owners load.
    assert_eq!(publish(&["ada", "acme"], "ada").draft(), publish(&[], "").draft());
    assert_ne!(publish(&["ada", "acme"], "acme").draft(), publish(&[], "").draft(), "picking another owner counts");
}
