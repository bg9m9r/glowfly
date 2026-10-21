//! Azure Boards work items in Glowfly: details with the HTML description as text, the parent and
//! paged comments; the Create dialog's types, tags, people and parents; and creating a work item
//! with a JSON Patch.

use super::issues::new_issue;
use super::*;

const BASE: &str = "dev.azure.com/contoso/Fabrikam%20Web";
const FIELDS: &str = "fields=System.Title,System.State,System.WorkItemType,System.AssignedTo,System.Tags&errorPolicy=omit&api-version=7.1";

fn t() -> Target {
    remote("https://dev.azure.com/contoso/Fabrikam%20Web/_git/web%20app")
}

const STORY: &str = r#"{"id":5,"fields":{"System.Title":"Login page","System.State":"Active","System.WorkItemType":"User Story",
    "System.AssignedTo":{"displayName":"Ada","uniqueName":"ada@contoso.com"},"System.Tags":"ux; needs design",
    "System.Description":"<div>Users sign in.</div><ul><li>Email</li><li>SSO &amp; more</li></ul>",
    "System.CreatedBy":{"displayName":"Grace"},"System.CreatedDate":"2026-01-02T10:00:00.000Z"},
    "relations":[{"rel":"System.LinkTypes.Related","url":"https://dev.azure.com/contoso/_apis/wit/workItems/9"},
                 {"rel":"System.LinkTypes.Hierarchy-Reverse","url":"https://dev.azure.com/contoso/_apis/wit/workItems/1"}]}"#;

const BUG: &str = r#"{"id":7,"fields":{"System.Title":"Crash","System.State":"New","System.WorkItemType":"Bug",
    "System.Description":"","Microsoft.VSTS.TCM.ReproSteps":"<p>Open it</p><p>It <b>crashes</b></p>"}}"#;

fn details_server() -> Mock {
    Mock::start(|r| {
        let route = r.route();
        let path = route.strip_prefix(&format!("GET {BASE}")).unwrap_or_default();
        match path {
            "/_apis/wit/workitems/5?$expand=relations&api-version=7.1" => Reply::json(STORY),
            "/_apis/wit/workitems/7?$expand=relations&api-version=7.1" => Reply::json(BUG),
            p if p == format!("/_apis/wit/workitems?ids=1&{FIELDS}") => {
                Reply::json(r#"{"value":[{"id":1,"fields":{"System.Title":"Accounts","System.WorkItemType":"Epic"}}]}"#)
            }
            "/_apis/wit/workItems/5/comments?$top=200&order=asc&api-version=7.1-preview.4" => Reply::json(
                r#"{"comments":[{"id":1,"text":"<div>First &lt;one&gt;</div>","createdBy":{"displayName":"Ada"},"createdDate":"2026-01-03T10:00:00Z"},
                {"id":2,"text":"gone","isDeleted":true}],"continuationToken":"next-1"}"#,
            ),
            "/_apis/wit/workItems/5/comments?$top=200&order=asc&api-version=7.1-preview.4&continuationToken=next-1" => Reply::json(
                r#"{"comments":[{"id":3,"text":"Second","createdBy":{"displayName":"Grace"},"createdDate":"2026-01-04T10:00:00Z"}],"continuationToken":""}"#,
            ),
            "/_apis/wit/workItems/7/comments?$top=200&order=asc&api-version=7.1-preview.4" => Reply::json(r#"{"comments":[]}"#),
            _ => Reply::not_found(),
        }
    })
}

#[test]
fn details_read_the_html_parent_and_paged_comments() {
    let m = details_server();
    let d = issue_details(&t(), &token(Provider::AzureDevOps), "5").unwrap();
    assert_eq!((d.issue.key.as_str(), d.issue.kind.as_str(), d.issue.state.as_str()), ("5", "User Story", "Active"));
    assert_eq!(d.issue.facets.labels, ["ux", "needs design"]);
    assert_eq!(d.body, "Users sign in.\n- Email\n- SSO & more");
    assert_eq!((d.author.as_str(), d.created, d.assignees.as_slice()), ("Grace", 1_767_348_000, &["Ada".to_owned()][..]));
    assert_eq!(d.epic.as_deref(), Some("1  Accounts"));
    let rows: Vec<(&str, &str)> = d.comments.iter().map(|c| (c.author.as_str(), c.body.as_str())).collect();
    assert_eq!(rows, [("Ada", "First <one>"), ("Grace", "Second")], "both pages, the deleted one left out");
    assert_eq!(routes(&m).len(), 4);

    let d = issue_details(&t(), &token(Provider::AzureDevOps), "7").unwrap();
    assert_eq!((d.body.as_str(), d.epic.as_deref()), ("Open it\n\nIt crashes", None), "a Bug's repro steps");
    let e = issue_details(&t(), &token(Provider::AzureDevOps), "8").unwrap_err();
    assert!(e.is_not_found() && e.to_string().contains("Work item AB#8 wasn't found"), "{e}");
    let before = routes(&m).len();
    assert!(issue_details(&t(), &token(Provider::AzureDevOps), "AB#5").unwrap_err().is_not_found());
    assert_eq!(routes(&m).len(), before, "a key that isn't a number sends nothing");
}

