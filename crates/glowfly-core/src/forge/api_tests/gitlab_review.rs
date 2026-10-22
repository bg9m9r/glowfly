use super::*;
use serde_json::{Value, json};

const API: &str = "gitlab.acme.io/api/v4";
const PROJECT: &str = "/projects/group%2Fsub%2Fapp";
const NOTES: &str = "/merge_requests/7/notes?sort=asc&order_by=created_at&per_page=100";

fn acme() -> Target {
    let t = remote("https://gitlab.acme.io/group/sub/app.git");
    assert_eq!(t.provider, Provider::GitLabSelfManaged);
    t
}

fn gl() -> Target {
    remote("https://gitlab.com/g/app")
}

/// `base` with `patch`'s fields set over it.
fn patched(mut base: Value, patch: Value) -> Value {
    if let (Some(b), Value::Object(p)) = (base.as_object_mut(), patch) {
        b.extend(p);
    }
    base
}

fn project(patch: Value) -> Value {
    let base = json!({"merge_method":"rebase_merge","squash_option":"default_on","remove_source_branch_after_merge":false,
        "permissions":{"project_access":{"access_level":40},"group_access":null}});
    patched(base, patch)
}

fn mr(patch: Value) -> Value {
    let base = json!({"state":"opened","draft":false,"work_in_progress":true,"discussion_locked":true,
        "detailed_merge_status":"not_approved","merge_status":"can_be_merged","has_conflicts":false,"sha":"h1",
        "diff_refs":{"base_sha":"b1","start_sha":"s1","head_sha":"h1"},"source_project_id":5,"target_project_id":5,
        "author":{"name":"Ada L","username":"ada"},"user":{"can_merge":false},"force_remove_source_branch":true,
        "should_remove_source_branch":null,"web_url":"https://gitlab.acme.io/group/sub/app/-/merge_requests/7"});
    patched(base, patch)
}

fn note(id: u64, system: bool, kind: Option<&str>, body: &str, time: &str) -> Value {
    json!({"id":id,"body":body,"author":{"name":format!("User {id}"),"username":format!("u{id}")},"created_at":time,
        "system":system,"type":kind})
}

fn notes() -> Value {
    json!([
        note(1, true, None, "changed the description", "2026-01-01T09:00:00.000Z"),
        note(2, true, None, "approved this merge request", "2026-01-01T10:00:00.000Z"),
        note(3, false, None, "Looks good", "2026-01-01T11:00:00.000Z"),
        note(4, false, Some("DiffNote"), "On this line", "2026-01-01T11:30:00.000Z"),
        note(5, true, None, " unapproved this merge request ", "2026-01-01T12:00:00.000Z"),
        note(6, false, Some("DiscussionNote"), "A thread", "2026-01-01T13:00:00.000Z"),
    ])
}

struct Answers {
    project: Value,
    mr: Value,
    approvals: Reply,
    notes: Value,
}

