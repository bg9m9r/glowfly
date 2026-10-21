//! Issue details and creating issues inside Glowfly: one issue with its description, people and
//! comments, the choices the Create issue dialog offers (labels, people, types, epics), and
//! creating an issue. GitHub and GitHub Enterprise Server (`github_issues.rs`), GitLab
//! (`gitlab_issues.rs`), Jira Cloud and Data Center (`jira_issues.rs`), Bitbucket Cloud
//! (`bitbucket_issues.rs`), Azure DevOps work items (`azure_issues.rs`) and Trello cards
//! (`trello_issues.rs`). Bitbucket Data Center has no issue tracker of its own, so it answers
//! `Error::Unsupported` without sending anything. Links in the answers are checked here, once
//! for every service.

use super::filters::Named;
use super::{Auth, Entry, Issue, Provider, Target, links, mention, service};
use crate::error::{Error, Result};

/// One issue as its page on the service has it.
#[derive(Clone, Debug, PartialEq)]
pub struct IssueDetails {
    pub issue: Issue,
    pub body: String,
    pub author: String,
    /// Unix seconds; 0 when the service sent no readable time.
    pub created: i64,
    pub assignees: Vec<String>,
    /// Bitbucket: trivial, minor, major, critical or blocker.
    pub priority: Option<String>,
    /// Jira: "KEY  Summary" of the parent epic; Azure DevOps: "id  title" of the parent work
    /// item.
    pub epic: Option<String>,
    /// Oldest first; kind always EntryKind::Comment.
    pub comments: Vec<Entry>,
}

/// Someone an issue can be assigned to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Person {
    /// A GitHub login, a GitLab numeric user id, a Jira Cloud accountId, a Jira Data Center
    /// username, a Bitbucket account uuid, an Azure DevOps uniqueName or a Trello member id:
    /// what the service takes when assigning.
    pub id: String,
    pub name: String,
}

/// What the Create issue dialog offers on a service.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CreateOptions {
    pub labels: Vec<String>,
    pub people: Vec<Person>,
    /// Issue types (Jira: by id; GitLab: issue and incident; Bitbucket: kinds; Azure DevOps:
    /// work item types; Trello: the board's lists).
    pub types: Vec<Named>,
    /// Priorities (Bitbucket).
    pub priorities: Vec<Named>,
    /// The `types` key the dialog picks first.
    pub default_type: Option<String>,
    /// The `priorities` key the dialog picks first.
    pub default_priority: Option<String>,
    /// Open epics of the project (Jira), or the open epics and features a work item can be
    /// filed under (Azure DevOps).
    pub epics: Vec<Named>,
    /// Labels the service doesn't know yet can be added.
    pub labels_free: bool,
    pub max_assignees: usize,
    /// Trello's open boards, sent only when asked with no board.
    pub boards: Vec<Named>,
    /// Trello's page that allows a read-and-write token for the connected API key.
    pub write_token_url: Option<String>,
}

/// An issue to create.
#[derive(Clone, Debug, PartialEq)]
pub struct NewIssue {
    /// The Jira project key or Trello board id; empty elsewhere.
    pub project: String,
    pub title: String,
    pub description: String,
    pub labels: Vec<String>,
    /// `Person::id`s.
    pub assignees: Vec<String>,
    /// A `CreateOptions::types` key: Jira type id, GitLab issue_type, Bitbucket kind, Azure
    /// work item type name, Trello list id.
    pub kind: Option<String>,
    /// A `CreateOptions::priorities` key (Bitbucket).
    pub priority: Option<String>,
    /// A `CreateOptions::epics` key: Jira epic key or Azure parent work item id.
    pub epic: Option<String>,
}

/// The longest title (Jira: summary; Trello: name) Glowfly sends.
pub const MAX_TITLE: usize = 255;

/// The longest description the service takes, in characters.
pub fn max_description(p: Provider) -> usize {
    match p {
        Provider::GitLab | Provider::GitLabSelfManaged => 1_048_576,
        Provider::Trello => 16_384,
        _ if p.is_jira() => 32_767,
        _ => 65_536,
    }
}

/// What the tracker calls an issue: "work item" on Azure DevOps, "card" on Trello, else
/// "issue".
pub fn noun(p: Provider) -> &'static str {
    match p {
        Provider::AzureDevOps => "work item",
        Provider::Trello => "card",
        _ => "issue",
    }
}

/// What the tracker calls an issue's title: "summary" on Jira, "name" on Trello, else "title".
pub fn title_word(p: Provider) -> &'static str {
    match p {
        Provider::Trello => "name",
        _ if p.is_jira() => "summary",
        _ => "title",
    }
}

