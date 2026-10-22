//! The issue panel (update/issue.rs, ui/issue/): a copy of the listed issue, the tracker it
//! came from, the details loaded from the service, and the texts that describe them. Pure: the
//! details are loaded by `Effect::Issue`.

use super::ForgeState;
use super::issue_words::{epic_label, kind_row_label, labels_label, people_label, priority_label};
pub use super::issue_words::{milestone_label, state_label};
use glowfly_core::forge::{Issue, IssueDetails, Provider, Target, issues_in_app};
use glowfly_core::repo::{BranchInfo, Snapshot};
use glowfly_core::settings::Settings;

/// The service's details of the issue.
#[derive(Clone, PartialEq, Debug)]
pub enum Details {
    /// The tracker has no issue details in Glowfly.
    Unsupported,
    /// The repository isn't trusted, so nothing is asked.
    Held,
    Loading,
    Loaded(Box<IssueDetails>),
    Failed(String),
}

/// The open issue panel.
#[derive(Clone, Debug)]
pub struct IssueView {
    pub key: String,
    /// The issue as listed, replaced by the loaded copy.
    pub issue: Issue,
    /// The tracker it belongs to.
    pub target: Target,
    /// "#12" or "PROJ-7".
    pub mention: String,
    /// "issue", "work item" or "card".
    pub noun: &'static str,
    pub details: Details,
    /// The details are loading again while the last ones stay on screen.
    pub reloading: bool,
    /// The comment box's draft, its touched state, whether it is posting and the last failure.
    pub comment: super::issue_comment::CommentDraft,
}

impl IssueView {
    /// The panel for `issue` on `fs`'s tracker; None without a tracker.
    pub fn new(issue: Issue, fs: &ForgeState) -> Option<IssueView> {
        let target = fs.tracker.clone()?;
        Some(IssueView {
            key: issue.key.clone(),
            mention: fs.mention(&issue.key),
            noun: fs.issue_noun(),
            details: initial_details(fs),
            reloading: false,
            comment: Default::default(),
            issue,
            target,
        })
    }

    pub fn loaded(&self) -> Option<&IssueDetails> {
        match &self.details {
            Details::Loaded(d) => Some(d),
            _ => None,
        }
    }

    /// When the issue was opened, once the details are loaded and the service sent a time.
    pub fn created(&self) -> Option<i64> {
        self.loaded().map(|d| d.created).filter(|&c| c > 0)
    }

    pub fn provider(&self) -> Provider {
        self.target.provider
    }

    /// Whether the description and comments are Markdown (GitHub, GitLab, Bitbucket, Trello),
    /// not text Glowfly made from a rich text format (Azure Boards' HTML, Jira Cloud's
    /// documents) or Jira Data Center's own markup, which are shown as they are.
    pub fn markdown(&self) -> bool {
        !matches!(self.provider(), Provider::AzureDevOps | Provider::JiraCloud | Provider::JiraDataCenter)
    }

    /// Closed, done or set aside: the icon turns purple.
    pub fn closed(&self) -> bool {
        let s = self.issue.state.to_lowercase();
        s.starts_with("closed") || matches!(s.as_str(), "done" | "resolved" | "invalid" | "duplicate" | "wontfix" | "removed" | "archived")
    }

    /// The issue's type (Bitbucket's kinds capitalized; Trello: the card's board), else the
    /// noun: "Bug", "Issue".
    pub fn kind(&self) -> String {
        let kind = self.issue.kind.trim();
        if kind.is_empty() {
            capitalized(self.noun)
        } else if self.provider() == Provider::Bitbucket {
            capitalized(kind)
        } else {
            self.issue.kind.clone()
        }
    }

    /// "Bug · opened by ada · March 3, 2026" ("Card on Website · …" on Trello); only the type
    /// until the details load.
    pub fn kind_line(&self, settings: &Settings, offset: i32) -> String {
        let mut line = match self.provider() {
            Provider::Trello if !self.issue.kind.trim().is_empty() => format!("Card on {}", self.issue.kind.trim()),
            _ => self.kind(),
        };
        if let Some(d) = self.loaded() {
            if !d.author.is_empty() {
                line.push_str(&format!(" · opened by {}", d.author));
            }
            if d.created > 0 {
                line.push_str(&format!(" · {}", settings.format_date_word(d.created, offset)));
            }
        }
        line
    }
}

