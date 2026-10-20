//! The main window in German with no English left: the graph header and its notes, the left
//! panel's section headers, filter hint and empty lines, the WIP row and relative dates (the hunk
//! buttons and the menus are in `snapshot_tests_language_menus`). Painted text has no
//! accessibility node, so every painted galley is read as well as every node, and checked
//! against the English these areas used to show.

use super::snapshot_tests::{NOW, harness, remove_fixture};
use super::snapshot_tests_forge_filters::target;
use super::snapshot_tests_language_check::{self as check, matches, starred};
use super::snapshot_tests_remotes::with_remotes;
use crate::msg::Msg;
use crate::state::{AppState, ForgeState, Status};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use std::path::PathBuf;
use trellis_core::forge::Provider;
use trellis_core::i18n::Lang;
use trellis_core::settings::ThemeChoice;

pub(super) type H = Harness<'static, (AppState, Vec<Msg>)>;

/// What this item's areas showed in English. A `*` stands for a name or number ("Merge into *").
const ENGLISH: &[&str] = &[
    // Graph header and history notes.
    "Branch / tag",
    "Graph",
    "Message",
    "Changes",
    "Commit",
    "Date",
    "Load more",
    "Fetch full history",
    "Full history",
    "Shallow clone",
    "newest * commits",
    "Loading more commits…",
    // Left panel.
    "Filter (branches, PRs, issues)",
    "Local",
    "Remote",
    "Pull requests",
    "Merge requests",
    "Issues",
    "Git Flow",
    "Tags",
    "Stashes",
    "Worktrees",
    "Submodules",
    "Local actions",
    "Remote actions",
    "Pull requests actions",
    "Merge requests actions",
    "Issues actions",
    "Git Flow actions",
    "Tags actions",
    "Stashes actions",
    "Worktrees actions",
    "Submodules actions",
    "More actions",
    "Pull requests filter: *",
    "Merge requests filter: *",
    "Issues filter: *",
    "Choose which pull requests to show",
    "Choose which merge requests to show",
    "Choose which issues to show",
    "New filter…",
    "New branch",
    "Add remote",
    "Create pull request",
    "Create merge request",
    "Create issue",
    "New tag",
    "New annotated tag",
    "Stash changes",
    "Stash staged changes",
    "Add worktree",
    "Add worktree…",
    "Add submodule",
    "Add submodule…",
    "Start Git Flow branch",
    "No worktrees",
    "No submodules",
    "Prune missing worktrees",
    "No branches yet. Fetch this remote to load them.",
    "worktree",
    "Uncommitted changes",
    // Branch menus.
    "Checkout",
    "Checkout *",
    "Open worktree",
    "Create branch here…",
    "Create worktree from this branch…",
    "Rename…",
    "Set upstream…",
    "Unset upstream",
    "Check for conflicts",
    "Check for conflicts with *",
    "Merge into *",
    "Rebase * onto this branch",
    "Interactive rebase * onto this branch…",
    "Fast-forward",
    "Fast-forward to *",
    "Fast-forward * to this branch",
    "Push",
    "Push to",
    "* (upstream)",
    "Compare with *",
    "Set as upstream of *",
    "Create pull request…",
    "Push and create a pull request…",
    "Push * and create a pull request…",
    "Create pull request from * into this branch…",
    "Name",
    "Link",
    "Copy",
    "Delete…",
    "Force delete…",
    "Delete together with *",
    "Delete from remote…",
    "Show in graph",
    "Hide in graph",
    "Show only this",
    "Stop showing only this",
    "Showing only *",
    "Show the rest",
    // Tag menu.
    "Annotate…",
    "All remotes",
    "All remotes…",
    "Delete locally…",
    "Delete from remote",
    "Branch *",
    "Tag *",
    // Commit menu.
    "Create tag here…",
    "Create worktree from this commit…",
    "Cherry-pick",
    "Cherry-pick merge commit…",
    "Revert",
    "Revert merge commit…",
    "Rebase * onto this commit",
    "Interactive rebase * onto this commit…",
    "Reset * to this commit",
    "Reset to this commit",
    "Soft: keep changes staged",
    "Mixed: keep files, unstage",
    "Hard: reset files too…",
    "Changes are stashed and Undo restores them",
    "Interactive rebase from here…",
    "Reword…",
    "Squash into parent…",
    "Drop…",
    "Compare with working directory",
    "Create patch…",
    "SHA",
    "Patch",
    // Hunk buttons.
    "Stage hunk",
    "Unstage hunk",
    "Discard hunk",
    "Revert hunk",
    "Stage * line",
    "Stage * lines",
    "Discard * lines",
    "Revert * lines",
    "Updating the diff…",
    // Dates.
    "just now",
    "yesterday",
    "* ago",
    "Updated *",
];

/// The `ENGLISH` entries that are not also German text (German keeps "Commit", "Graph", "Tags").
fn checkable() -> Vec<&'static str> {
    check::checkable(ENGLISH)
}

/// The shown strings that are one of this item's English texts.
fn english_shown(h: &H) -> Vec<String> {
    check::english_shown(h, ENGLISH, 60)
}

/// Fails listing each shown string that is one of this item's English texts.
pub(super) fn assert_no_english(h: &H, what: &str) {
    let found = english_shown(h);
    assert!(found.is_empty(), "{what} still shows English: {found:?}");
}

fn shows(h: &H, text: &str) -> bool {
    check::shown_texts(h).iter().any(|t| t == text)
}

