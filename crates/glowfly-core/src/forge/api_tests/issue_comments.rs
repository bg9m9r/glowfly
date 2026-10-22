//! Posting a comment on an issue on every tracker Glowfly shows issues from: the route, the
//! request body each service takes, the comment it answers with, and its refusals. Drafts that
//! are blank or too long, and keys that can't be an issue, send nothing.

use super::*;

/// When the answers say the comment was posted (2026-01-05T10:00:00Z), in Unix seconds.
const POSTED_AT: i64 = 1_767_607_200;

fn github() -> Target {
    remote("https://github.com/o/r")
}

#[test]
fn github_posts_a_normalised_comment() {
    let m = Mock::start(|r| match r.route().as_str() {
        "POST api.github.com/repos/o/r/issues/9/comments" => Reply::json(
            r#"{"user":{"login":"ada"},"body":"Thanks","created_at":"2026-01-05T10:00:00Z","html_url":"https://github.com/o/r/issues/9#issuecomment-5"}"#,
        ),
        _ => Reply::not_found(),
    });
    let e = post_issue_comment(&github(), &token(Provider::GitHub), "9", "\n\nThanks  \r\n").unwrap();
    let sent = find(&m, "POST api.github.com/repos/o/r/issues/9/comments");
    assert_eq!(sent.json(), serde_json::json!({ "body": "Thanks" }));
    assert_eq!(sent.header("Authorization"), Some("Bearer tok"));
    assert_eq!((e.author.as_str(), e.body.as_str(), e.time, e.kind), ("ada", "Thanks", POSTED_AT, EntryKind::Comment));
    assert_eq!(e.url, "https://github.com/o/r/issues/9#issuecomment-5");
    drop(m);

    let _m = Mock::start(|_| Reply::json(r#"{"user":{"login":"ada"},"body":"Thanks","html_url":"https://evil.example/c/5"}"#));
    let e = post_issue_comment(&github(), &token(Provider::GitHub), "9", "Thanks").unwrap();
    assert_eq!(e.url, "", "a link on another host is blanked");
}

#[test]
fn github_refusals_carry_the_service_words() {
    let auth = token(Provider::GitHub);
    let m = Mock::start(|_| Reply::status(403, r#"{"message":"Unable to create comment because issue is locked."}"#));
    let e = post_issue_comment(&github(), &auth, "9", "Hi").unwrap_err();
    assert!(matches!(&e, Error::Forbidden(msg) if msg.contains("Unable to create comment because issue is locked.")), "{e}");
    drop(m);

    let m = Mock::start(|_| Reply::not_found());
    let e = post_issue_comment(&github(), &auth, "9", "Hi").unwrap_err();
    assert!(e.is_not_found() && e.to_string().contains("Issue #9 wasn't found"), "{e}");
    let before = routes(&m).len();
    assert!(post_issue_comment(&github(), &auth, "../x", "Hi").unwrap_err().is_not_found());
    assert_eq!(routes(&m).len(), before, "a key that isn't a number sends nothing");
    drop(m);

    let m = Mock::start(|_| Reply::json(r#"{"user":{"login":"ada"},"body":"Hi","created_at":"2026-01-05T10:00:00Z","html_url":""}"#));
    let ghes = Target {
        provider: Provider::GitHubEnterprise,
        host: "git.acme.io".into(),
        owner: "o".into(),
        project: String::new(),
        repo: "r".into(),
    };
    post_issue_comment(&ghes, &token(Provider::GitHubEnterprise), "9", "Hi").unwrap();
    assert_eq!(routes(&m), ["POST git.acme.io/api/v3/repos/o/r/issues/9/comments"]);
}

#[test]
fn gitlab_posts_a_note() {
    let m = Mock::start(|r| match r.route().as_str() {
        "POST gitlab.com/api/v4/projects/g%2Fapp/issues/9/notes" => Reply::json(
            r#"{"id":77,"body":"On it","author":{"name":"Ada","username":"ada"},"created_at":"2026-01-05T10:00:00Z","system":false}"#,
        ),
        _ => Reply::not_found(),
    });
    let e = post_issue_comment(&remote("https://gitlab.com/g/app"), &token(Provider::GitLab), "9", "On it").unwrap();
    assert_eq!(find(&m, "POST gitlab.com/api/v4/projects/g%2Fapp/issues/9/notes").json(), serde_json::json!({ "body": "On it" }));
    assert_eq!((e.author.as_str(), e.body.as_str(), e.time), ("Ada", "On it", POSTED_AT));
    assert_eq!(e.url, "https://gitlab.com/g/app/-/issues/9#note_77");
}

#[test]
fn jira_cloud_sends_rich_text_and_data_center_plain() {
    let jira = |provider: Provider, host: &str| Target::jira(&Account { provider, host: host.into(), org: String::new() }, "PROJ");
    let m = Mock::start(|r| match r.route().as_str() {
        "POST acme.atlassian.net/rest/api/3/issue/PROJ-7/comment" => Reply::json(
            r#"{"author":{"displayName":"Ada"},"created":"2026-01-05T10:00:00.000+0000",
            "body":{"type":"doc","version":1,"content":[{"type":"paragraph","content":[{"type":"text","text":"Seen it"}]}]}}"#,
        ),
        "POST jira.acme.io/rest/api/2/issue/PROJ-7/comment" => {
            Reply::json(r#"{"author":{"displayName":"Grace"},"created":"2026-01-05T10:00:00.000+0000","body":"Seen it too"}"#)
        }
        _ => Reply::not_found(),
    });
    let cloud = jira(Provider::JiraCloud, "acme.atlassian.net");
    let e = post_issue_comment(&cloud, &token(Provider::JiraCloud), "PROJ-7", "Seen it\n\nTwice").unwrap();
    let body = find(&m, "POST acme.atlassian.net/rest/api/3/issue/PROJ-7/comment").json();
    assert_eq!(body["body"]["type"], "doc");
    let paragraphs = body["body"]["content"].as_array().unwrap();
    assert_eq!(paragraphs.len(), 2);
    assert!(paragraphs.iter().all(|p| p["type"] == "paragraph"));
    assert_eq!((e.author.as_str(), e.body.as_str(), e.time), ("Ada", "Seen it", POSTED_AT));

    let dc = jira(Provider::JiraDataCenter, "jira.acme.io");
    let e = post_issue_comment(&dc, &token(Provider::JiraDataCenter), "PROJ-7", "Seen it too").unwrap();
    let body = find(&m, "POST jira.acme.io/rest/api/2/issue/PROJ-7/comment").json();
    assert_eq!(body, serde_json::json!({ "body": "Seen it too" }));
    assert_eq!((e.author.as_str(), e.body.as_str()), ("Grace", "Seen it too"));

    let e = post_issue_comment(&cloud, &token(Provider::JiraCloud), "PROJ-8", "Hi").unwrap_err();
    assert!(e.is_not_found() && e.to_string().contains("Issue PROJ-8 wasn't found"), "{e}");
}

#[test]
fn bitbucket_cloud_posts_raw_markdown() {
    let t = remote("git@bitbucket.org:team/app.git");
    let auth = Auth::from_token(Provider::Bitbucket, Some("ada"), "app-pass", "test");
    let m = Mock::start(|r| match r.route().as_str() {
        "POST api.bitbucket.org/2.0/repositories/team/app/issues/3/comments" => Reply::json(
            r#"{"content":{"raw":"**Fixed**"},"user":{"display_name":"Grace"},"created_on":"2026-01-05T10:00:00+00:00",
            "links":{"html":{"href":"https://bitbucket.org/team/app/issues/3#comment-9"}}}"#,
        ),
        _ => Reply::not_found(),
    });
    let e = post_issue_comment(&t, &auth, "3", "**Fixed**").unwrap();
    let sent = find(&m, "POST api.bitbucket.org/2.0/repositories/team/app/issues/3/comments");
    assert_eq!(sent.json(), serde_json::json!({ "content": { "raw": "**Fixed**" } }));
    assert_eq!((e.author.as_str(), e.body.as_str(), e.time), ("Grace", "**Fixed**", POSTED_AT));
    assert_eq!(e.url, "https://bitbucket.org/team/app/issues/3#comment-9");
    drop(m);

    let _m = Mock::start(|_| Reply::status(404, r#"{"type":"error","error":{"message":"Repository has no issue tracker."}}"#));
    let e = post_issue_comment(&t, &auth, "3", "Hi").unwrap_err();
    assert!(matches!(&e, Error::Unsupported(msg) if msg.contains("turned off")), "{e}");
}

#[test]
fn azure_posts_html_and_names_the_scope() {
    let t = remote("https://dev.azure.com/contoso/Fabrikam%20Web/_git/web%20app");
    let route = "POST dev.azure.com/contoso/Fabrikam%20Web/_apis/wit/workItems/5/comments?api-version=7.1-preview.4";
    let m = Mock::start(move |r| {
        if r.route() == route {
            Reply::json(
                r#"{"id":3,"text":"<div>One</div><div><br></div><div>Two &amp; three</div>","createdBy":{"displayName":"Ada"},"createdDate":"2026-01-05T10:00:00Z"}"#,
            )
        } else {
            Reply::not_found()
        }
    });
    let e = post_issue_comment(&t, &token(Provider::AzureDevOps), "5", "One\n\nTwo & three").unwrap();
    assert_eq!(find(&m, route).json(), serde_json::json!({ "text": "<div>One</div><div><br></div><div>Two &amp; three</div>" }));
    assert_eq!((e.author.as_str(), e.time, e.url.as_str()), ("Ada", POSTED_AT, ""));
    assert!(e.body.contains("One") && e.body.contains("Two & three") && !e.body.contains('<'), "{:?}", e.body);
    drop(m);

    let _m = Mock::start(|_| Reply::status(403, r#"{"message":"Access denied."}"#));
    let e = post_issue_comment(&t, &token(Provider::AzureDevOps), "5", "Hi").unwrap_err();
    let hint = "commenting on work items needs a token with the Work Items (Read & write) scope";
    assert!(matches!(&e, Error::Forbidden(msg) if msg.contains(hint)), "{e}");
}

#[test]
fn trello_posts_a_comment_action() {
    let account = Account { provider: Provider::Trello, host: "trello.com".into(), org: String::new() };
    let t = Target::trello(&account, "5f1a2b3c4d5e6f7a8b9c0d1e", "Website");
    let auth = Auth::from_token(Provider::Trello, Some("0123456789abcdef0123456789abcdef"), "tok", "test");
    let m = Mock::start(|r| match r.route().as_str() {
        "POST api.trello.com/1/cards/AbCd1234/actions/comments" => Reply::json(
            r#"{"id":"c7","type":"commentCard","date":"2026-01-05T10:00:00.000Z","data":{"text":"Done"},
            "memberCreator":{"fullName":"Ada","username":"ada"}}"#,
        ),
        _ => Reply::not_found(),
    });
    let e = post_issue_comment(&t, &auth, "AbCd1234", "Done").unwrap();
    assert_eq!(find(&m, "POST api.trello.com/1/cards/AbCd1234/actions/comments").json()["text"], "Done");
    assert_eq!((e.author.as_str(), e.body.as_str(), e.time), ("Ada", "Done", POSTED_AT));
    assert_eq!(e.url, "https://trello.com/c/AbCd1234#comment-c7");
    let before = routes(&m).len();
    assert!(post_issue_comment(&t, &auth, "../x", "Done").unwrap_err().is_not_found());
    let e = post_issue_comment(&t, &auth, "AbCd1234", &"x".repeat(16_385)).unwrap_err();
    assert!(e.to_string().contains("at most 16,384 characters"), "{e}");
    assert_eq!(routes(&m).len(), before, "a key that isn't a short link and an over-long draft send nothing");
    drop(m);

    let _m = Mock::start(|r| match r.route().as_str() {
        "POST api.trello.com/1/cards/AbCd1234/actions/comments" => Reply::status(401, "unauthorized permission requested"),
        "GET api.trello.com/1/members/me?fields=fullName,username" => Reply::json(r#"{"fullName":"Ada"}"#),
        _ => Reply::not_found(),
    });
    let e = post_issue_comment(&t, &auth, "AbCd1234", "Done").unwrap_err();
    assert!(matches!(&e, Error::Forbidden(msg) if msg.contains("can read cards but not comment on them")), "{e}");
}
