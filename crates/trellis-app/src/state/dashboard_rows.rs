//! The dashboard's rows (pure), shared by the view and the tests: per service account, per
//! repository, the pull requests or issues the switch and the filter keep.

use super::dashboard::{AccountRow, AccountState, DashShow, Dashboard, TargetData, pr_label, target_name};
use trellis_core::forge::dashboard::mine_and_review;
use trellis_core::forge::{self, Issue, Provider, PullRequest, Target};
use trellis_core::matching::contains_lower;

#[derive(Clone, Copy)]
pub enum Row<'a> {
    Pr(&'a PullRequest),
    Issue(&'a Issue),
}

pub struct RepoSection<'a> {
    pub data: &'a TargetData,
    pub rows: Vec<Row<'a>>,
    /// Why this repository's pull requests (or, under Assigned, issues) couldn't be read.
    pub error: Option<&'a str>,
}

pub struct ServiceSection<'a> {
    pub account: &'a AccountRow,
    pub repos: Vec<RepoSection<'a>>,
}

/// "#12", "!7", or an issue's mention ("#3", "AB#9", "PROJ-3"; a Trello card's id).
pub fn row_label(target: &Target, row: Row) -> String {
    match row {
        Row::Pr(pr) => pr_label(target, pr.id),
        Row::Issue(i) => forge::mention(target.provider.ref_style(), &i.key),
    }
}

/// An issue row's text: its mention and title; a Trello card's title alone.
pub fn issue_text(target: &Target, issue: &Issue) -> String {
    if target.provider == Provider::Trello {
        issue.title.clone()
    } else {
        format!("{}  {}", forge::mention(target.provider.ref_style(), &issue.key), issue.title)
    }
}

/// What a switch shows when nothing is left.
pub fn empty_text(show: DashShow) -> &'static str {
    match show {
        DashShow::Mine => "None of your pull requests are open.",
        DashShow::Review => "No reviews are waiting for you.",
        DashShow::Assigned => "Nothing is assigned to you.",
    }
}

fn keeps(data: &TargetData, row: Row, filter_lower: &str) -> bool {
    if filter_lower.is_empty() {
        return true;
    }
    let label = row_label(&data.target, row);
    let mut fields: Vec<&str> = vec![&label];
    match row {
        Row::Pr(pr) => fields.extend([pr.title.as_str(), pr.author.as_str(), pr.source.as_str()]),
        Row::Issue(i) => fields.extend([i.title.as_str()].into_iter().chain(i.assignee.as_deref())),
    }
    let name = target_name(&data.target);
    fields.push(&name);
    fields.extend(data.folder_names());
    fields.into_iter().any(|f| contains_lower(f, filter_lower))
}

/// One repository's rows under `show`, and its error.
fn repo_rows<'a>(data: &'a TargetData, account: &'a AccountRow, show: DashShow, filter_lower: &str) -> RepoSection<'a> {
    let (rows, error): (Vec<Row<'a>>, Option<&'a str>) = match show {
        DashShow::Mine | DashShow::Review => match (&data.prs, &account.me) {
            (Some(Err(e)), _) => (Vec::new(), Some(e.as_str())),
            (Some(Ok(prs)), Some(me)) => {
                let (mine, review) = mine_and_review(prs, me);
                let picked = if show == DashShow::Mine { mine } else { review };
                (picked.into_iter().map(Row::Pr).collect(), None)
            }
            _ => (Vec::new(), None),
        },
        DashShow::Assigned => match &data.issues {
            Some(Err(e)) => (Vec::new(), Some(e.as_str())),
            Some(Ok(items)) => (items.iter().map(Row::Issue).collect(), None),
            None => (Vec::new(), None),
        },
    };
    let rows = rows.into_iter().filter(|r| keeps(data, *r, filter_lower)).collect();
    RepoSection { data, rows, error }
}

/// The body: accounts that answered, each with the repositories the switch is about. While a
/// filter is typed, repositories with nothing to show are left out, and so are empty accounts.
pub fn sections<'a>(d: &'a Dashboard, show: DashShow, filter_lower: &str) -> Vec<ServiceSection<'a>> {
    let wanted = |t: &TargetData| if show == DashShow::Assigned { t.wants_issues } else { t.wants_prs };
    d.accounts
        .iter()
        .filter(|a| a.state == AccountState::Ok)
        .map(|account| {
            let repos = d
                .targets
                .iter()
                .filter(|t| wanted(t) && t.target.account() == account.account)
                .map(|t| repo_rows(t, account, show, filter_lower))
                .filter(|r| filter_lower.is_empty() || !r.rows.is_empty() || r.error.is_some())
                .collect();
            ServiceSection { account, repos }
        })
        .filter(|s| !s.repos.is_empty())
        .collect()
}

/// How many rows each switch (Mine, Review requested, Assigned) shows with this filter.
pub fn counts(d: &Dashboard, filter_lower: &str) -> [usize; 3] {
    DashShow::ALL.map(|show| sections(d, show, filter_lower).iter().flat_map(|s| &s.repos).map(|r| r.rows.len()).sum())
}
