use super::bitbucket_review::{API, FULL, PR, basic, bb, pr};
use super::*;
use serde_json::{Value, json};
use std::sync::Mutex;

const TASK: &str = "https://api.bitbucket.org/2.0/repositories/team/app/pullrequests/12/merge/task-status/t1";

fn merge_req(method: MergeMethod, title: &str, message: &str) -> MergeRequest {
    MergeRequest { method, head_sha: FULL.into(), title: title.into(), message: message.into() }
}

fn merged() -> String {
    pr(json!({"state":"MERGED","merge_commit":{"hash":"m1"}})).to_string()
}

/// A server whose fresh pull request is `fresh` and whose merge answers `merge`, then task
/// polls answer `tasks` in turn (the last one again once they run out).
fn merge_server(fresh: Value, merge: impl Fn() -> Reply + Send + Sync + 'static, tasks: Vec<String>) -> Mock {
    let tasks = Mutex::new(tasks);
    Mock::start(move |r| match r.route().as_str() {
        route if route == format!("GET {API}{PR}") => Reply::json(fresh.to_string()),
        route if route == format!("POST {API}{PR}/merge") => merge(),
        route if route == format!("GET {API}{PR}/merge/task-status/t1") => {
            let mut tasks = tasks.lock().unwrap();
            let next = if tasks.len() > 1 { tasks.remove(0) } else { tasks.first().cloned().unwrap_or_default() };
            Reply::json(next)
        }
        _ => Reply::not_found(),
    })
}

fn posted(m: &Mock) -> Vec<Value> {
    m.requests().iter().filter(|r| r.method == "POST").map(|r| r.json()).collect()
}

#[test]
fn merge_checks_the_head_then_sends_the_strategy() {
    let m = merge_server(pr(json!({})), || Reply::json(merged()), Vec::new());
    let (t, auth) = (bb(), basic());
    assert_eq!(merge(&t, &auth, 12, &merge_req(MergeMethod::Merge, "Title", "Body")).unwrap().sha, "m1");
    assert_eq!(routes(&m)[..2], [format!("GET {API}{PR}"), format!("POST {API}{PR}/merge")], "the pull request is read first");
    merge(&t, &auth, 12, &merge_req(MergeMethod::Squash, "Title", "")).unwrap();
    merge(&t, &auth, 12, &merge_req(MergeMethod::FastForward, "Ignored", "Ignored too")).unwrap();
    merge(&t, &auth, 12, &merge_req(MergeMethod::Rebase, "Ignored", "")).unwrap();
    merge(&t, &auth, 12, &merge_req(MergeMethod::Merge, "", "")).unwrap();
    assert_eq!(
        posted(&m),
        [
            json!({"type":"pullrequest","merge_strategy":"merge_commit","message":"Title\n\nBody"}),
            json!({"type":"pullrequest","merge_strategy":"squash","message":"Title"}),
            json!({"type":"pullrequest","merge_strategy":"fast_forward"}),
            json!({"type":"pullrequest","merge_strategy":"rebase_fast_forward"}),
            json!({"type":"pullrequest","merge_strategy":"merge_commit"}),
        ]
    );
}

#[test]
fn merge_refuses_a_moved_head_or_a_closed_pr() {
    let moved = pr(json!({"source":{"branch":{"name":"feature"},"commit":{"hash":"aaaaaaaaaaaa"}}}));
    let m = merge_server(moved, || Reply::json(merged()), Vec::new());
    let e = merge(&bb(), &basic(), 12, &merge_req(MergeMethod::Merge, "", "")).unwrap_err();
    assert!(matches!(&e, Error::Other(m) if m.starts_with("The source branch changed")), "{e}");
    assert!(posted(&m).is_empty());
    drop(m);
    let m = merge_server(pr(json!({"state":"DECLINED"})), || Reply::json(merged()), Vec::new());
    let e = merge(&bb(), &basic(), 12, &merge_req(MergeMethod::Merge, "", "")).unwrap_err();
    assert!(matches!(&e, Error::Other(m) if m.contains("isn't open any more")), "{e}");
    assert!(posted(&m).is_empty());
}

fn task(status: &str, result: Option<String>) -> String {
    let result: Value = result.map_or(Value::Null, |r| serde_json::from_str(&r).unwrap());
    json!({ "task_status": status, "merge_result": result }).to_string()
}

fn background() -> Reply {
    Reply::status(202, "").header("Location", TASK)
}

#[test]
fn a_background_merge_is_polled() {
    let (t, auth) = (bb(), basic());
    let req = merge_req(MergeMethod::Squash, "", "");
    let polls = |m: &Mock| routes(m).iter().filter(|r| r.contains("/task-status/")).count();

    let m = merge_server(pr(json!({})), background, vec![task("PENDING", None), task("SUCCESS", Some(merged()))]);
    assert_eq!(merge(&t, &auth, 12, &req).unwrap().sha, "m1");
    assert_eq!(polls(&m), 2);
    drop(m);

    let m = merge_server(pr(json!({})), background, vec![task("PENDING", None)]);
    let e = merge(&t, &auth, 12, &req).unwrap_err();
    assert!(matches!(&e, Error::Other(m) if m.starts_with("Bitbucket is still merging it")), "{e}");
    assert_eq!(polls(&m), 10);
    drop(m);

    let elsewhere = || Reply::status(202, "").header("Location", "https://evil.example/task-status/t1");
    let m = merge_server(pr(json!({})), elsewhere, vec![task("SUCCESS", Some(merged()))]);
    let e = merge(&t, &auth, 12, &req).unwrap_err();
    assert!(matches!(&e, Error::Other(m) if m.contains("still merging")), "{e}");
    assert_eq!(routes(&m).len(), 2, "no poll on another host: {:?}", routes(&m));
    drop(m);

    let m = merge_server(pr(json!({})), background, vec![task("FAILED", None)]);
    let e = merge(&t, &auth, 12, &req).unwrap_err();
    assert!(matches!(&e, Error::Other(m) if m.starts_with("Bitbucket didn't merge it")), "{e}");
    assert_eq!(polls(&m), 1);
}

#[test]
fn merge_answers() {
    let (t, auth) = (bb(), basic());
    let req = merge_req(MergeMethod::Merge, "", "");
    let _m = merge_server(pr(json!({})), || Reply::json(pr(json!({})).to_string()), Vec::new());
    let e = merge(&t, &auth, 12, &req).unwrap_err();
    assert!(matches!(&e, Error::Other(m) if m.starts_with("Bitbucket didn't merge it")), "still open: {e}");
    let _m = merge_server(pr(json!({})), || Reply::status(555, r#"{"type":"error","error":{"message":"timeout"}}"#), Vec::new());
    let e = merge(&t, &auth, 12, &req).unwrap_err();
    assert!(matches!(&e, Error::Other(m) if m.starts_with("Bitbucket took too long")), "{e}");
    let failed = || Reply::status(400, r#"{"type":"error","error":{"message":"Merge checks failed"}}"#);
    let _m = merge_server(pr(json!({})), failed, Vec::new());
    let e = merge(&t, &auth, 12, &req).unwrap_err();
    assert!(matches!(&e, Error::Http { status: 400, message } if message.contains("Merge checks failed")), "{e}");
    let _m = merge_server(pr(json!({})), || Reply::status(401, r#"{"type":"error","error":{"message":"bad"}}"#), Vec::new());
    assert!(merge(&t, &auth, 12, &req).unwrap_err().is_auth(), "Cloud's 401 is a sign-in problem");
}