fn capitalized(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

/// The listed copy of issue `key`: assigned to the user, found by the shown filter, or cached
/// for a commit message's reference.
pub fn find_listed<'a>(fs: &'a ForgeState, key: &str) -> Option<&'a Issue> {
    let searched = fs.filters.issues.iter().flat_map(|l| l.items.iter());
    fs.my_issues.iter().chain(searched).find(|i| i.key == key).or_else(|| fs.cache.get(key))
}

/// The issue to open for `key`: the listed copy, else one with only its key and page.
pub fn listed_or_bare(fs: &ForgeState, key: &str) -> Issue {
    find_listed(fs, key).cloned().unwrap_or_else(|| Issue {
        key: key.to_owned(),
        title: String::new(),
        state: String::new(),
        kind: String::new(),
        assignee: None,
        url: fs.issue_url(key),
        facets: Default::default(),
    })
}

/// The side panel's Details rows, as (label, text), in the tracker's words. Labels are listed
/// comma-separated (the view draws them as chips); rows without a value are left out, apart
/// from the assignees.
pub fn meta_rows(v: &IssueView, settings: &Settings, offset: i32) -> Vec<(&'static str, String)> {
    let p = v.provider();
    let i = &v.issue;
    let d = v.loaded();
    let assignees = match d {
        Some(d) => d.assignees.join(", "),
        None => i.assignee.clone().unwrap_or_default(),
    };
    let mut rows = vec![(state_label(p), i.state.clone()), (kind_row_label(p), v.kind())];
    if let Some(priority) = d.and_then(|d| d.priority.as_ref()).filter(|x| !x.is_empty()) {
        rows.push((priority_label(p), capitalized(priority)));
    }
    rows.push((people_label(p), if assignees.is_empty() { "Unassigned".to_owned() } else { assignees }));
    if !i.facets.labels.is_empty() {
        rows.push((labels_label(p), i.facets.labels.join(", ")));
    }
    if let Some(m) = i.facets.milestone.as_ref().filter(|m| !m.is_empty()) {
        rows.push((milestone_label(p), m.clone()));
    }
    if let Some(e) = d.and_then(|d| d.epic.clone()).filter(|_| p.is_jira() || p == Provider::AzureDevOps) {
        rows.push((epic_label(p), e));
    }
    if let Some(d) = d {
        if !d.author.is_empty() {
            rows.push(("Created by", d.author.clone()));
        }
        if d.created > 0 {
            rows.push(("Created", settings.format_date_word(d.created, offset)));
        }
    }
    rows
}

/// Whether local branch `name` is named after issue `key`.
pub fn links_branch(fs: &ForgeState, name: &str, key: &str) -> bool {
    fs.branch_issue(name).as_deref() == Some(key)
}

/// The local branches named after issue `key`.
pub fn linked_branches<'a>(snap: &'a Snapshot, fs: &ForgeState, key: &str) -> Vec<&'a BranchInfo> {
    snap.locals.iter().filter(|b| links_branch(fs, &b.name, key)).collect()
}

