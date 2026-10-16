//! The issue panel (update/issue.rs, ui/issue/): a copy of the listed issue, the tracker it
//! came from, the details loaded from the service, and the texts that describe them. Pure: the
//! details are loaded by `Effect::Issue`.

use super::ForgeState;
use trellis_core::forge::{Issue, IssueDetails, Provider, Target, issues_in_app};
use trellis_core::repo::{BranchInfo, Snapshot};
use trellis_core::settings::Settings;

/// The service's details of the issue.
#[derive(Clone, PartialEq, Debug)]
pub enum Details {
    /// The tracker has no issue details in Trellis.
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

    pub fn provider(&self) -> Provider {
        self.target.provider
    }

    /// Closed or done: the icon turns purple.
    pub fn closed(&self) -> bool {
        let s = self.issue.state.to_lowercase();
        s.starts_with("closed") || matches!(s.as_str(), "done" | "resolved")
    }

    /// The issue's type, else the noun: "Bug", "Issue".
    pub fn kind(&self) -> String {
        if self.issue.kind.trim().is_empty() { capitalized(self.noun) } else { self.issue.kind.clone() }
    }

    /// "Bug · opened by ada · 3 days ago"; only the type until the details load.
    pub fn kind_line(&self, settings: &Settings, now: i64, offset: i32) -> String {
        let mut line = self.kind();
        if let Some(d) = self.loaded() {
            if !d.author.is_empty() {
                line.push_str(&format!(" · opened by {}", d.author));
            }
            if d.created > 0 {
                line.push_str(&format!(" · {}", settings.format_time(d.created, now, offset)));
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

/// "Status" on Jira, "State" on code hosts.
pub fn state_label(p: Provider) -> &'static str {
    if p.is_jira() { "Status" } else { "State" }
}

/// "Fix version" on Jira, "Milestone" on code hosts.
pub fn milestone_label(p: Provider) -> &'static str {
    if p.is_jira() { "Fix version" } else { "Milestone" }
}

/// The side panel's Details rows, as (label, text). Labels are listed comma-separated (the view
/// draws them as chips); rows without a value are left out, apart from the assignees.
pub fn meta_rows(v: &IssueView, settings: &Settings, now: i64, offset: i32) -> Vec<(&'static str, String)> {
    let p = v.provider();
    let i = &v.issue;
    let d = v.loaded();
    let assignees = match d {
        Some(d) => d.assignees.join(", "),
        None => i.assignee.clone().unwrap_or_default(),
    };
    let mut rows = vec![
        (state_label(p), i.state.clone()),
        ("Type", v.kind()),
        ("Assignees", if assignees.is_empty() { "Unassigned".to_owned() } else { assignees }),
    ];
    if !i.facets.labels.is_empty() {
        rows.push(("Labels", i.facets.labels.join(", ")));
    }
    if let Some(m) = i.facets.milestone.as_ref().filter(|m| !m.is_empty()) {
        rows.push((milestone_label(p), m.clone()));
    }
    if let Some(e) = d.and_then(|d| d.epic.clone()).filter(|_| p.is_jira()) {
        rows.push(("Epic", e));
    }
    if let Some(d) = d {
        if !d.author.is_empty() {
            rows.push(("Created by", d.author.clone()));
        }
        if d.created > 0 {
            rows.push(("Created", settings.format_time(d.created, now, offset)));
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
/// issue details in Trellis.
pub fn initial_details(fs: &ForgeState) -> Details {
    if fs.tracker.as_ref().is_some_and(|t| issues_in_app(t.provider)) { Details::Loading } else { Details::Unsupported }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::forge_filters::IssueList;
    use crate::state::{ForgeState, Status};
    use trellis_core::forge::{Account, FilterChoice, IssueFacets};

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
            epic: Some("PROJ-1  Accounts".into()),
            comments: Vec::new(),
        }
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
        assert_eq!(labels(meta_rows(&v, &settings, 0, 0)), ["State", "Type", "Assignees", "Labels", "Milestone"]);
        assert_eq!(v.kind_line(&settings, 0, 0), "Bug");
        v.details = Details::Loaded(Box::new(details(v.issue.clone())));
        let rows = meta_rows(&v, &settings, 0, 0);
        assert_eq!(
            labels(rows.clone()),
            ["State", "Type", "Assignees", "Labels", "Milestone", "Created by", "Created"],
            "no epic off Jira"
        );
        assert_eq!(rows[2].1, "ada, linus");
        assert!(v.kind_line(&settings, 0, 0).starts_with("Bug · opened by grace · "));

        let fs = ForgeState::new(None, String::new(), Some(jira()));
        let mut v = IssueView::new(Issue { key: "PROJ-7".into(), state: "In Progress".into(), ..issue("PROJ-7", "Login") }, &fs).unwrap();
        assert_eq!(v.mention, "PROJ-7");
        v.details = Details::Loaded(Box::new(details(v.issue.clone())));
        let rows = meta_rows(&v, &settings, 0, 0);
        assert_eq!(labels(rows.clone()), ["Status", "Type", "Assignees", "Labels", "Fix version", "Epic", "Created by", "Created"]);
        assert_eq!(rows[5].1, "PROJ-1  Accounts");
        v.issue.assignee = None;
        v.details = Details::Loading;
        assert_eq!(meta_rows(&v, &settings, 0, 0)[2].1, "Unassigned");
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
        for (p, supported) in [(Provider::GitHub, true), (Provider::GitLab, true), (Provider::Bitbucket, false), (Provider::Trello, false)]
        {
            let fs = ForgeState::new(None, String::new(), Some(other(p)));
            assert_eq!(initial_details(&fs) == Details::Loading, supported, "{p:?}");
            assert_eq!(initial_details(&fs) == Details::Unsupported, !supported, "{p:?}");
        }
        assert!(IssueView::new(issue("1", "x"), &ForgeState::new(Some(github()), "origin".into(), None)).is_none());
    }
}
