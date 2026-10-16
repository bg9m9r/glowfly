//! Issue details and creating issues inside Trellis: one issue with its description, people and
//! comments, the choices the Create issue dialog offers (labels, people, types, epics), and
//! creating an issue. GitHub and GitHub Enterprise Server (`github_issues.rs`), GitLab
//! (`gitlab_issues.rs`) and Jira Cloud and Data Center (`jira_issues.rs`) for now; every other
//! service answers `Error::Unsupported` without sending anything. Links in the answers are
//! checked here, once for every service.

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
    /// Jira: "KEY  Summary" of the parent epic.
    pub epic: Option<String>,
    /// Oldest first; kind always EntryKind::Comment.
    pub comments: Vec<Entry>,
}

/// Someone an issue can be assigned to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Person {
    /// A GitHub login, a GitLab numeric user id, a Jira Cloud accountId or a Jira Data Center
    /// username: what the service takes when assigning.
    pub id: String,
    pub name: String,
}

/// What the Create issue dialog offers on a service.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CreateOptions {
    pub labels: Vec<String>,
    pub people: Vec<Person>,
    /// Issue types (Jira: by id; GitLab: issue and incident).
    pub types: Vec<Named>,
    /// Open epics of the project (Jira).
    pub epics: Vec<Named>,
    /// Labels the service doesn't know yet can be added.
    pub labels_free: bool,
    pub max_assignees: usize,
}

/// An issue to create.
#[derive(Clone, Debug, PartialEq)]
pub struct NewIssue {
    /// The Jira project key; empty elsewhere.
    pub project: String,
    pub title: String,
    pub description: String,
    pub labels: Vec<String>,
    /// `Person::id`s.
    pub assignees: Vec<String>,
    /// A `CreateOptions::types` key.
    pub kind: Option<String>,
    /// A `CreateOptions::epics` key.
    pub epic: Option<String>,
}

/// The longest title (Jira: summary) Trellis sends.
pub const MAX_TITLE: usize = 255;

/// The longest description the service takes, in characters.
pub fn max_description(p: Provider) -> usize {
    match p {
        Provider::GitLab | Provider::GitLabSelfManaged => 1_048_576,
        _ if p.is_jira() => 32_767,
        _ => 65_536,
    }
}

impl NewIssue {
    /// Why the service would refuse the issue, in the order the dialog lists them.
    pub fn problems(&self, p: Provider) -> Vec<String> {
        let jira = p.is_jira();
        let mut out = Vec::new();
        if jira && self.project.trim().is_empty() {
            out.push("Choose a project.".to_owned());
        }
        let title = self.title.trim();
        if title.is_empty() {
            out.push(if jira { "Enter a summary." } else { "Enter a title." }.to_owned());
        } else if title.chars().count() > MAX_TITLE {
            out.push(format!("Keep the {} to {MAX_TITLE} characters.", if jira { "summary" } else { "title" }));
        }
        let max = max_description(p);
        if self.description.chars().count() > max {
            out.push(format!("The description is too long for {} ({max} characters at most).", p.name()));
        }
        if jira && self.kind.as_deref().is_none_or(|k| k.trim().is_empty()) {
            out.push("Choose an issue type.".to_owned());
        }
        out
    }
}

/// Whether issue details and creating issues work inside Trellis for this service.
pub fn issues_in_app(p: Provider) -> bool {
    p.issues_in_app()
}

fn unsupported(t: &Target) -> Error {
    Error::Unsupported(format!("{} issues aren't supported in Trellis yet", t.provider.name()))
}

/// Turns the service's 404 for `key` into a readable "wasn't found".
pub(super) fn missing(t: &Target, key: &str) -> impl FnOnce(Error) -> Error {
    let shown = mention(t.provider.ref_style(), key);
    move |e| if e.is_not_found() { Error::NotFound(format!("Issue {shown} wasn't found")) } else { e }
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

/// The labels, people, types and epics to offer when creating an issue in `project` (Jira).
pub fn issue_create_options(t: &Target, auth: &Auth, project: &str) -> Result<CreateOptions> {
    if !issues_in_app(t.provider) {
        return Err(unsupported(t));
    }
    service(t.provider).issue_create_options(t, auth, project)
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
        for (p, max) in [(Provider::GitHub, 65_536), (Provider::GitLabSelfManaged, 1_048_576), (Provider::JiraCloud, 32_767)] {
            assert_eq!(max_description(p), max);
            let long = NewIssue { description: "d".repeat(max + 1), ..ok.clone() };
            let msg = format!("The description is too long for {} ({max} characters at most).", p.name());
            assert_eq!(long.problems(p), [msg], "{p:?}");
            assert!(NewIssue { description: "d".repeat(max), ..ok.clone() }.problems(p).is_empty());
        }
    }

    #[test]
    fn issues_in_app_for_code_hosts_and_jira() {
        for p in Provider::ALL {
            let expected = matches!(
                p,
                Provider::GitHub
                    | Provider::GitHubEnterprise
                    | Provider::GitLab
                    | Provider::GitLabSelfManaged
                    | Provider::JiraCloud
                    | Provider::JiraDataCenter
            );
            assert_eq!(issues_in_app(p), expected, "{p:?}");
        }
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
