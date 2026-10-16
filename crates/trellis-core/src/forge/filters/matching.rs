//! Which conditions each service can apply, the problems that keep a filter from being saved,
//! and matching loaded pull requests against conditions. Pure: nothing here calls a service.

use super::{Conditions, Draft, FilterKind, Me, Milestone, Review, Sprint, Who};
use crate::forge::{Provider, PullRequest};

/// One condition of a filter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Author,
    Assignee,
    Review,
    Mentions,
    Labels,
    Milestone,
    Sprint,
    Draft,
    Base,
}

impl Field {
    pub const ALL: [Field; 9] = [
        Field::Author,
        Field::Assignee,
        Field::Review,
        Field::Mentions,
        Field::Labels,
        Field::Milestone,
        Field::Sprint,
        Field::Draft,
        Field::Base,
    ];

    /// Whether `c` sets this condition (away from "any").
    pub fn is_set(self, c: &Conditions) -> bool {
        match self {
            Field::Author => c.author != Who::Anyone,
            Field::Assignee => c.assignee != Who::Anyone,
            Field::Review => c.review != Review::Any,
            Field::Mentions => c.mentions_me,
            Field::Labels => !c.labels.is_empty(),
            Field::Milestone => c.milestone != Milestone::Any,
            Field::Sprint => c.sprint != Sprint::Any,
            Field::Draft => c.draft != Draft::Any,
            Field::Base => !c.base.trim().is_empty(),
        }
    }

    fn noun(self, p: Provider) -> &'static str {
        match self {
            Field::Author => "author",
            Field::Assignee => "assignee",
            Field::Review => "review",
            Field::Mentions => "mention",
            Field::Labels if p == Provider::AzureDevOps => "tag",
            Field::Labels => "label",
            Field::Milestone if p.is_jira() => "fix version",
            Field::Milestone => "milestone",
            Field::Sprint => "sprint",
            Field::Draft => "draft state",
            Field::Base => "target branch",
        }
    }
}

fn github_family(p: Provider) -> bool {
    matches!(p, Provider::GitHub | Provider::GitHubEnterprise)
}

fn gitlab_family(p: Provider) -> bool {
    matches!(p, Provider::GitLab | Provider::GitLabSelfManaged)
}

fn supports(p: Provider, kind: FilterKind, f: Field) -> bool {
    let (gh, gl) = (github_family(p), gitlab_family(p));
    match kind {
        FilterKind::Prs => {
            let code_host = !p.is_tracker_only();
            match f {
                Field::Author | Field::Review | Field::Draft | Field::Base => code_host,
                Field::Assignee | Field::Milestone => gh || gl,
                Field::Labels => gh || gl || p == Provider::AzureDevOps,
                Field::Mentions | Field::Sprint => false,
            }
        }
        FilterKind::Issues => {
            // Trello's cards can't be searched by condition.
            if !p.has_issues() || p == Provider::Trello {
                return false;
            }
            match f {
                Field::Author | Field::Assignee => true,
                Field::Mentions => gh || p == Provider::AzureDevOps,
                Field::Labels => gh || gl || p == Provider::AzureDevOps || p.is_jira(),
                Field::Milestone => gh || gl || p == Provider::Bitbucket || p.is_jira(),
                Field::Sprint => p.is_jira(),
                Field::Review | Field::Draft | Field::Base => false,
            }
        }
    }
}

/// "pull request" / "merge request", or "issue" / "work item".
pub fn item_noun(p: Provider, kind: FilterKind) -> &'static str {
    match kind {
        FilterKind::Prs => p.pr_noun(),
        FilterKind::Issues if p == Provider::AzureDevOps => "work item",
        FilterKind::Issues if p == Provider::Trello => "card",
        FilterKind::Issues => "issue",
    }
}

/// Why `p` can't filter `kind` by `f`, or None when it can.
pub fn unsupported_field(p: Provider, kind: FilterKind, f: Field) -> Option<String> {
    if supports(p, kind, f) {
        return None;
    }
    if kind == FilterKind::Issues && !p.has_issues() {
        return Some(format!("{} has no issue tracker.", p.name()));
    }
    if kind == FilterKind::Prs && p.is_tracker_only() {
        return Some(format!("{} doesn't host pull requests.", p.name()));
    }
    Some(format!("{} can't filter {}s by {}.", p.name(), item_noun(p, kind), f.noun(p)))
}

