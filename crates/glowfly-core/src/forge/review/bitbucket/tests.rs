use super::*;

fn participants(json: &str) -> Vec<ApiParticipant> {
    serde_json::from_str(json).unwrap()
}

#[test]
fn state_mapping() {
    assert_eq!(pr_state("OPEN"), PrState::Open);
    assert_eq!(pr_state("MERGED"), PrState::Merged);
    assert_eq!(pr_state("DECLINED"), PrState::Closed);
    assert_eq!(pr_state("SUPERSEDED"), PrState::Closed);
}

#[test]
fn my_vote_comes_from_my_participant_entry() {
    let list = participants(
        r#"[{"user":{"uuid":"{a}"},"approved":true,"state":null},{"user":{"uuid":"{b}"},"approved":false,"state":"approved"},
        {"user":{"uuid":"{c}"},"approved":false,"state":"changes_requested"},{"user":{"uuid":"{d}"},"approved":false,"state":null}]"#,
    );
    assert_eq!(my_vote(&list, "{a}"), Vote::Approved);
    assert_eq!(my_vote(&list, "{b}"), Vote::Approved, "the state says so");
    assert_eq!(my_vote(&list, "{c}"), Vote::ChangesRequested);
    assert_eq!(my_vote(&list, "{d}"), Vote::Neither);
    assert_eq!(my_vote(&list, "{e}"), Vote::Neither, "another user's vote isn't mine");
    assert_eq!(my_vote(&list, ""), Vote::Neither, "an unknown user has no vote");
    let upper = participants(r#"[{"user":{"uuid":"{ABC-1}"},"approved":true}]"#);
    assert_eq!(my_vote(&upper, "{abc-1}"), Vote::Approved, "uuids without case");
}

fn comments(json: &str) -> Vec<ApiComment> {
    serde_json::from_str(json).unwrap()
}

#[test]
fn general_comments_and_replies_are_kept_code_comments_counted() {
    let list = comments(
        r#"[{"id":1,"content":{"raw":"First"},"user":{"display_name":"Ada"},"created_on":"2026-01-01T10:00:00.123456+00:00",
            "links":{"html":{"href":"https://bitbucket.org/t/a/pull-requests/1/_/diff#comment-1"}}},
        {"id":2,"content":{"raw":"On a line"},"user":{"display_name":"Grace"},"created_on":"2026-01-01T11:00:00Z","inline":{"path":"a.rs","to":3}},
        {"id":3,"content":{"raw":"Reply on the line"},"user":{"display_name":"Ada"},"created_on":"2026-01-01T12:00:00Z","parent":{"id":2}},
        {"id":4,"content":{"raw":"Gone"},"created_on":"2026-01-01T13:00:00Z","deleted":true},
        {"id":5,"content":{"raw":"A reply"},"user":{"display_name":"Linus"},"created_on":"2026-01-01T14:00:00Z","parent":{"id":1}},
        {"id":6,"content":{"raw":""},"created_on":"2026-01-01T15:00:00Z","inline":{"path":"a.rs"},"deleted":true}]"#,
    );
    let (entries, inline) = comment_entries(list);
    assert_eq!(inline, 2, "the code comment and its reply; the deleted one isn't counted");
    let got: Vec<(&str, &str)> = entries.iter().map(|e| (e.author.as_str(), e.body.as_str())).collect();
    assert_eq!(got, [("Ada", "First"), ("Linus", "A reply")]);
    assert_eq!(entries[0].time, 1_767_261_600);
    assert_eq!(entries[0].kind, EntryKind::Comment);
    assert_eq!(entries[0].url, "https://bitbucket.org/t/a/pull-requests/1/_/diff#comment-1");
    assert_eq!(entries[1].url, "");
}

#[test]
fn votes_become_reviews() {
    let activity: Vec<ApiActivity> = serde_json::from_str(
        r#"[{"approval":{"date":"2026-01-02T10:00:00Z","user":{"display_name":"Grace"}}},
        {"comment":{"id":1}},{"update":{"state":"OPEN"}},
        {"changes_requested":{"date":"2026-01-03T10:00:00Z","user":{"display_name":"Linus"}}}]"#,
    )
    .unwrap();
    let page = "https://bitbucket.org/t/a/pull-requests/1";
    let entries = vote_entries(activity, page);
    let got: Vec<(&str, EntryKind, &str, &str)> =
        entries.iter().map(|e| (e.author.as_str(), e.kind, e.body.as_str(), e.url.as_str())).collect();
    assert_eq!(
        got,
        [("Grace", EntryKind::Review(Verdict::Approved), "", page), ("Linus", EntryKind::Review(Verdict::ChangesRequested), "", page),],
        "comment and update activity is left out"
    );
    assert_eq!(entries[0].time, 1_767_348_000);
}

#[test]
fn conflicts_are_merge_conflict_statuses() {
    let stat = |s: &str| DiffStat { status: s.into() };
    assert!(conflicted(&[stat("modified"), stat("merge conflict")]));
    assert!(!conflicted(&[stat("modified"), stat("added"), stat("removed"), stat("renamed")]));
    assert!(!conflicted(&[stat("local deleted")]));
    assert!(!conflicted(&[]));
}
