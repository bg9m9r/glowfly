//! The status bar's counts, following the sidebar's lists: the pull requests and issues each
//! section shows under its filter (`forge_filters::pr_view` and `issue_view`). Nothing while a
//! filtered list is still loading, isn't supported or is still finding the user's account.

use super::forge_filters::{self, IssueList};
use super::{ForgeState, Status};
use glowfly_core::forge::{FilterChoice, FilterKind};
use glowfly_core::i18n::Lang;
use glowfly_core::settings::RepoForgeFilters;

/// "3 open pull requests" (or merge requests, as the service calls them).
pub fn open_requests_text(t: Lang, noun: &str, n: usize) -> String {
    if noun == "merge request" { t.trn("status.open_merge_requests", n, &[]) } else { t.trn("status.open_pull_requests", n, &[]) }
}

/// "N open pull requests" under the default filter, else "N in <filter>" for the rows the
/// PULL REQUESTS section shows.
pub fn prs_text(t: Lang, fs: &ForgeState, f: &RepoForgeFilters) -> Option<String> {
    let view = forge_filters::pr_view(fs, f);
    let (choice, _, label) = f.conditions(FilterKind::Prs);
    if choice == FilterChoice::default_for(FilterKind::Prs) {
        return Some(open_requests_text(t, fs.pr_noun(), view.rows.len()));
    }
    // No rows and nothing to say about it: unsupported, or still finding the account.
    if view.rows.is_empty() && view.empty.is_none() {
        return None;
    }
    Some(in_filter(t, view.rows.len(), &label))
}

/// "N assigned" under the default filter, else "N in <filter>" once its search has answered.
pub fn issues_text(t: Lang, fs: &ForgeState, f: &RepoForgeFilters) -> Option<String> {
    let view = forge_filters::issue_view(fs, f);
    let (choice, c, label) = f.conditions(FilterKind::Issues);
    if choice == FilterChoice::default_for(FilterKind::Issues) {
        return Some(t.trf("status.assigned", &[("count", &view.rows.len())]));
    }
    let answered = matches!(&fs.filters.issues, Some(IssueList { choice: ch, conditions, status: Status::Ok, .. }) if *ch == choice && *conditions == c);
    answered.then(|| in_filter(t, view.rows.len(), &label))
}

fn in_filter(t: Lang, count: usize, filter: &str) -> String {
    t.trf("status.in_filter", &[("count", &count), ("filter", &filter)])
}

#[cfg(test)]
#[path = "status_counts_tests.rs"]
mod tests;
