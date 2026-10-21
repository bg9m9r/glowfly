//! Bitbucket Cloud issues in Glowfly: details with comments, the Create dialog's choices, creating
//! an issue, and a repository whose issue tracker is turned off.

use super::issues::new_issue;
use super::*;
use base64::Engine;

const REPO: &str = "api.bitbucket.org/2.0/repositories/team/app";

fn t() -> Target {
    remote("git@bitbucket.org:team/app.git")
}

fn auth() -> Auth {
    Auth::from_token(Provider::Bitbucket, Some("ada"), "app-pass", "test")
}

const ISSUE: &str = r#"{"id":3,"title":"Crash","state":"open","kind":"bug","priority":"critical",
    "assignee":{"display_name":"Grace","uuid":"{g}"},"links":{"html":{"href":"https://bitbucket.org/team/app/issues/3"}},
    "milestone":{"name":"v1"},"content":{"raw":"It **crashes**."},"reporter":{"display_name":"Ada","uuid":"{a}"},
    "created_on":"2026-01-02T10:00:00.000000+00:00"}"#;

const COMMENTS: &str = r#"{"values":[
    {"content":{"raw":"Seen it"},"user":{"display_name":"Grace"},"created_on":"2026-01-03T10:00:00+00:00",
     "links":{"html":{"href":"https://bitbucket.org/team/app/issues/3#comment-1"}}},
    {"content":{"raw":""},"user":{"display_name":"Grace"},"created_on":"2026-01-03T11:00:00+00:00",
     "links":{"html":{"href":"https://bitbucket.org/team/app/issues/3#comment-2"}}},
    {"content":{"raw":"Elsewhere"},"user":{"display_name":"Bob"},"created_on":"2026-01-04T10:00:00+00:00",
     "links":{"html":{"href":"https://evil.example/c/3"}}}]}"#;

#[test]
fn details_read_the_issue_and_its_comments() {
    let m = Mock::start(|r| match r.route().strip_prefix(&format!("GET {REPO}")).unwrap_or_default() {
        "/issues/3" => Reply::json(ISSUE),
        "/issues/3/comments?sort=created_on&pagelen=100" => Reply::json(COMMENTS),
        _ => Reply::not_found(),
    });
    let d = issue_details(&t(), &auth(), "3").unwrap();
    assert_eq!(routes(&m), [format!("GET {REPO}/issues/3"), format!("GET {REPO}/issues/3/comments?sort=created_on&pagelen=100")]);
    let basic = format!("Basic {}", base64::engine::general_purpose::STANDARD.encode("ada:app-pass"));
    assert!(m.requests().iter().all(|r| r.header("Authorization") == Some(basic.as_str())));
    assert_eq!((d.issue.key.as_str(), d.issue.state.as_str(), d.issue.kind.as_str()), ("3", "open", "bug"));
    assert_eq!(d.issue.url, "https://bitbucket.org/team/app/issues/3");
    assert_eq!((d.body.as_str(), d.author.as_str(), d.created), ("It **crashes**.", "Ada", 1_767_348_000));
    assert_eq!((d.priority.as_deref(), d.assignees.as_slice()), (Some("critical"), &["Grace".to_owned()][..]));
    assert_eq!(d.issue.facets.milestone.as_deref(), Some("v1"));
    let rows: Vec<(&str, &str, &str)> = d.comments.iter().map(|c| (c.author.as_str(), c.body.as_str(), c.url.as_str())).collect();
    assert_eq!(
        rows,
        [("Grace", "Seen it", "https://bitbucket.org/team/app/issues/3#comment-1"), ("Bob", "Elsewhere", "")],
        "a change-only comment is left out; a link elsewhere is blanked"
    );
}

