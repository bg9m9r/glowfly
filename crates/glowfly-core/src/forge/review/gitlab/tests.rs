use super::*;

#[test]
fn merge_state_mapping() {
    use MergeState::*;
    let detailed = [
        ("mergeable", Clean),
        ("conflict", Conflicts),
        ("need_rebase", Behind),
        ("ci_must_pass", Blocked),
        ("ci_still_running", Blocked),
        ("status_checks_must_pass", Blocked),
        ("security_policy_pipeline_check", Blocked),
        ("security_policy_violations", Blocked),
        ("not_approved", Blocked),
        ("requested_changes", Blocked),
        ("discussions_not_resolved", Blocked),
        ("merge_request_blocked", Blocked),
        ("merge_time", Blocked),
        ("jira_association_missing", Blocked),
        ("locked_paths", Blocked),
        ("locked_lfs_files", Blocked),
        ("title_regex", Blocked),
        ("commits_status", Blocked),
        ("draft_status", Blocked),
        ("checking", Unknown),
        ("unchecked", Unknown),
        ("preparing", Unknown),
        ("approvals_syncing", Unknown),
        ("not_open", Unknown),
        ("unknown", Unknown),
        ("something_new", Unknown),
    ];
    for (d, want) in detailed {
        assert_eq!(merge_state(Some(d), Some("can_be_merged"), false), want, "{d}");
    }
    let legacy = [
        (Some("can_be_merged"), Clean),
        (Some("cannot_be_merged"), Conflicts),
        (Some("unchecked"), Unknown),
        (Some("checking"), Unknown),
        (Some("cannot_be_merged_recheck"), Unknown),
        (None, Unknown),
    ];
    for (l, want) in legacy {
        assert_eq!(merge_state(None, l, false), want, "{l:?}");
    }
    assert_eq!(merge_state(Some("mergeable"), None, true), Conflicts, "has_conflicts wins");
    assert_eq!(merge_state(None, None, true), Conflicts);
}

#[test]
fn state_mapping() {
    assert_eq!(pr_state("opened"), PrState::Open);
    assert_eq!(pr_state("locked"), PrState::Open, "locked is GitLab merging it");
    assert_eq!(pr_state("closed"), PrState::Closed);
    assert_eq!(pr_state("merged"), PrState::Merged);
}

#[test]
fn methods_follow_the_project() {
    use MergeMethod::*;
    let cases: [(Option<&str>, Option<&str>, &[MergeMethod]); 13] = [
        (Some("merge"), Some("never"), &[Merge]),
        (Some("merge"), Some("always"), &[Squash]),
        (Some("merge"), Some("default_on"), &[Squash, Merge]),
        (Some("merge"), Some("default_off"), &[Merge, Squash]),
        (Some("merge"), None, &[Merge, Squash]),
        (Some("rebase_merge"), Some("never"), &[Merge]),
        (Some("rebase_merge"), Some("default_on"), &[Squash, Merge]),
        (Some("ff"), Some("never"), &[FastForward]),
        (Some("ff"), Some("always"), &[Squash]),
        (Some("ff"), Some("default_on"), &[Squash, FastForward]),
        (Some("ff"), Some("default_off"), &[FastForward, Squash]),
        (Some("ff"), None, &[FastForward, Squash]),
        (None, None, &[Merge, Squash]),
    ];
    for (m, s, want) in cases {
        assert_eq!(methods(m, s), want, "{m:?} {s:?}");
    }
}

#[test]
fn who_can_merge() {
    let access = |level| Some(Access { access_level: level });
    let project = |level| Permissions { project_access: access(level), group_access: None };
    let user = |can| MrUser { can_merge: Some(can) };
    assert_eq!(can_merge(Some(&user(false)), Some(&project(50))), Some(false), "the merge request's word wins");
    assert_eq!(can_merge(Some(&user(true)), None), Some(true));
    assert_eq!(can_merge(None, Some(&project(30))), Some(true));
    assert_eq!(can_merge(None, Some(&project(40))), Some(true));
    assert_eq!(can_merge(None, Some(&project(20))), Some(false));
    let group = Permissions { project_access: access(20), group_access: access(40) };
    assert_eq!(can_merge(Some(&MrUser { can_merge: None }), Some(&group)), Some(true), "the group's access counts");
    assert_eq!(can_merge(None, Some(&Permissions::default())), None);
    assert_eq!(can_merge(None, None), None);
}

#[test]
fn approval_notes() {
    assert_eq!(approval_kind("approved this merge request"), Some(Verdict::Approved));
    assert_eq!(approval_kind("  approved this merge request\n"), Some(Verdict::Approved));
    assert_eq!(approval_kind("unapproved this merge request"), Some(Verdict::ApprovalRemoved));
    assert_eq!(approval_kind("changed the description"), None);
    assert_eq!(approval_kind("approved this merge request twice"), None);
}

#[test]
fn merge_message() {
    let req = |title: &str, message: &str| MergeRequest {
        method: MergeMethod::Merge,
        head_sha: "h".into(),
        title: title.into(),
        message: message.into(),
    };
    assert_eq!(message_of(&req("Title", "Body")), "Title\n\nBody");
    assert_eq!(message_of(&req(" Title ", "")), "Title");
    assert_eq!(message_of(&req("", "Body")), "Body");
    assert_eq!(message_of(&req("", " ")), "");
}