fn answers() -> Answers {
    Answers {
        project: project(json!({})),
        mr: mr(json!({})),
        approvals: Reply::json(r#"{"user_has_approved":true,"user_can_approve":false,"approved":true,"approved_by":[]}"#),
        notes: notes(),
    }
}

fn server(a: Answers) -> Mock {
    let Answers { project, mr, approvals, notes } = a;
    let approvals = std::sync::Mutex::new(Some(approvals));
    Mock::start(move |r| {
        let route = r.route();
        let Some(path) = route.strip_prefix(&format!("GET {API}")) else { return Reply::not_found() };
        if path == "/user" {
            return Reply::json(r#"{"username":"me","name":"Me"}"#);
        }
        let Some(path) = path.strip_prefix(PROJECT) else { return Reply::not_found() };
        match path {
            "" => Reply::json(project.to_string()),
            "/merge_requests/7" => Reply::json(mr.to_string()),
            "/merge_requests/7/approvals" => approvals.lock().unwrap().take().unwrap_or_else(Reply::not_found),
            p if p == NOTES => Reply::json(notes.to_string()),
            _ => Reply::not_found(),
        }
    })
}

fn page_of(a: Answers) -> PrPage {
    let _m = server(a);
    pr_page(&acme(), &token(Provider::GitLabSelfManaged), 7).unwrap()
}

#[test]
fn pr_page_reads_the_merge_request_its_approvals_and_notes() {
    let m = server(answers());
    let page = pr_page(&acme(), &token(Provider::GitLabSelfManaged), 7).unwrap();
    let want: Vec<String> = [
        "/user",
        PROJECT,
        &format!("{PROJECT}/merge_requests/7"),
        &format!("{PROJECT}/merge_requests/7/approvals"),
        &format!("{PROJECT}{NOTES}"),
    ]
    .iter()
    .map(|p| format!("GET {API}{p}"))
    .collect();
    assert_eq!(routes(&m), want);
    assert!(m.requests().iter().all(|r| r.header("Authorization") == Some("Bearer tok")));
    let d = &page.detail;
    assert_eq!((d.state, d.draft, d.locked, d.merge_state, d.mergeable), (PrState::Open, true, true, MergeState::Blocked, None));
    assert_eq!((d.head_sha.as_str(), d.base_sha.as_str(), d.author.as_str(), d.me.as_str()), ("h1", "b1", "ada", "me"));
    assert!(d.same_repo);
    assert_eq!(d.can_merge, Some(false), "the merge request's can_merge wins over Maintainer access");
    assert_eq!(d.methods, [MergeMethod::Squash, MergeMethod::Merge]);
    assert!(d.deletes_merged_branches);
    assert_eq!(d.style, ReviewStyle::Approvals { by_me: true, can_approve: Some(false) });
    assert_eq!(d.pending_review, None);
    let summary: Vec<(&str, EntryKind, &str)> = page.entries.iter().map(|e| (e.author.as_str(), e.kind, e.body.as_str())).collect();
    assert_eq!(
        summary,
        [
            ("User 2", EntryKind::Review(Verdict::Approved), ""),
            ("User 3", EntryKind::Comment, "Looks good"),
            ("User 5", EntryKind::Review(Verdict::ApprovalRemoved), ""),
            ("User 6", EntryKind::Comment, "A thread"),
        ],
        "oldest first, without other system notes or code comments"
    );
    assert_eq!(page.entries[1].url, "https://gitlab.acme.io/group/sub/app/-/merge_requests/7#note_3");
    assert!(page.threads.is_empty());
    let n = page.threads_note.as_deref().unwrap_or_default();
    assert!(n.contains("open in the browser") && n.contains("(1 on this merge request)"), "{n}");
}

#[test]
fn a_page_on_another_host_leaves_note_links_empty() {
    let page = page_of(Answers { mr: mr(json!({"web_url":"https://evil.example/group/sub/app/-/merge_requests/7"})), ..answers() });
    assert!(page.entries.iter().all(|e| e.url.is_empty()), "{:?}", page.entries);
}

#[test]
fn notes_are_read_through_next_links() {
    let next = format!(r#"<https://{API}{PROJECT}{NOTES}&page=2>; rel="next""#);
    let m = Mock::start(move |r| {
        let route = r.route();
        if route.ends_with("/user") {
            Reply::json(r#"{"username":"me"}"#)
        } else if route.contains("/notes?") {
            if route.ends_with("&page=2") {
                Reply::json(json!([note(2, false, None, "second", "2026-01-02T00:00:00Z")]).to_string())
            } else {
                Reply::json(json!([note(1, false, None, "first", "2026-01-01T00:00:00Z")]).to_string()).header("Link", &next)
            }
        } else if route.ends_with("/merge_requests/7") {
            Reply::json(mr(json!({})).to_string())
        } else if route.ends_with("/approvals") {
            Reply::json("{}")
        } else {
            Reply::json(project(json!({})).to_string())
        }
    });
    let page = pr_page(&acme(), &token(Provider::GitLabSelfManaged), 7).unwrap();
    let bodies: Vec<&str> = page.entries.iter().map(|e| e.body.as_str()).collect();
    assert_eq!(bodies, ["first", "second"]);
    assert_eq!(page.threads_note, None, "no code comments, no note");
    assert_eq!(routes(&m).iter().filter(|r| r.contains("/notes?")).count(), 2);
}

#[test]
fn missing_approvals_still_load_the_page() {
    let page = page_of(Answers { approvals: Reply::not_found(), ..answers() });
    assert_eq!(page.detail.style, ReviewStyle::Approvals { by_me: false, can_approve: None });
    let _m = server(Answers { approvals: Reply::status(500, r#"{"message":"boom"}"#), ..answers() });
    let e = pr_page(&acme(), &token(Provider::GitLabSelfManaged), 7).unwrap_err();
    assert!(matches!(&e, Error::Http { status: 500, .. }), "{e}");
}

#[test]
fn approved_by_me_is_found_in_the_list_when_not_said() {
    let approvals = Reply::json(r#"{"approved_by":[{"user":{"username":"ME","name":"Me"}}]}"#);
    let page = page_of(Answers { approvals, ..answers() });
    assert_eq!(page.detail.style, ReviewStyle::Approvals { by_me: true, can_approve: None });
}

#[test]
fn project_and_merge_request_variants() {
    let d = page_of(Answers { mr: mr(json!({"source_project_id":9})), ..answers() }).detail;
    assert!(!d.same_repo, "a fork");
    let d = page_of(Answers { project: project(json!({"merge_method":"ff","squash_option":"never"})), ..answers() }).detail;
    assert_eq!(d.methods, [MergeMethod::FastForward]);
    let d = page_of(Answers { project: json!({"merge_method":"merge"}), ..answers() }).detail;
    assert_eq!(d.methods, [MergeMethod::Merge, MergeMethod::Squash], "an old server without squash_option");
    let access = |level: u32| project(json!({"permissions":{"project_access":{"access_level":level},"group_access":null}}));
    let d = page_of(Answers { project: access(30), mr: mr(json!({"user":null})), ..answers() }).detail;
    assert_eq!(d.can_merge, Some(true));
    let d = page_of(Answers { project: access(20), mr: mr(json!({"user":null})), ..answers() }).detail;
    assert_eq!(d.can_merge, Some(false));
    let d = page_of(Answers { mr: mr(json!({"has_conflicts":true,"detailed_merge_status":"mergeable"})), ..answers() }).detail;
    assert_eq!((d.merge_state, d.mergeable), (MergeState::Conflicts, Some(false)));
    let d = page_of(Answers { mr: mr(json!({"detailed_merge_status":null,"merge_status":"cannot_be_merged"})), ..answers() }).detail;
    assert_eq!(d.merge_state, MergeState::Conflicts);
    let flags = json!({"force_remove_source_branch":null,"should_remove_source_branch":null});
    let d = page_of(Answers { project: project(json!({"remove_source_branch_after_merge":true})), mr: mr(flags), ..answers() }).detail;
    assert!(d.deletes_merged_branches, "the project's default when the merge request says nothing");
    let flags = json!({"force_remove_source_branch":false,"should_remove_source_branch":null});
    let d = page_of(Answers { project: project(json!({"remove_source_branch_after_merge":true})), mr: mr(flags), ..answers() }).detail;
    assert!(!d.deletes_merged_branches, "the merge request's own flag wins");
}

#[test]
fn post_comment_posts_a_note() {
    let m = Mock::start(|r| match r.route().as_str() {
        "POST gitlab.com/api/v4/projects/g%2Fapp/merge_requests/7/notes" => {
            Reply::status(201, note(41, false, None, "Thanks!", "2026-01-02T03:04:05.000Z").to_string())
        }
        _ => Reply::not_found(),
    });
    let e = post_comment(&gl(), &token(Provider::GitLab), 7, "Thanks!").unwrap();
    assert_eq!(find(&m, "POST").json(), json!({ "body": "Thanks!" }));
    assert_eq!((e.author.as_str(), e.body.as_str(), e.kind, e.time), ("User 41", "Thanks!", EntryKind::Comment, 1_767_323_045));
    assert_eq!(e.url, "https://gitlab.com/g/app/-/merge_requests/7#note_41");
}

fn review(event: ReviewEvent, body: &str) -> ReviewRequest {
    ReviewRequest { event, body: body.into(), pending: None, head_sha: "h1".into() }
}

const MR_API: &str = "gitlab.com/api/v4/projects/g%2Fapp/merge_requests/7";

/// A server that answers `failing` (a route suffix after the merge request) with `status`.
fn review_server(failing: &'static str, status: u16) -> Mock {
    Mock::start(move |r| {
        let route = r.route();
        let Some(rest) = route.strip_prefix("POST ").and_then(|p| p.strip_prefix(MR_API)) else { return Reply::not_found() };
        if rest == failing {
            return Reply::status(status, r#"{"message":"refused"}"#);
        }
        match rest {
            "/notes" => Reply::status(201, note(9, false, None, "x", "2026-01-01T00:00:00Z").to_string()),
            "/approve" | "/unapprove" => Reply::status(201, "{}"),
            _ => Reply::not_found(),
        }
    })
}

fn sent(m: &Mock) -> Vec<(String, Value)> {
    m.requests().iter().map(|r| (r.route(), r.json())).collect()
}

#[test]
fn submit_review_comments_approves_and_takes_back() {
    let (t, auth) = (gl(), token(Provider::GitLab));
    let m = review_server("", 0);
    submit_review(&t, &auth, 7, &review(ReviewEvent::Comment, "Nice")).unwrap();
    submit_review(&t, &auth, 7, &review(ReviewEvent::Approve, "")).unwrap();
    submit_review(&t, &auth, 7, &review(ReviewEvent::Approve, "Ship it")).unwrap();
    submit_review(&t, &auth, 7, &review(ReviewEvent::Unapprove, "")).unwrap();
    let e = submit_review(&t, &auth, 7, &review(ReviewEvent::RequestChanges, "No")).unwrap_err();
    assert!(matches!(&e, Error::Unsupported(_)), "{e}");
    let post = |path: &str, body: Value| (format!("POST {MR_API}{path}"), body);
    assert_eq!(
        sent(&m),
        [
            post("/notes", json!({"body":"Nice"})),
            post("/approve", json!({"sha":"h1"})),
            post("/approve", json!({"sha":"h1"})),
            post("/notes", json!({"body":"Ship it"})),
            post("/unapprove", json!({})),
        ]
    );
}

#[test]
fn approval_refusals_read_plainly() {
    let (t, auth) = (gl(), token(Provider::GitLab));
    let call = |failing, status, ev, body: &str| {
        let _m = review_server(failing, status);
        submit_review(&t, &auth, 7, &review(ev, body)).unwrap_err()
    };
    let e = call("/approve", 409, ReviewEvent::Approve, "");
    assert!(matches!(&e, Error::Other(m) if m.starts_with("The source branch changed on the service")), "{e}");
    let e = call("/approve", 401, ReviewEvent::Approve, "");
    assert!(matches!(&e, Error::Forbidden(_)) && !e.is_auth(), "{e}");
    let e = call("/unapprove", 404, ReviewEvent::Unapprove, "");
    assert!(matches!(&e, Error::Other(m) if m.starts_with("You haven't approved")), "{e}");
    let e = call("/unapprove", 401, ReviewEvent::Unapprove, "");
    assert!(!e.is_auth(), "{e}");
    let e = call("/notes", 500, ReviewEvent::Approve, "Ship it");
    assert!(matches!(&e, Error::Other(m) if m.starts_with("Approved, but the summary")), "{e}");
    let e = call("/notes", 500, ReviewEvent::Unapprove, "Oops");
    assert!(matches!(&e, Error::Other(m) if m.starts_with("Removed your approval, but the summary")), "{e}");
}

fn merge_req(method: MergeMethod, title: &str, message: &str) -> MergeRequest {
    MergeRequest { method, head_sha: "h1".into(), title: title.into(), message: message.into() }
}

#[test]
fn merge_sends_sha_squash_and_message() {
    let m = Mock::start(|r| match r.route().as_str() {
        "PUT gitlab.com/api/v4/projects/g%2Fapp/merge_requests/7/merge" => {
            Reply::json(r#"{"state":"merged","sha":"h1","merge_commit_sha":"m1","squash_commit_sha":null}"#)
        }
        _ => Reply::not_found(),
    });
    let (t, auth) = (gl(), token(Provider::GitLab));
    assert_eq!(merge(&t, &auth, 7, &merge_req(MergeMethod::Merge, "Title", "Body")).unwrap().sha, "m1");
    merge(&t, &auth, 7, &merge_req(MergeMethod::Squash, "Title", "")).unwrap();
    merge(&t, &auth, 7, &merge_req(MergeMethod::FastForward, "Ignored", "Ignored too")).unwrap();
    merge(&t, &auth, 7, &merge_req(MergeMethod::Merge, "", "")).unwrap();
    let e = merge(&t, &auth, 7, &merge_req(MergeMethod::Rebase, "", "")).unwrap_err();
    assert!(matches!(&e, Error::Unsupported(_)), "{e}");
    let bodies: Vec<Value> = m.requests().iter().map(|r| r.json()).collect();
    assert_eq!(
        bodies,
        [
            json!({"sha":"h1","squash":false,"merge_commit_message":"Title\n\nBody"}),
            json!({"sha":"h1","squash":true,"squash_commit_message":"Title"}),
            json!({"sha":"h1","squash":false}),
            json!({"sha":"h1","squash":false}),
        ]
    );
}

#[test]
fn merge_answers() {
    let (t, auth) = (gl(), token(Provider::GitLab));
    let answer = |body: &'static str| {
        let _m = Mock::start(move |_| Reply::json(body));
        merge(&t, &auth, 7, &merge_req(MergeMethod::FastForward, "", ""))
    };
    let done = answer(r#"{"state":"merged","sha":"h1","merge_commit_sha":null,"squash_commit_sha":null}"#).unwrap();
    assert_eq!(done.sha, "h1", "a fast-forward has no merge commit");
    let done = answer(r#"{"state":"merged","sha":"h1","merge_commit_sha":null,"squash_commit_sha":"s1"}"#).unwrap();
    assert_eq!(done.sha, "s1");
    let e = answer(r#"{"state":"opened","merge_error":"Pipeline failed"}"#).unwrap_err();
    assert!(matches!(&e, Error::Other(m) if m == "Pipeline failed"), "{e}");
    let e = answer(r#"{"state":"opened"}"#).unwrap_err();
    assert!(matches!(&e, Error::Other(m) if m.starts_with("GitLab didn't merge it")), "{e}");
}

#[test]
fn merge_refusals_read_plainly() {
    let call = || merge(&gl(), &token(Provider::GitLab), 7, &merge_req(MergeMethod::Merge, "", "")).map(drop);
    let e = error_for(405, r#"{"message":"405 Method Not Allowed"}"#, call);
    assert!(matches!(&e, Error::Other(m) if m.starts_with("GitLab can't merge it yet")), "{e}");
    let e = error_for(409, r#"{"message":"SHA does not match HEAD of source branch"}"#, call);
    assert!(matches!(&e, Error::Other(m) if m.contains("source branch changed")), "{e}");
    let e = error_for(401, r#"{"message":"401 Unauthorized"}"#, call);
    assert!(matches!(&e, Error::Forbidden(_)) && !e.is_auth(), "{e}");
    let e = error_for(422, r#"{"message":"Branch cannot be merged"}"#, call);
    assert!(matches!(&e, Error::Http { status: 422, message } if message.contains("Branch cannot be merged")), "{e}");
}