/// Why an issue without a type is refused, on trackers that need one (Trello: its list).
pub fn type_problem(p: Provider) -> Option<&'static str> {
    match p {
        Provider::AzureDevOps => Some("Choose a work item type."),
        Provider::Trello => Some("Choose a list."),
        _ if p.is_jira() => Some("Choose an issue type."),
        _ => None,
    }
}

/// Why an issue without a project is refused: a Jira project, a Trello board.
pub fn project_problem(p: Provider) -> Option<&'static str> {
    match p {
        Provider::Trello => Some("Choose a board."),
        _ if p.is_jira() => Some("Choose a project."),
        _ => None,
    }
}

/// Whether every issue on the tracker needs a type.
pub fn type_required(p: Provider) -> bool {
    type_problem(p).is_some()
}

/// The part of a new issue a problem is about, so a form can show it under that field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IssueField {
    Project,
    Title,
    Description,
    Type,
    Assignees,
}

impl NewIssue {
    /// Why the service would refuse the issue, in the order the dialog lists them.
    pub fn problems(&self, p: Provider) -> Vec<String> {
        self.field_problems(p).into_iter().map(|(_, why)| why).collect()
    }

    /// `problems`, each with the field it is about.
    pub fn field_problems(&self, p: Provider) -> Vec<(IssueField, String)> {
        let mut out = Vec::new();
        if let Some(why) = project_problem(p)
            && self.project.trim().is_empty()
        {
            out.push((IssueField::Project, why.to_owned()));
        }
        let word = title_word(p);
        let title = self.title.trim();
        if title.is_empty() {
            out.push((IssueField::Title, format!("Enter a {word}.")));
        } else if title.chars().count() > MAX_TITLE {
            out.push((IssueField::Title, format!("Keep the {word} to {MAX_TITLE} characters.")));
        }
        let max = max_description(p);
        if self.description.chars().count() > max {
            out.push((IssueField::Description, format!("The description is too long for {} ({max} characters at most).", p.name())));
        }
        if let Some(why) = type_problem(p)
            && self.kind.as_deref().is_none_or(|k| k.trim().is_empty())
        {
            out.push((IssueField::Type, why.to_owned()));
        }
        out
    }
}

/// Whether issue details and creating issues work inside Glowfly for this service.
pub fn issues_in_app(p: Provider) -> bool {
    p.issues_in_app()
}

fn unsupported(t: &Target) -> Error {
    if t.provider == Provider::BitbucketDataCenter {
        return Error::Unsupported("Bitbucket Data Center has no issue tracker of its own; use Jira for its issues".into());
    }
    Error::Unsupported(format!("{} issues aren't supported in Glowfly yet", t.provider.name()))
}

/// "Issue #8 wasn't found", "Card AbCd1234 wasn't found": in the tracker's words.
pub(super) fn not_found(t: &Target, key: &str) -> Error {
    let mut c = noun(t.provider).chars();
    let noun: String = c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default();
    Error::NotFound(format!("{noun} {} wasn't found", mention(t.provider.ref_style(), key)))
}

/// Turns the service's 404 for `key` into a readable "wasn't found".
pub(super) fn missing(t: &Target, key: &str) -> impl FnOnce(Error) -> Error {
    let found = not_found(t, key);
    move |e| if e.is_not_found() { found } else { e }
}

/// Issue `key` with its description and comments.
pub fn issue_details(t: &Target, auth: &Auth, key: &str) -> Result<IssueDetails> {
    if !issues_in_app(t.provider) {
        return Err(unsupported(t));
    }
    let mut d = service(t.provider).issue_details(t, auth, key)?;
    d.issue = links::issue(t, d.issue);
    for c in &mut d.comments {
        c.url = links::web_url(t, std::mem::take(&mut c.url));
    }
    Ok(d)
}

/// The labels, people, types and epics to offer when creating an issue in `project` (a Jira
/// project or Trello board).
pub fn issue_create_options(t: &Target, auth: &Auth, project: &str) -> Result<CreateOptions> {
    if !issues_in_app(t.provider) {
        return Err(unsupported(t));
    }
    let mut o = service(t.provider).issue_create_options(t, auth, project)?;
    o.write_token_url = o.write_token_url.filter(|u| links::https_host(u).is_some());
    Ok(o)
}

/// Creates `new`; an issue with problems is refused before anything is sent.
pub fn create_issue(t: &Target, auth: &Auth, new: &NewIssue) -> Result<Issue> {
    if !issues_in_app(t.provider) {
        return Err(unsupported(t));
    }
    if let Some(first) = new.problems(t.provider).into_iter().next() {
        return Err(Error::Other(first));
    }
    service(t.provider).create_issue(t, auth, new).map(|i| links::issue(t, i))
}