/// The details a newly opened panel starts with: loading, or none for a tracker without
/// issue details in Glowfly.
pub fn initial_details(fs: &ForgeState) -> Details {
    if fs.tracker.as_ref().is_some_and(|t| issues_in_app(t.provider)) { Details::Loading } else { Details::Unsupported }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::forge_filters::IssueList;
    use crate::state::{ForgeState, Status};
    use glowfly_core::forge::{Account, FilterChoice, IssueFacets};

    fn github() -> Target {
        Target { provider: Provider::GitHub, host: "github.com".into(), owner: "o".into(), project: String::new(), repo: "r".into() }
    }

    fn jira() -> Target {
        Target::jira(&Account { provider: Provider::JiraCloud, host: "acme.atlassian.net".into(), org: String::new() }, "PROJ")
    }

    pub(crate) fn issue(key: &str, title: &str) -> Issue {
        Issue {
            key: key.into(),
            title: title.into(),
            state: "open".into(),
            kind: "Bug".into(),
            assignee: Some("ada".into()),
            url: format!("https://github.com/o/r/issues/{key}"),
            facets: IssueFacets { labels: vec!["bug".into(), "ui".into()], milestone: Some("v1".into()) },
        }
    }

    fn details(issue: Issue) -> IssueDetails {
        IssueDetails {
            issue,
            body: "Body".into(),
            author: "grace".into(),
            created: 1_767_348_000,
            assignees: vec!["ada".into(), "linus".into()],
            priority: None,
            epic: Some("PROJ-1  Accounts".into()),
            comments: Vec::new(),
        }
    }

    #[test]
    fn bodies_are_markdown_except_on_azure_boards_and_jira() {
        let fs = ForgeState::new(Some(github()), "origin".into(), Some(github()));
        assert!(IssueView::new(issue("3", "a"), &fs).unwrap().markdown());
        let fs = ForgeState::new(Some(github()), "origin".into(), Some(jira()));
        assert!(!IssueView::new(issue("PROJ-3", "a"), &fs).unwrap().markdown());
    }

    #[test]
    fn finds_listed_issues_in_order() {
        let mut fs = ForgeState::new(Some(github()), "origin".into(), Some(github()));
        fs.cache.insert("3".into(), issue("3", "cached"));
        fs.cache.insert("1".into(), issue("1", "cached one"));
        fs.filters.issues = Some(IssueList {
            choice: FilterChoice::default(),
            conditions: Default::default(),
            items: vec![issue("2", "searched"), issue("1", "searched one")],
            status: Status::Ok,
        });
        fs.my_issues = vec![issue("1", "mine")];
        assert_eq!(find_listed(&fs, "1").map(|i| i.title.as_str()), Some("mine"));
        assert_eq!(find_listed(&fs, "2").map(|i| i.title.as_str()), Some("searched"));
        assert_eq!(find_listed(&fs, "3").map(|i| i.title.as_str()), Some("cached"));
        assert!(find_listed(&fs, "4").is_none());
        let bare = listed_or_bare(&fs, "4");
        assert_eq!((bare.title.as_str(), bare.url.as_str()), ("", "https://github.com/o/r/issues/4"));
    }

    #[test]
    fn meta_rows_per_tracker() {
        let settings = Settings { relative_dates: false, date_locale: "en_US".into(), ..Settings::default() };
        let fs = ForgeState::new(Some(github()), "origin".into(), Some(github()));
        let mut v = IssueView::new(issue("12", "Crash"), &fs).unwrap();
        assert_eq!(v.details, Details::Loading);
        let labels = |rows: Vec<(&'static str, String)>| rows.into_iter().map(|(l, _)| l).collect::<Vec<_>>();
        assert_eq!(labels(meta_rows(&v, &settings, 0)), ["State", "Type", "Assignees", "Labels", "Milestone"]);
        assert_eq!(v.kind_line(&settings, 0), "Bug");
        assert_eq!(v.created(), None);
        v.details = Details::Loaded(Box::new(details(v.issue.clone())));
        let rows = meta_rows(&v, &settings, 0);
        assert_eq!(
            labels(rows.clone()),
            ["State", "Type", "Assignees", "Labels", "Milestone", "Created by", "Created"],
            "no epic off Jira"
        );
        assert_eq!(rows[2].1, "ada, linus");
        assert_eq!(rows[6].1, "January 2, 2026");
        assert_eq!(v.kind_line(&settings, 0), "Bug · opened by grace · January 2, 2026");
        assert_eq!(v.created(), Some(1_767_348_000));

        let fs = ForgeState::new(None, String::new(), Some(jira()));
        let mut v = IssueView::new(Issue { key: "PROJ-7".into(), state: "In Progress".into(), ..issue("PROJ-7", "Login") }, &fs).unwrap();
        assert_eq!(v.mention, "PROJ-7");
        v.details = Details::Loaded(Box::new(details(v.issue.clone())));
        let rows = meta_rows(&v, &settings, 0);
        assert_eq!(labels(rows.clone()), ["Status", "Type", "Assignees", "Labels", "Fix version", "Epic", "Created by", "Created"]);
        assert_eq!(rows[5].1, "PROJ-1  Accounts");
        v.issue.assignee = None;
        v.details = Details::Loading;
        assert_eq!(meta_rows(&v, &settings, 0)[2].1, "Unassigned");

        let tracker =
            |p: Provider, host: &str| ForgeState::new(None, String::new(), Some(Target { provider: p, host: host.into(), ..github() }));
        let fs = tracker(Provider::Bitbucket, "bitbucket.org");
        let mut v = IssueView::new(Issue { kind: "bug".into(), ..issue("3", "Crash") }, &fs).unwrap();
        v.details = Details::Loaded(Box::new(IssueDetails { priority: Some("major".into()), epic: None, ..details(v.issue.clone()) }));
        let rows = meta_rows(&v, &settings, 0);
        assert_eq!(labels(rows.clone()), ["State", "Kind", "Priority", "Assignees", "Labels", "Milestone", "Created by", "Created"]);
        assert_eq!((rows[1].1.as_str(), rows[2].1.as_str()), ("Bug", "Major"));

        let fs = tracker(Provider::AzureDevOps, "dev.azure.com");
        let mut v = IssueView::new(Issue { kind: "User Story".into(), ..issue("5", "Login") }, &fs).unwrap();
        v.issue.facets.milestone = None;
        v.details = Details::Loaded(Box::new(IssueDetails { epic: Some("1  Accounts".into()), ..details(v.issue.clone()) }));
        let rows = meta_rows(&v, &settings, 0);
        assert_eq!(labels(rows.clone()), ["State", "Type", "Assigned to", "Tags", "Parent", "Created by", "Created"]);
        assert_eq!((v.mention.as_str(), rows[4].1.as_str()), ("AB#5", "1  Accounts"));

        let fs = tracker(Provider::Trello, "trello.com");
        let mut v = IssueView::new(Issue { state: "Doing".into(), kind: "Website".into(), ..issue("AbCd1234", "Fix") }, &fs).unwrap();
        v.issue.facets.milestone = None;
        v.details = Details::Loaded(Box::new(details(v.issue.clone())));
        let rows = meta_rows(&v, &settings, 0);
        assert_eq!(labels(rows.clone()), ["List", "Board", "Members", "Labels", "Created by", "Created"], "no epic on Trello");
        assert_eq!(rows[1].1, "Website");
        assert_eq!(v.kind_line(&settings, 0), "Card on Website · opened by grace · January 2, 2026");
    }

    #[test]
    fn closed_states() {
        let fs = ForgeState::new(Some(github()), "origin".into(), Some(github()));
        for (state, closed) in [("open", false), ("closed (not planned)", true), ("wontfix", true), ("Archived", true), ("Doing", false)] {
            let v = IssueView::new(Issue { state: state.into(), ..issue("1", "x") }, &fs).unwrap();
            assert_eq!(v.closed(), closed, "{state}");
        }
    }

    #[test]
    fn branches_named_after_the_issue() {
        let fs = ForgeState::new(Some(github()), "origin".into(), Some(github()));
        assert!(links_branch(&fs, "12-crash", "12"));
        assert!(!links_branch(&fs, "12-crash", "1"));
        assert!(!links_branch(&fs, "main", "12"));
        let fs = ForgeState::new(None, String::new(), Some(jira()));
        assert!(links_branch(&fs, "feature/PROJ-7-x", "PROJ-7"));
        assert!(!links_branch(&fs, "feature/PROJ-7-x", "PROJ-8"));
    }

    #[test]
    fn details_only_where_the_tracker_has_them() {
        let other = |p: Provider| Target { provider: p, host: "example.com".into(), ..github() };
        for (p, supported) in [
            (Provider::GitHub, true),
            (Provider::GitLab, true),
            (Provider::Bitbucket, true),
            (Provider::Trello, true),
            (Provider::AzureDevOps, true),
            (Provider::BitbucketDataCenter, false),
        ] {
            let fs = ForgeState::new(None, String::new(), Some(other(p)));
            assert_eq!(initial_details(&fs) == Details::Loading, supported, "{p:?}");
            assert_eq!(initial_details(&fs) == Details::Unsupported, !supported, "{p:?}");
        }
        assert!(IssueView::new(issue("1", "x"), &ForgeState::new(Some(github()), "origin".into(), None)).is_none());
    }
}
