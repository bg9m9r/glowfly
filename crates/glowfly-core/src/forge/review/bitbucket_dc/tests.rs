use super::*;

fn pr(json: &str) -> DcPr {
    serde_json::from_str(json).unwrap()
}

#[test]
fn state_mapping() {
    assert_eq!(pr_state("OPEN"), PrState::Open);
    assert_eq!(pr_state("MERGED"), PrState::Merged);
    assert_eq!(pr_state("DECLINED"), PrState::Closed);
}

#[test]
fn my_vote_and_slug_come_from_the_people_on_it() {
    let p = pr(r#"{"author":{"user":{"name":"ada","slug":"ada"},"status":"UNAPPROVED"},
        "reviewers":[{"user":{"name":"Grace.H","slug":"grace.h"},"status":"APPROVED"},{"user":{"name":"linus","slug":"linus"},"status":"NEEDS_WORK"}],
        "participants":[{"user":{"name":"bob","slug":"bob"},"status":"UNAPPROVED"}]}"#);
    assert_eq!(my_vote(&p, "grace.h"), Vote::Approved, "names without case");
    assert_eq!(my_vote(&p, "linus"), Vote::ChangesRequested);
    assert_eq!(my_vote(&p, "bob"), Vote::Neither);
    assert_eq!(my_vote(&p, "ada"), Vote::Neither, "the author doesn't vote");
    assert_eq!(my_vote(&p, ""), Vote::Neither);
    assert_eq!(my_slug(&p, "GRACE.H").as_deref(), Some("grace.h"));
    assert_eq!(my_slug(&p, "ada").as_deref(), Some("ada"), "the author is found too");
    assert_eq!(my_slug(&p, "bob").as_deref(), Some("bob"));
    assert_eq!(my_slug(&p, "eve"), None);
    let odd = pr(r#"{"participants":[{"user":{"name":"Ada.L","slug":"ada.l"},"status":"NEEDS_WORK"}]}"#);
    assert_eq!((my_vote(&odd, "Ada.L"), my_slug(&odd, "Ada.L").as_deref()), (Vote::ChangesRequested, Some("ada.l")));
}

#[test]
fn merge_check_mapping() {
    let check = |json: &str| merge_state(&serde_json::from_str(json).unwrap());
    assert_eq!(check(r#"{"canMerge":false,"conflicted":true,"outcome":"CONFLICTED","vetoes":[]}"#), MergeState::Conflicts);
    assert_eq!(check(r#"{"canMerge":false,"conflicted":false,"outcome":"CONFLICTED"}"#), MergeState::Conflicts);
    assert_eq!(check(r#"{"canMerge":true,"conflicted":false,"outcome":"CLEAN","vetoes":[]}"#), MergeState::Clean);
    assert_eq!(check(r#"{"canMerge":false,"outcome":"CLEAN","vetoes":[{"summaryMessage":"Needs 2 approvals"}]}"#), MergeState::Blocked);
    assert_eq!(check(r#"{"canMerge":false,"outcome":"UNKNOWN","vetoes":[]}"#), MergeState::Unknown);
}

fn activity(json: &str) -> Vec<Activity> {
    serde_json::from_str(json).unwrap()
}

const PAGE: &str = "https://bitbucket.acme.io/projects/PROJ/repos/demo/pull-requests/3";

#[test]
fn entries_flatten_replies_and_leave_out_code_and_deleted_comments() {
    // Newest first, as the server sends them.
    let list = activity(
        r#"[
        {"action":"REVIEWED","createdDate":1767400000000,"user":{"displayName":"Linus"}},
        {"action":"COMMENTED","commentAction":"DELETED","createdDate":1767390000000,"user":{"displayName":"Ada"},"comment":{"id":13,"text":"oops"}},
        {"action":"UNAPPROVED","createdDate":1767380000000,"user":{"displayName":"Grace"}},
        {"action":"COMMENTED","commentAction":"REPLIED","createdDate":1767370000000,"user":{"displayName":"Linus"},
            "comment":{"id":11,"text":"Agreed","author":{"displayName":"Linus"},"createdDate":1767370000000}},
        {"action":"COMMENTED","commentAction":"EDITED","createdDate":1767365000000,"user":{"displayName":"Ada"},
            "comment":{"id":10,"text":"First, edited","author":{"displayName":"Ada"},"createdDate":1767300000000}},
        {"action":"COMMENTED","commentAction":"ADDED","createdDate":1767360000000,"user":{"displayName":"Grace"},"commentAnchor":{"path":"a.rs","line":3},
            "comment":{"id":20,"text":"On a line","author":{"displayName":"Grace"},"createdDate":1767360000000}},
        {"action":"APPROVED","createdDate":1767350000000,"user":{"displayName":"Grace"}},
        {"action":"COMMENTED","commentAction":"ADDED","createdDate":1767300000000,"user":{"displayName":"Ada"},
            "comment":{"id":10,"text":"First, edited","author":{"displayName":"Ada"},"createdDate":1767300000000,"comments":[
                {"id":11,"text":"Agreed","author":{"displayName":"Linus"},"createdDate":1767370000000,"comments":[
                    {"id":12,"text":"Thanks","author":{"displayName":"Ada"},"createdDate":1767375000000}]},
                {"id":13,"text":"oops","author":{"displayName":"Ada"},"createdDate":1767376000000}]}},
        {"action":"OPENED","createdDate":1767200000000,"user":{"displayName":"Ada"}}
    ]"#,
    );
    let (mut got, code) = entries(list, PAGE);
    got.sort_by_key(|e| e.time);
    assert_eq!(code, 1, "the anchored comment");
    let summary: Vec<(&str, EntryKind, &str, i64)> = got.iter().map(|e| (e.author.as_str(), e.kind, e.body.as_str(), e.time)).collect();
    assert_eq!(
        summary,
        [
            ("Ada", EntryKind::Comment, "First, edited", 1_767_300_000),
            ("Grace", EntryKind::Review(Verdict::Approved), "", 1_767_350_000),
            ("Linus", EntryKind::Comment, "Agreed", 1_767_370_000),
            ("Ada", EntryKind::Comment, "Thanks", 1_767_375_000),
            ("Grace", EntryKind::Review(Verdict::ApprovalRemoved), "", 1_767_380_000),
            ("Linus", EntryKind::Review(Verdict::ChangesRequested), "", 1_767_400_000),
        ],
        "two levels of replies, no duplicates, no deleted or code comments"
    );
    assert_eq!(got[0].url, format!("{PAGE}/overview?commentId=10"));
    assert_eq!(got[1].url, format!("{PAGE}/overview"));
}

#[test]
fn personal_repositories_link_under_users() {
    let t = |owner: &str| Target {
        provider: super::super::super::Provider::BitbucketDataCenter,
        host: "bitbucket.acme.io".into(),
        owner: owner.into(),
        project: String::new(),
        repo: "demo".into(),
    };
    assert_eq!(pr_web(&t("~ada"), 3), "https://bitbucket.acme.io/users/ada/repos/demo/pull-requests/3");
    assert_eq!(pr_web(&t("PROJ"), 3), PAGE);
}