#[test]
fn a_turned_off_tracker_says_so() {
    let m = Mock::start(|r| match r.route().as_str() {
        "GET api.bitbucket.org/2.0/repositories/team/app" => Reply::json(r#"{"has_issues":false}"#),
        _ => Reply::status(404, r#"{"type":"error","error":{"message":"Repository has no issue tracker."}}"#),
    });
    let e = issue_details(&t(), &auth(), "3").unwrap_err();
    assert!(matches!(&e, Error::Unsupported(msg) if msg.contains("turned off")), "{e}");
    let e = issue_create_options(&t(), &auth(), "").unwrap_err();
    assert!(matches!(&e, Error::Unsupported(msg) if msg.contains("Turn it on in the repository's settings")), "{e}");
    assert!(!routes(&m).iter().any(|r| r.contains("/workspaces/")), "no members are read");
    let e = create_issue(&t(), &auth(), &new_issue("Crash")).unwrap_err();
    assert!(matches!(&e, Error::Unsupported(msg) if msg.contains("turned off")), "{e}");
    let before = routes(&m).len();
    assert!(issue_details(&t(), &auth(), "x1").unwrap_err().is_not_found());
    assert_eq!(routes(&m).len(), before, "a key that isn't a number sends nothing");
}

#[test]
fn options_offer_kinds_priorities_and_members() {
    let _m = Mock::start(|r| match r.route().as_str() {
        "GET api.bitbucket.org/2.0/repositories/team/app" => Reply::json(r#"{"has_issues":true}"#),
        "GET api.bitbucket.org/2.0/workspaces/team/members?pagelen=100" => Reply::json(
            r#"{"values":[{"user":{"display_name":"Ada","uuid":"{a}"}},{"user":{"display_name":"","uuid":"{b}"}},
            {"user":{"display_name":"Ada","uuid":"{a}"}}]}"#,
        ),
        _ => Reply::not_found(),
    });
    let o = issue_create_options(&t(), &auth(), "").unwrap();
    let keys = |v: &[crate::forge::filters::Named]| v.iter().map(|n| (n.key.clone(), n.title.clone())).collect::<Vec<_>>();
    let types: Vec<(String, String)> = [("bug", "Bug"), ("enhancement", "Enhancement"), ("proposal", "Proposal"), ("task", "Task")]
        .map(|(k, t)| (k.into(), t.into()))
        .to_vec();
    assert_eq!(keys(&o.types), types);
    assert_eq!(o.priorities.iter().map(|p| p.key.as_str()).collect::<Vec<_>>(), ["trivial", "minor", "major", "critical", "blocker"]);
    assert_eq!((o.default_type.as_deref(), o.default_priority.as_deref()), (Some("bug"), Some("major")));
    assert_eq!(o.people, [Person { id: "{a}".into(), name: "Ada".into() }, Person { id: "{b}".into(), name: "{b}".into() }]);
    assert_eq!((o.max_assignees, o.labels_free, o.labels.len()), (1, false, 0));
}

#[test]
fn forbidden_members_leave_no_people() {
    let _m = Mock::start(|r| match r.route().as_str() {
        "GET api.bitbucket.org/2.0/repositories/team/app" => Reply::json("{}"),
        "GET api.bitbucket.org/2.0/workspaces/team/members?pagelen=100" => Reply::status(403, r#"{"error":{"message":"no"}}"#),
        _ => Reply::not_found(),
    });
    let o = issue_create_options(&t(), &auth(), "").unwrap();
    assert!(o.people.is_empty() && !o.types.is_empty());
}

#[test]
fn create_sends_only_what_was_chosen() {
    let m = Mock::start(|r| match r.route().as_str() {
        "POST api.bitbucket.org/2.0/repositories/team/app/issues" => Reply::json(
            r#"{"id":4,"title":"Crash","state":"new","kind":"bug","links":{"html":{"href":"https://bitbucket.org/team/app/issues/4"}}}"#,
        ),
        _ => Reply::not_found(),
    });
    let new = NewIssue {
        description: "Steps".into(),
        kind: Some("bug".into()),
        priority: Some("major".into()),
        assignees: vec!["{g}".into()],
        ..new_issue("Crash")
    };
    let i = create_issue(&t(), &auth(), &new).unwrap();
    assert_eq!((i.key.as_str(), i.url.as_str()), ("4", "https://bitbucket.org/team/app/issues/4"));
    let body = find(&m, "POST api.bitbucket.org/2.0/repositories/team/app/issues").json();
    assert_eq!(
        body,
        serde_json::json!({ "title": "Crash", "content": { "raw": "Steps" }, "kind": "bug", "priority": "major", "assignee": { "uuid": "{g}" } })
    );
    create_issue(&t(), &auth(), &new_issue("Bare")).unwrap();
    assert_eq!(m.requests()[1].json(), serde_json::json!({ "title": "Bare" }), "empty fields are left out");
}
