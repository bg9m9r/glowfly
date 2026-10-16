//! Pull request and issue filters: the conditions a filter holds, the built-in filters, the
//! filters a user saves for a repository (kept in settings), matching loaded pull requests
//! (`matching`) and each service's issue search and pickers (`github`, `gitlab`, `bitbucket`,
//! `azure`, `jira`).

use super::{Auth, Issue, Target, links, service};
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};

pub(super) mod azure;
pub(super) mod bitbucket;
pub(super) mod github;
pub(super) mod gitlab;
pub(super) mod jira;
pub mod matching;

/// Which sidebar section a filter belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum FilterKind {
    #[default]
    Prs,
    Issues,
}

/// A person condition. `Nobody` only applies to the assignee.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Who {
    #[default]
    Anyone,
    Me,
    Nobody,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Review {
    #[default]
    Any,
    /// A review is asked of the signed-in user, who hasn't given one yet.
    WaitingForMe,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Draft {
    #[default]
    Any,
    /// Ready for review only.
    Ready,
    /// Drafts only.
    Drafts,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Milestone {
    #[default]
    Any,
    /// Items without a milestone.
    None,
    /// `key` is the GitHub milestone number, and the title everywhere else.
    Named { key: String, title: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Sprint {
    #[default]
    Any,
    /// Any active or future sprint.
    Open,
    Named {
        id: u64,
        name: String,
    },
}

/// What a filter asks for. Every field at its default matches everything.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Conditions {
    pub author: Who,
    pub assignee: Who,
    pub review: Review,
    pub mentions_me: bool,
    /// Every one must be present (compared ignoring case).
    pub labels: Vec<String>,
    pub milestone: Milestone,
    pub sprint: Sprint,
    pub draft: Draft,
    /// The target branch, exactly; empty for any.
    pub base: String,
}

/// The filters every repository has.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Builtin {
    #[default]
    AllPrs,
    MyPrs,
    NeedsMyReview,
    PrsAssignedToMe,
    IssuesAssignedToMe,
    IssuesCreatedByMe,
    IssuesMentioningMe,
}

impl Builtin {
    pub const PRS: [Builtin; 4] = [Builtin::AllPrs, Builtin::MyPrs, Builtin::NeedsMyReview, Builtin::PrsAssignedToMe];
    pub const ISSUES: [Builtin; 3] = [Builtin::IssuesAssignedToMe, Builtin::IssuesCreatedByMe, Builtin::IssuesMentioningMe];

    pub fn kind(self) -> FilterKind {
        match self {
            Builtin::AllPrs | Builtin::MyPrs | Builtin::NeedsMyReview | Builtin::PrsAssignedToMe => FilterKind::Prs,
            Builtin::IssuesAssignedToMe | Builtin::IssuesCreatedByMe | Builtin::IssuesMentioningMe => FilterKind::Issues,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Builtin::AllPrs => "All",
            Builtin::MyPrs => "Mine",
            Builtin::NeedsMyReview => "Needs my review",
            Builtin::PrsAssignedToMe | Builtin::IssuesAssignedToMe => "Assigned to me",
            Builtin::IssuesCreatedByMe => "Created by me",
            Builtin::IssuesMentioningMe => "Mentioning me",
        }
    }

    pub fn conditions(self) -> Conditions {
        let c = Conditions::default();
        match self {
            Builtin::AllPrs => c,
            Builtin::MyPrs | Builtin::IssuesCreatedByMe => Conditions { author: Who::Me, ..c },
            Builtin::NeedsMyReview => Conditions { review: Review::WaitingForMe, ..c },
            Builtin::PrsAssignedToMe | Builtin::IssuesAssignedToMe => Conditions { assignee: Who::Me, ..c },
            Builtin::IssuesMentioningMe => Conditions { mentions_me: true, ..c },
        }
    }

    /// What a section shows until the user chooses: every pull request, and assigned issues.
    pub fn default_for(kind: FilterKind) -> Builtin {
        match kind {
            FilterKind::Prs => Builtin::AllPrs,
            FilterKind::Issues => Builtin::IssuesAssignedToMe,
        }
    }
}

/// The filter a section shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FilterChoice {
    Builtin(Builtin),
    /// A saved filter, by its id.
    Saved(u64),
}

impl Default for FilterChoice {
    fn default() -> Self {
        FilterChoice::Builtin(Builtin::AllPrs)
    }
}

impl FilterChoice {
    pub fn default_for(kind: FilterKind) -> FilterChoice {
        FilterChoice::Builtin(Builtin::default_for(kind))
    }
}

/// A filter the user named and saved for a repository.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SavedFilter {
    pub id: u64,
    pub name: String,
    pub kind: FilterKind,
    pub conditions: Conditions,
}

/// What pull request filters match on. Ids are the service's stable ones for people (compared
/// with `Me::id`), not display names.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PrFacets {
    pub author_id: String,
    pub assignee_ids: Vec<String>,
    /// Every listed or requested reviewer.
    pub reviewer_ids: Vec<String>,
    /// Reviewers asked for who haven't voted yet (or were asked again).
    pub awaiting_ids: Vec<String>,
    pub labels: Vec<String>,
    /// The milestone's title.
    pub milestone: Option<String>,
}

/// An issue's labels and milestone (Jira: its first fix version), for tooltips.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct IssueFacets {
    pub labels: Vec<String>,
    pub milestone: Option<String>,
}

/// The signed-in user: `id` is what pull requests carry in their facets (a login, a username,
/// an account uuid or an identity id).
#[derive(Clone, Debug, PartialEq)]
pub struct Me {
    pub id: String,
    pub name: String,
}

/// A milestone, fix version or sprint offered in a picker.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Named {
    pub key: String,
    pub title: String,
}

/// The labels, milestones and sprints a service offers for a filter's pickers.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct FilterOptions {
    pub labels: Vec<String>,
    pub milestones: Vec<Named>,
    pub sprints: Vec<Named>,
}

/// `s` with `quote` and backslashes escaped by a backslash, for a quoted string in a query
/// language (JQL, Bitbucket's `q=`).
pub(crate) fn esc_quoted(s: &str, quote: char) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        if ch == quote || ch == '\\' {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

/// The signed-in user on `t`'s service.
pub fn me(t: &Target, auth: &Auth) -> Result<Me> {
    service(t.provider).me(t, auth)
}

/// Open issues on `t`'s tracker matching `c`. `me`, when known, saves looking the user up.
/// A condition the tracker can't apply is refused before anything is sent.
pub fn find_issues(t: &Target, auth: &Auth, c: &Conditions, me: Option<&Me>) -> Result<Vec<Issue>> {
    if let Some(why) = matching::unsupported(t.provider, FilterKind::Issues, c) {
        return Err(Error::Unsupported(why));
    }
    service(t.provider).find_issues(t, auth, c, me).map(|items| links::issues(t, items))
}

/// The labels, milestones and sprints `t`'s service offers for `kind`'s filters.
pub fn filter_options(t: &Target, auth: &Auth, kind: FilterKind) -> Result<FilterOptions> {
    service(t.provider).filter_options(t, auth, kind)
}

#[cfg(test)]
mod tests;