pub(super) fn german(st: &mut AppState) {
    st.settings.language = "de".into();
}

/// The fixture with remotes in German, every left-panel section open.
pub(super) fn panels(name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_remotes(ThemeChoice::Dark, name);
    german(&mut st);
    st.view.sections.tags = true;
    (st, dir)
}

#[test]
fn the_english_list_matches_names_and_counts() {
    assert!(matches("Merge into *", "Merge into main"));
    assert!(!matches("Merge into *", "Merge into "));
    assert!(matches("Rebase * onto this branch", "Rebase main onto this branch"));
    assert!(matches("Pull requests filter: *", "Pull requests filter: Open"));
    assert!(matches("Interactive rebase * onto this branch…", "Interactive rebase main onto this branch…"));
    assert!(!matches("Rebase * onto this branch", "Rebase  onto this branch"));
    assert!(!matches("* ago", "vor 2 Tagen"));
    assert!(matches("* ago", "2 days ago"));
    assert_eq!(starred("Branch {name}"), "Branch *");
    let entries = checkable();
    assert!(!entries.contains(&"Commit") && !entries.contains(&"Branch *"), "German keeps these");
    assert!(entries.contains(&"Local") && entries.contains(&"Stage hunk"));
}

#[test]
fn the_check_catches_the_english_window() {
    let (mut st, dir) = panels("lang-main-english");
    st.settings.language = "en".into();
    st.settings.relative_dates = true;
    let mut h = harness(st, true);
    h.run();
    let found = english_shown(&h);
    for text in ["Branch / tag", "Message", "Local", "Uncommitted changes", "Local actions", "No worktrees", "New branch"] {
        assert!(found.iter().any(|f| f == text), "{text} is caught: {found:?}");
    }
    assert!(found.iter().any(|f| f.ends_with(" days ago")), "relative dates are caught: {found:?}");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn panels_german() {
    let (mut st, dir) = panels("lang-main-panels");
    st.settings.relative_dates = true;
    st.settings.columns.changes = true;
    let newest = st.snap.as_ref().unwrap().commits.iter().map(|c| c.time).max().unwrap();
    let mut h = harness(st, true);
    h.run();
    let de = Lang::De;
    for key in ["graph.header.refs", "graph.header.message", "graph.header.changes", "graph.header.commit", "graph.header.date"] {
        assert!(shows(&h, de.tr(key)), "the {key} title is shown whole");
    }
    for key in [
        "sidebar.local.header",
        "sidebar.remote.header",
        "sidebar.tags.header",
        "sidebar.stashes.header",
        "sidebar.worktrees.header",
        "sidebar.submodules.header",
    ] {
        assert!(shows(&h, de.tr(key)), "the {key} header");
    }
    assert!(shows(&h, de.tr("sidebar.worktrees.empty")) && shows(&h, de.tr("sidebar.submodules.empty")));
    assert!(shows(&h, de.tr("sidebar.remote.no_branches.hint")), "backup has no branches yet");
    let hint = de.tr("sidebar.filter.hint");
    let cut = hint.split(' ').next().unwrap();
    assert!(check::shown_texts(&h).iter().any(|t| t.starts_with(cut)), "the filter field's hint");
    assert!(shows(&h, de.tr("graph.wip.label")), "the WIP row");
    assert!(h.query_by_label(de.tr("details.wip.stage_all.button")).is_some(), "the WIP lists");
    let days = usize::try_from((NOW - newest) / 86_400).unwrap();
    assert!(shows(&h, &de.trn("dates.days_ago", days, &[])), "a German relative date");
    let more = de.trf("sidebar.section.more.name", &[("section", &de.tr("sidebar.local.header"))]);
    assert!(h.query_by_label(&more).is_some(), "the ⋯ is named in German");
    assert!(h.query_by_label(de.tr("sidebar.action.new_branch")).is_some(), "the + is named in German");
    assert_no_english(&h, "the main window");
    h.check_snapshot("panels_german");
    drop(h);
    remove_fixture(&dir);
}

/// The fixture in German with a code host on `provider` and an issue tracker.
fn with_forge(name: &str, provider: Provider, host: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = panels(name);
    let mut f = ForgeState::new(Some(target(provider, host)), "origin".into(), Some(target(provider, host)));
    f.host_status = Status::Ok;
    f.tracker_status = Status::Ok;
    st.forge = Some(f);
    (st, dir)
}

#[test]
fn forge_headers_german() {
    let de = Lang::De;
    let (st, dir) = with_forge("lang-main-github", Provider::GitHub, "github.com");
    let mut h = harness(st, true);
    h.run();
    assert!(shows(&h, de.tr("sidebar.prs.header")), "the pull requests header");
    assert!(shows(&h, de.tr("sidebar.issues.header")), "the issues header");
    let create = de.trf("sidebar.action.create_pr", &[("noun", &de.tr("forge.noun.pull_request"))]);
    assert!(h.query_by_label(&create).is_some(), "the PR section's + is named in German");
    assert_no_english(&h, "a GitHub repository");
    drop(h);
    remove_fixture(&dir);

    let (st, dir) = with_forge("lang-main-gitlab", Provider::GitLab, "gitlab.com");
    let mut h = harness(st, true);
    h.run();
    assert!(shows(&h, de.tr("sidebar.mrs.header")), "the merge requests header");
    assert_no_english(&h, "a GitLab repository");
    drop(h);
    remove_fixture(&dir);
}