/// The first condition `c` sets that `p` can't apply to `kind`, as the reason.
pub fn unsupported(p: Provider, kind: FilterKind, c: &Conditions) -> Option<String> {
    Field::ALL.into_iter().filter(|f| f.is_set(c)).find_map(|f| unsupported_field(p, kind, f))
}

/// Everything that keeps `c` from working on `p`: conditions it can't apply and labels it
/// can't search for.
pub fn problems(p: Provider, kind: FilterKind, c: &Conditions) -> Vec<String> {
    let mut out: Vec<String> = unsupported(p, kind, c).into_iter().collect();
    if c.labels.iter().any(|l| l.trim().is_empty()) {
        out.push(format!("A {} is empty.", Field::Labels.noun(p)));
    }
    if github_family(p) || gitlab_family(p) {
        for l in c.labels.iter().filter(|l| l.contains(',')) {
            out.push(format!("{} can't search for a label that contains a comma: \u{201c}{l}\u{201d}.", p.name()));
        }
    }
    out
}

/// Whether `c` needs to know who the signed-in user is.
pub fn needs_me(kind: FilterKind, c: &Conditions) -> bool {
    c.author == Who::Me || c.assignee == Who::Me || (kind == FilterKind::Prs && c.review == Review::WaitingForMe) || c.mentions_me
}

fn same_id(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

fn same_text(a: &str, b: &str) -> bool {
    a.trim().to_lowercase() == b.trim().to_lowercase()
}

fn matches(pr: &PullRequest, c: &Conditions, me: Option<&Me>) -> bool {
    let f = &pr.facets;
    let me_id = me.map(|m| m.id.as_str()).unwrap_or_default();
    let author = match c.author {
        Who::Anyone => true,
        Who::Me => same_id(&f.author_id, me_id),
        Who::Nobody => false,
    };
    let assignee = match c.assignee {
        Who::Anyone => true,
        Who::Me => f.assignee_ids.iter().any(|a| same_id(a, me_id)),
        Who::Nobody => f.assignee_ids.is_empty(),
    };
    let review = match c.review {
        Review::Any => true,
        Review::WaitingForMe => f.awaiting_ids.iter().any(|a| same_id(a, me_id)),
    };
    let labels = c.labels.iter().all(|l| f.labels.iter().any(|have| same_text(have, l)));
    let milestone = match &c.milestone {
        Milestone::Any => true,
        Milestone::None => f.milestone.is_none(),
        Milestone::Named { title, .. } => f.milestone.as_deref().is_some_and(|m| same_text(m, title)),
    };
    let draft = match c.draft {
        Draft::Any => true,
        Draft::Ready => !pr.draft,
        Draft::Drafts => pr.draft,
    };
    let base = c.base.trim().is_empty() || pr.target == c.base.trim();
    author && assignee && review && labels && milestone && draft && base
}

/// The pull requests in `prs` that match `c`, in order; None while `c` needs the signed-in
/// user and `me` isn't known yet.
pub fn matching_prs<'a>(prs: &'a [PullRequest], c: &Conditions, me: Option<&Me>) -> Option<Vec<&'a PullRequest>> {
    if needs_me(FilterKind::Prs, c) && me.is_none() {
        return None;
    }
    Some(prs.iter().filter(|p| matches(p, c, me)).collect())
}

/// Whether `p`'s issue searches name the signed-in user by their id, so a search on "me" needs
/// them looked up first (Azure DevOps and Jira queries say "me" themselves).
pub fn issue_search_names_user(p: Provider) -> bool {
    github_family(p) || gitlab_family(p) || p == Provider::Bitbucket
}

/// Whether issue searches name a milestone by its number (GitHub), so only listed ones can be
/// picked; elsewhere the title is the key.
pub fn milestones_by_number(p: Provider) -> bool {
    github_family(p)
}

/// What the service calls a milestone.
pub fn milestone_noun(p: Provider) -> &'static str {
    if p.is_jira() { "Fix version" } else { "Milestone" }
}

/// What the service calls labels.
pub fn labels_noun(p: Provider) -> &'static str {
    if p == Provider::AzureDevOps { "Tags" } else { "Labels" }
}