const WHERE: &str = "[System.TeamProject] = @project AND [System.WorkItemType] IN ('Epic', 'Feature') AND [System.State] NOT IN \
                     ('Closed', 'Done', 'Removed', 'Resolved')";

#[test]
fn options_offer_types_tags_people_and_parents() {
    let m = Mock::start(|r| {
        let route = r.route();
        match route.as_str() {
            "GET dev.azure.com/contoso/Fabrikam%20Web/_apis/wit/workitemtypes?api-version=7.1" => Reply::json(
                r#"{"value":[{"name":"Bug"},{"name":"Epic"},{"name":"User Story"},{"name":"Old","isDisabled":true},
                {"name":"Code Review Request"},{"name":"Feature"}]}"#,
            ),
            "GET dev.azure.com/contoso/Fabrikam%20Web/_apis/wit/workitemtypecategories?api-version=7.1" => Reply::json(
                r#"{"value":[{"referenceName":"Microsoft.RequirementCategory","defaultWorkItemType":{"name":"User Story"},"workItemTypes":[{"name":"User Story"}]},
                {"referenceName":"Microsoft.HiddenCategory","defaultWorkItemType":{"name":"Code Review Request"},"workItemTypes":[{"name":"Code Review Request"}]}]}"#,
            ),
            "GET dev.azure.com/contoso/Fabrikam%20Web/_apis/wit/tags?api-version=7.1-preview.1" => {
                Reply::json(r#"{"value":[{"name":"ux"},{"name":"backend"}]}"#)
            }
            "GET dev.azure.com/contoso/_apis/projects/Fabrikam%20Web?api-version=7.1" => {
                Reply::json(r#"{"id":"p-1","defaultTeam":{"id":"team-1","name":"Fabrikam Web Team"}}"#)
            }
            "GET dev.azure.com/contoso/_apis/projects/Fabrikam%20Web/teams/team-1/members?$top=100&api-version=7.1" => Reply::json(
                r#"{"value":[{"identity":{"displayName":"Ada","uniqueName":"ada@contoso.com"}},
                {"identity":{"displayName":"Readers","uniqueName":"[Fabrikam]\\Readers","isContainer":true}},
                {"identity":{"displayName":"Ada","uniqueName":"ada@contoso.com"}}]}"#,
            ),
            "POST dev.azure.com/contoso/Fabrikam%20Web/_apis/wit/wiql?$top=1000&api-version=7.1" => {
                Reply::json(r#"{"workItems":[{"id":1},{"id":2}]}"#)
            }
            r if r == format!("GET {BASE}/_apis/wit/workitems?ids=1,2&{FIELDS}") => Reply::json(
                r#"{"value":[{"id":1,"fields":{"System.Title":"Accounts","System.WorkItemType":"Epic"}},
                {"id":2,"fields":{"System.Title":"Search","System.WorkItemType":"Feature"}}]}"#,
            ),
            _ => Reply::not_found(),
        }
    });
    let o = issue_create_options(&t(), &token(Provider::AzureDevOps), "").unwrap();
    assert_eq!(o.types.iter().map(|n| n.key.as_str()).collect::<Vec<_>>(), ["Bug", "Epic", "User Story", "Feature"]);
    assert_eq!(o.default_type.as_deref(), Some("User Story"));
    assert_eq!(o.labels, ["ux", "backend"]);
    assert_eq!(o.people, [Person { id: "ada@contoso.com".into(), name: "Ada".into() }]);
    let parents: Vec<(&str, &str)> = o.epics.iter().map(|e| (e.key.as_str(), e.title.as_str())).collect();
    assert_eq!(parents, [("1", "Epic · Accounts"), ("2", "Feature · Search")]);
    assert_eq!((o.labels_free, o.max_assignees), (true, 1));
    let query =
        find(&m, "POST dev.azure.com/contoso/Fabrikam%20Web/_apis/wit/wiql").json()["query"].as_str().unwrap_or_default().to_owned();
    assert_eq!(query, format!("SELECT [System.Id] FROM WorkItems WHERE {WHERE} ORDER BY [System.ChangedDate] DESC"));
}