/// A service's time: RFC 3339, else Jira's `2026-01-02T10:00:00.000+0000`; 0 when unreadable.
pub(super) fn api_time(s: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(s)
        .or_else(|_| chrono::DateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f%z"))
        .map(|d| d.timestamp())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new(title: &str) -> NewIssue {
        NewIssue {
            project: String::new(),
            title: title.into(),
            description: String::new(),
            labels: Vec::new(),
            assignees: Vec::new(),
            kind: None,
            priority: None,
            epic: None,
        }
    }

    #[test]
    fn problems_per_service() {
        assert_eq!(new("  ").problems(Provider::GitHub), ["Enter a title."]);
        assert!(new("Crash").problems(Provider::GitLab).is_empty());
        assert_eq!(new(&"x".repeat(256)).problems(Provider::GitHub), ["Keep the title to 255 characters."]);
        assert!(new(&"x".repeat(255)).problems(Provider::GitHub).is_empty());
        let jira = new("");
        assert_eq!(jira.problems(Provider::JiraCloud), ["Choose a project.", "Enter a summary.", "Choose an issue type."]);
        let ok = NewIssue { project: "P".into(), kind: Some("10001".into()), ..new("Crash") };
        assert!(ok.problems(Provider::JiraDataCenter).is_empty());
        for (p, max) in [
            (Provider::GitHub, 65_536),
            (Provider::GitLabSelfManaged, 1_048_576),
            (Provider::JiraCloud, 32_767),
            (Provider::Trello, 16_384),
        ] {
            assert_eq!(max_description(p), max);
            let long = NewIssue { description: "d".repeat(max + 1), ..ok.clone() };
            let msg = format!("The description is too long for {} ({max} characters at most).", p.name());
            assert_eq!(long.problems(p), [msg], "{p:?}");
            assert!(NewIssue { description: "d".repeat(max), ..ok.clone() }.problems(p).is_empty());
        }
        assert_eq!(new("").problems(Provider::Trello), ["Choose a board.", "Enter a name.", "Choose a list."]);
        assert_eq!(new(&"x".repeat(256)).problems(Provider::Trello)[1], "Keep the name to 255 characters.");
        assert_eq!(new("Login").problems(Provider::AzureDevOps), ["Choose a work item type."]);
        assert!(NewIssue { kind: Some("User Story".into()), ..new("Login") }.problems(Provider::AzureDevOps).is_empty());
        assert!(new("Crash").problems(Provider::Bitbucket).is_empty(), "no type needed");
    }

    #[test]
    fn words_per_tracker() {
        use Provider::*;
        for (p, n, word, kind, project) in [
            (GitHub, "issue", "title", None, None),
            (Bitbucket, "issue", "title", None, None),
            (AzureDevOps, "work item", "title", Some("Choose a work item type."), None),
            (JiraCloud, "issue", "summary", Some("Choose an issue type."), Some("Choose a project.")),
            (JiraDataCenter, "issue", "summary", Some("Choose an issue type."), Some("Choose a project.")),
            (Trello, "card", "name", Some("Choose a list."), Some("Choose a board.")),
        ] {
            assert_eq!((noun(p), title_word(p), type_problem(p), project_problem(p)), (n, word, kind, project), "{p:?}");
            assert_eq!(type_required(p), kind.is_some(), "{p:?}");
        }
    }

    #[test]
    fn issues_in_app_everywhere_but_bitbucket_data_center() {
        for p in Provider::ALL {
            assert_eq!(issues_in_app(p), p != Provider::BitbucketDataCenter, "{p:?}");
        }
    }

    #[test]
    fn not_found_names_the_trackers_noun() {
        let t = |p: Provider| Target { provider: p, host: "h".into(), owner: "o".into(), project: "p".into(), repo: "r".into() };
        let gone = || Error::NotFound(String::new());
        assert_eq!(missing(&t(Provider::Trello), "AbCd1234")(gone()).to_string(), "not found: Card AbCd1234 wasn't found");
        assert_eq!(missing(&t(Provider::AzureDevOps), "9")(gone()).to_string(), "not found: Work item AB#9 wasn't found");
        assert_eq!(missing(&t(Provider::GitHub), "8")(gone()).to_string(), "not found: Issue #8 wasn't found");
        assert!(matches!(missing(&t(Provider::GitHub), "8")(Error::Forbidden("x".into())), Error::Forbidden(_)));
        let e = unsupported(&t(Provider::BitbucketDataCenter));
        assert!(e.to_string().contains("no issue tracker of its own; use Jira"), "{e}");
    }

    #[test]
    fn reads_service_times() {
        assert_eq!(api_time("2026-01-02T10:00:00Z"), 1_767_348_000);
        assert_eq!(api_time("2026-01-02T10:00:00.000+0000"), 1_767_348_000);
        assert_eq!(api_time("2026-01-02T11:00:00.000+0100"), 1_767_348_000);
        assert_eq!(api_time("yesterday"), 0);
        assert_eq!(api_time(""), 0);
    }
}
