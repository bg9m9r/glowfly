//! Issue details, Create issue options and creating issues on Jira Cloud (rich text, the epic
//! as parent) and Jira Data Center (plain text, the Epic Link field).

use super::*;

const CLOUD: &str = "acme.atlassian.net";
const DC: &str = "jira.acme.io";

fn jira(provider: Provider, host: &str, project: &str) -> Target {
    Target::jira(&Account { provider, host: host.into(), org: String::new() }, project)
}

fn new_issue() -> NewIssue {
    NewIssue {
        project: "PROJ".into(),
        title: "Login fails".into(),
        description: "Steps\nhere\n\nThen this".into(),
        labels: vec!["ui".into()],
        assignees: vec!["acc-1".into()],
        kind: Some("10001".into()),
        priority: None,
        epic: Some("PROJ-1".into()),
    }
}

fn path_of(r: &Request, host: &str) -> String {
    r.route().strip_prefix(&format!("{} {host}", r.method)).unwrap_or_default().to_owned()
}

#[test]
fn cloud_details_convert_rich_text_and_page_comments() {
    let m = Mock::start(|r| {
        let path = path_of(r, CLOUD);
        if path
            == "/rest/api/3/issue/PROJ-7?fields=summary,status,issuetype,assignee,labels,fixVersions,description,reporter,created,parent"
        {
            return Reply::json(
                r#"{"key":"PROJ-7","fields":{"summary":"Login","status":{"name":"In Progress"},"issuetype":{"name":"Story"},
                "assignee":{"displayName":"Ada"},"labels":["ui"],"reporter":{"displayName":"Grace"},"created":"2026-01-02T10:00:00.000+0000",
                "description":{"type":"doc","version":1,"content":[{"type":"paragraph","content":[{"type":"text","text":"Users can't sign in"}]}]},
                "parent":{"key":"PROJ-1","fields":{"summary":"Accounts","issuetype":{"name":"Epic"}}}}}"#,
            );
        }
        let comment = |text: &str| {
            format!(
                r#"{{"author":{{"displayName":"Ada"}},"created":"2026-01-03T10:00:00.000+0000","body":{{"type":"doc","version":1,"content":[{{"type":"paragraph","content":[{{"type":"text","text":"{text}"}}]}}]}}}}"#
            )
        };
        match path.as_str() {
            "/rest/api/3/issue/PROJ-7/comment?orderBy=created&maxResults=100" => {
                Reply::json(format!(r#"{{"startAt":0,"total":2,"comments":[{}]}}"#, comment("first")))
            }
            "/rest/api/3/issue/PROJ-7/comment?orderBy=created&maxResults=100&startAt=1" => {
                Reply::json(format!(r#"{{"startAt":1,"total":2,"comments":[{}]}}"#, comment("second")))
            }
            _ => Reply::not_found(),
        }
    });
    let d = issue_details(&jira(Provider::JiraCloud, CLOUD, ""), &token(Provider::JiraCloud), "PROJ-7").unwrap();
    assert_eq!((d.issue.state.as_str(), d.issue.kind.as_str()), ("In Progress", "Story"));
    assert_eq!(d.issue.url, "https://acme.atlassian.net/browse/PROJ-7");
    assert_eq!((d.body.as_str(), d.author.as_str(), d.created), ("Users can't sign in", "Grace", 1_767_348_000));
    assert_eq!(d.assignees, ["Ada"]);
    assert_eq!(d.epic.as_deref(), Some("PROJ-1  Accounts"));
    assert_eq!(d.comments.iter().map(|c| c.body.as_str()).collect::<Vec<_>>(), ["first", "second"]);
    assert_eq!(routes(&m).len(), 3, "no field list on Cloud");
}

#[test]
fn data_center_details_read_the_epic_link_field() {
    let m = Mock::start(|r| match path_of(r, DC).as_str() {
        "/rest/api/2/field" => Reply::json(
            r#"[{"id":"summary","schema":{"type":"string"}},{"id":"customfield_10008","schema":{"custom":"com.pyxis.greenhopper.jira:gh-epic-link"}}]"#,
        ),
        "/rest/api/2/issue/PROJ-7?fields=summary,status,issuetype,assignee,labels,fixVersions,description,reporter,created,parent,customfield_10008" => {
            Reply::json(
                r#"{"key":"PROJ-7","fields":{"summary":"Login","description":"Plain *text*","reporter":{"displayName":"Grace"},
                "created":"2026-01-02T10:00:00.000+0000","customfield_10008":"PROJ-1"}}"#,
            )
        }
        "/rest/api/2/issue/PROJ-7/comment?orderBy=created&maxResults=100" => {
            Reply::json(r#"{"startAt":0,"total":1,"comments":[{"author":{"displayName":"Ada"},"body":"ok","created":"x"}]}"#)
        }
        _ => Reply::not_found(),
    });
    let d = issue_details(&jira(Provider::JiraDataCenter, DC, ""), &token(Provider::JiraDataCenter), "PROJ-7").unwrap();
    assert_eq!((d.body.as_str(), d.epic.as_deref()), ("Plain *text*", Some("PROJ-1")));
    assert!(d.assignees.is_empty());
    assert_eq!((d.comments[0].body.as_str(), d.comments[0].time), ("ok", 0));
    assert_eq!(routes(&m).len(), 3);
    drop(m);

    let _m = Mock::start(|_| Reply::not_found());
    let e = issue_details(&jira(Provider::JiraDataCenter, DC, ""), &token(Provider::JiraDataCenter), "PROJ-9").unwrap_err();
    assert!(e.is_not_found() && e.to_string().contains("Issue PROJ-9 wasn't found"), "{e}");
}

#[test]
fn create_options_per_site() {
    let m = Mock::start(|r| {
        let route = r.route();
        let path = path_of(r, CLOUD);
        if path.starts_with("/rest/api/3/search/jql?") {
            return Reply::json(r#"{"issues":[{"key":"PROJ-1","fields":{"summary":"Accounts"}}]}"#);
        }
        if route.starts_with(&format!("GET {DC}/rest/api/2/search?")) {
            return Reply::status(400, r#"{"errorMessages":["issuetype Epic does not exist"]}"#);
        }
        match route.as_str() {
            "GET acme.atlassian.net/rest/api/3/project/PROJ" | "GET jira.acme.io/rest/api/2/project/PROJ" => Reply::json(
                r#"{"issueTypes":[{"id":"10001","name":"Task","subtask":false},{"id":"10002","name":"Sub-task","subtask":true},{"id":"10003","name":"Bug"}]}"#,
            ),
            "GET acme.atlassian.net/rest/api/3/user/assignable/search?project=PROJ&maxResults=100" => {
                Reply::json(r#"[{"accountId":"acc-1","displayName":"Ada"}]"#)
            }
            "GET jira.acme.io/rest/api/2/user/assignable/search?project=PROJ&maxResults=100" => {
                Reply::json(r#"[{"name":"ada","displayName":"Ada"}]"#)
            }
            "GET acme.atlassian.net/rest/api/3/label?maxResults=1000" => Reply::json(r#"{"values":["ui","perf"]}"#),
            _ => Reply::not_found(),
        }
    });
    let o = issue_create_options(&jira(Provider::JiraCloud, CLOUD, ""), &token(Provider::JiraCloud), "PROJ").unwrap();
    assert_eq!(o.types.iter().map(|t| (t.key.as_str(), t.title.as_str())).collect::<Vec<_>>(), [("10001", "Task"), ("10003", "Bug")]);
    assert_eq!(o.people, [Person { id: "acc-1".into(), name: "Ada".into() }]);
    assert_eq!(o.epics.iter().map(|e| (e.key.as_str(), e.title.as_str())).collect::<Vec<_>>(), [("PROJ-1", "Accounts")]);
    assert_eq!(o.labels, ["ui", "perf"]);
    assert_eq!((o.labels_free, o.max_assignees), (true, 1));
    let jql = find(&m, "GET acme.atlassian.net/rest/api/3/search/jql?").path;
    assert!(jql.contains(&enc(r#"project = "PROJ" AND issuetype = Epic AND statusCategory != Done"#)), "{jql}");

    let o = issue_create_options(&jira(Provider::JiraDataCenter, DC, ""), &token(Provider::JiraDataCenter), "PROJ").unwrap();
    assert_eq!(o.people, [Person { id: "ada".into(), name: "Ada".into() }]);
    assert!(o.epics.is_empty() && o.labels.is_empty(), "a failed epic search leaves no epics; Data Center lists no labels");
    drop(m);

    let m = Mock::start(|_| Reply::json("{}"));
    let e = issue_create_options(&jira(Provider::JiraCloud, CLOUD, ""), &token(Provider::JiraCloud), " ").unwrap_err();
    assert!(e.to_string().contains("Choose a project."), "{e}");
    assert!(m.requests().is_empty());
}

#[test]
fn cloud_create_sends_rich_text_and_reads_the_issue_back() {
    let m = Mock::start(|r| match r.route().as_str() {
        "POST acme.atlassian.net/rest/api/3/issue" => Reply::json(r#"{"id":"100","key":"NEW-1"}"#),
        "GET acme.atlassian.net/rest/api/3/issue/NEW-1?fields=summary,status,issuetype,assignee,labels,fixVersions" => {
            Reply::json(r#"{"key":"NEW-1","fields":{"summary":"Login fails","status":{"name":"To Do"},"issuetype":{"name":"Task"}}}"#)
        }
        _ => Reply::not_found(),
    });
    let i = create_issue(&jira(Provider::JiraCloud, CLOUD, ""), &token(Provider::JiraCloud), &new_issue()).unwrap();
    assert_eq!((i.key.as_str(), i.state.as_str(), i.kind.as_str()), ("NEW-1", "To Do", "Task"));
    let f = find(&m, "POST acme.atlassian.net/rest/api/3/issue").json()["fields"].clone();
    assert_eq!(f["project"]["key"], "PROJ");
    assert_eq!((f["summary"].as_str(), f["issuetype"]["id"].as_str()), (Some("Login fails"), Some("10001")));
    assert_eq!(f["description"]["type"], "doc");
    assert_eq!(f["description"]["content"].as_array().map(Vec::len), Some(2));
    assert_eq!((f["assignee"]["accountId"].as_str(), f["parent"]["key"].as_str()), (Some("acc-1"), Some("PROJ-1")));
    assert_eq!(f["labels"], serde_json::json!(["ui"]));
    assert_eq!(routes(&m).len(), 2, "no field list on Cloud");
}

#[test]
fn data_center_create_sends_plain_text_and_the_epic_link() {
    let m = Mock::start(|r| match r.route().as_str() {
        "GET jira.acme.io/rest/api/2/field" => {
            Reply::json(r#"[{"id":"customfield_10008","schema":{"custom":"com.pyxis.greenhopper.jira:gh-epic-link"}}]"#)
        }
        "POST jira.acme.io/rest/api/2/issue" => Reply::json(r#"{"key":"NEW-2"}"#),
        _ => Reply::not_found(),
    });
    let new = NewIssue { assignees: vec!["ada".into()], ..new_issue() };
    let i = create_issue(&jira(Provider::JiraDataCenter, DC, ""), &token(Provider::JiraDataCenter), &new).unwrap();
    assert_eq!((i.key.as_str(), i.title.as_str()), ("NEW-2", "Login fails"), "falls back when the read-back finds nothing");
    assert_eq!(i.url, "https://jira.acme.io/browse/NEW-2");
    let f = find(&m, "POST jira.acme.io/rest/api/2/issue").json()["fields"].clone();
    assert_eq!(f["description"], "Steps\nhere\n\nThen this");
    assert_eq!((f["assignee"]["name"].as_str(), f["customfield_10008"].as_str()), (Some("ada"), Some("PROJ-1")));
    assert!(f.get("parent").is_none());
}

#[test]
fn field_errors_become_the_message() {
    let _m = Mock::start(|r| match r.route().as_str() {
        "POST acme.atlassian.net/rest/api/3/issue" => {
            Reply::status(400, r#"{"errorMessages":[],"errors":{"customfield_10011":"Epic Name is required."}}"#)
        }
        _ => Reply::not_found(),
    });
    let e = create_issue(&jira(Provider::JiraCloud, CLOUD, ""), &token(Provider::JiraCloud), &new_issue()).unwrap_err();
    assert!(e.to_string().contains("customfield_10011: Epic Name is required."), "{e}");
}