#[test]
fn forbidden_team_members_leave_no_people() {
    let _m = Mock::start(|r| match r.route().as_str() {
        "GET dev.azure.com/contoso/Fabrikam%20Web/_apis/wit/workitemtypes?api-version=7.1" => Reply::json(r#"{"value":[{"name":"Task"}]}"#),
        "GET dev.azure.com/contoso/Fabrikam%20Web/_apis/wit/workitemtypecategories?api-version=7.1" => Reply::json(r#"{"value":[]}"#),
        "GET dev.azure.com/contoso/Fabrikam%20Web/_apis/wit/tags?api-version=7.1-preview.1" => Reply::json(r#"{"value":[]}"#),
        "POST dev.azure.com/contoso/Fabrikam%20Web/_apis/wit/wiql?$top=1000&api-version=7.1" => Reply::json(r#"{"workItems":[]}"#),
        _ => Reply::status(403, r#"{"message":"TF401027: You need the Project and Team read permission."}"#),
    });
    let o = issue_create_options(&t(), &token(Provider::AzureDevOps), "").unwrap();
    assert!(o.people.is_empty());
    assert_eq!((o.types.len(), o.default_type), (1, None));
}

const CREATED: &str = r#"{"id":12,"fields":{"System.Title":"Login page","System.State":"New","System.WorkItemType":"User Story"}}"#;

#[test]
fn create_posts_a_json_patch() {
    let m = Mock::start(|r| match r.route().as_str() {
        "POST dev.azure.com/contoso/Fabrikam%20Web/_apis/wit/workitems/$User%20Story?api-version=7.1" => Reply::json(CREATED),
        "POST dev.azure.com/contoso/Fabrikam%20Web/_apis/wit/workitems/$Bug?api-version=7.1" => Reply::json(CREATED),
        _ => Reply::not_found(),
    });
    let new = NewIssue {
        description: "Sign in <fast>\nwith SSO\n\nThen more".into(),
        labels: vec!["a".into(), "b".into()],
        assignees: vec!["ada@contoso.com".into()],
        kind: Some("User Story".into()),
        epic: Some("1".into()),
        ..new_issue("Login page")
    };
    let i = create_issue(&t(), &token(Provider::AzureDevOps), &new).unwrap();
    assert_eq!((i.key.as_str(), i.url.as_str()), ("12", "https://dev.azure.com/contoso/Fabrikam%20Web/_workitems/edit/12"));
    let sent = find(&m, "POST dev.azure.com/contoso/Fabrikam%20Web/_apis/wit/workitems/$User%20Story");
    assert_eq!(sent.header("Content-Type"), Some("application/json-patch+json"));
    let add = |path: &str, value: serde_json::Value| serde_json::json!({ "op": "add", "path": path, "value": value });
    assert_eq!(
        sent.json(),
        serde_json::json!([
            add("/fields/System.Title", "Login page".into()),
            add("/fields/System.Description", "<div>Sign in &lt;fast&gt;<br>with SSO</div><div><br></div><div>Then more</div>".into()),
            add("/fields/System.Tags", "a; b".into()),
            add("/fields/System.AssignedTo", "ada@contoso.com".into()),
            add(
                "/relations/-",
                serde_json::json!({ "rel": "System.LinkTypes.Hierarchy-Reverse", "url": "https://dev.azure.com/contoso/_apis/wit/workItems/1" })
            ),
        ])
    );
    let bug = NewIssue { description: "Steps".into(), kind: Some("bug".into()), ..new_issue("Crash") };
    let bug = NewIssue { kind: Some("Bug".into()), ..bug };
    create_issue(&t(), &token(Provider::AzureDevOps), &bug).unwrap();
    let sent = find(&m, "POST dev.azure.com/contoso/Fabrikam%20Web/_apis/wit/workitems/$Bug");
    assert_eq!(sent.json()[1], add("/fields/Microsoft.VSTS.TCM.ReproSteps", "<div>Steps</div>".into()), "a Bug's text is its repro steps");
    assert_eq!(sent.json().as_array().map(Vec::len), Some(2));
}

#[test]
fn a_read_only_token_is_told_the_scope() {
    let _m = Mock::start(|_| Reply::status(403, r#"{"message":"TF237111: no permission"}"#));
    let new = NewIssue { kind: Some("Task".into()), ..new_issue("Do it") };
    let e = create_issue(&t(), &token(Provider::AzureDevOps), &new).unwrap_err();
    assert!(matches!(&e, Error::Forbidden(m) if m.contains("TF237111") && m.contains("Work Items (Read & write) scope")), "{e}");
}
