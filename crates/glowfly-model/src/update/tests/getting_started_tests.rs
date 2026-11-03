//! Headless tests for Help > Getting started: opening it (from Help, the palette and the
//! welcome) and closing it, cards ticking themselves off from settings and repository state only,
//! listing the finished cards, each kind of Show me, and hiding the page from the Help menu.

use super::tests::{git, scratch_repo, state};
use super::update;
use crate::effect::{Effect, LoadKind};
use crate::msg::{Msg, RepoMsg};
use crate::onboarding::OnboardingMsg as O;
use crate::state::layout::{ToolView, tool_view};
use crate::state::onboarding::getting_started::{Card, Group, HIDDEN_ID, ShowMe, groups, hidden, progress};
use crate::state::palette::PaletteCommand as C;
use crate::state::palette::{Mode, entries};
use crate::state::{AppState, PrefPage, Selection};
use glowfly_core::forge::{Account, Connection, Provider};
use glowfly_core::repo::{self, LoadOptions};
use glowfly_core::settings::ThemeChoice;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn run(st: &mut AppState, m: O) -> Vec<Effect> {
    update(st, Msg::Onboarding(m), Instant::now())
}

fn saves(fx: &[Effect]) -> bool {
    fx.iter().any(|e| matches!(e, Effect::SaveSettings(_)))
}

fn load(st: &mut AppState, dir: &Path) {
    let snap = Box::new(repo::load(dir, &LoadOptions::all(100)).unwrap());
    st.loading = true;
    let m = Msg::Repo(RepoMsg::RepoLoaded { generation: st.load_gen, kind: LoadKind::Open, result: Ok(snap) });
    update(st, m, Instant::now());
}

/// A state with the scratch repository `name` open (after `prepare` ran in it).
fn opened(name: &str, prepare: impl FnOnce(&Path)) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    prepare(&dir);
    let mut st = state();
    st.settings.welcome_done = true;
    load(&mut st, &dir);
    (st, dir)
}

fn open_cards(st: &AppState, show_finished: bool) -> Vec<Card> {
    groups(st, show_finished).into_iter().flat_map(|(_, cards)| cards.into_iter().map(|(c, _)| c)).collect()
}

#[test]
fn opens_over_the_central_area_and_closes() {
    let (mut st, dir) = opened("gs-open", |_| {});
    st.dashboard.open = true;
    run(&mut st, O::OpenGettingStarted);
    assert!(st.onboarding.getting_started.is_some() && !st.dashboard.open, "it takes the dashboard's place");
    assert!(!st.graph_shown());
    assert_eq!(tool_view(&st), Some(ToolView::GettingStarted), "the details panel becomes its rail");
    run(&mut st, O::ShowFinished(true));
    run(&mut st, O::OpenGettingStarted);
    assert!(st.onboarding.getting_started.as_ref().unwrap().show_finished, "opening it again keeps it as it was");
    run(&mut st, O::CloseGettingStarted);
    assert!(st.onboarding.getting_started.is_none() && st.graph_shown());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_welcome_start_step_finishes_the_welcome() {
    let mut st = state();
    run(&mut st, O::Startup);
    assert!(st.onboarding.welcome.is_some());
    let fx = run(&mut st, O::OpenGettingStarted);
    assert!(st.onboarding.welcome.is_none() && st.settings.welcome_done && saves(&fx), "the welcome counts as seen");
    assert!(st.onboarding.getting_started.is_some(), "without a repository too");
    assert_eq!(tool_view(&st), None, "no repository: no details panel to fold");
}

#[test]
fn the_palette_always_lists_it_and_help_unless_hidden() {
    let mut st = state();
    let listed = |st: &AppState| entries(st).into_iter().any(|e| e.command == C::Onboarding(O::OpenGettingStarted));
    assert!(listed(&st));
    let fx = run(&mut st, O::HideGettingStarted(true));
    assert!(hidden(&st.settings) && saves(&fx));
    assert!(st.toasts.iter().any(|t| t.text == st.settings.lang().tr("getting_started.hidden.toast")), "says where it still is");
    assert!(listed(&st), "the palette still finds it");
    assert!(!saves(&run(&mut st, O::HideGettingStarted(true))), "hiding it twice changes nothing");
    run(&mut st, O::ResetHints);
    assert!(!hidden(&st.settings), "Show tips again brings it back");
    run(&mut st, O::HideGettingStarted(true));
    let fx = run(&mut st, O::HideGettingStarted(false));
    assert!(!st.settings.hints_seen.contains(HIDDEN_ID) && saves(&fx));
}

#[test]
fn cards_tick_off_from_settings() {
    let mut st = state();
    assert_eq!(progress(&st), (0, Card::ALL.len()));
    assert_eq!(groups(&st, false).iter().map(|(g, _)| *g).collect::<Vec<_>>(), Group::ALL, "every group has a card to try");
    assert_eq!(open_cards(&st, false).first(), Some(&Card::Tour), "the tour comes first");
    let account = Account { provider: Provider::GitHub, host: "github.com".into(), org: String::new() };
    st.settings.connections.push(Connection { account, user: "ada".into(), method: "token".into() });
    st.settings.theme = ThemeChoice::Light;
    st.settings.keybindings.insert("palette".into(), vec!["Ctrl+K".into()]);
    st.settings.external_tools.diff_tool = "meld".into();
    let done = [Card::Connect, Card::Theme, Card::Keys, Card::Tools];
    assert!(done.iter().all(|c| c.done(&st)));
    assert_eq!(progress(&st), (4, Card::ALL.len()));
    let listed = open_cards(&st, false);
    assert!(done.iter().all(|c| !listed.contains(c)), "finished cards are left out");
    assert_eq!(open_cards(&st, true), Card::ALL, "unless asked for");
    st.settings.external_tools.diff_tool = "  ".into();
    assert!(!Card::Tools.done(&st), "a blank program isn't a tool");
}

#[test]
fn cards_tick_off_from_the_repository() {
    let prepare = |d: &Path| {
        git(d, &["stash", "-q"]);
        git(d, &["worktree", "add", "-q", "-b", "side", "../gs-ticks-side"]);
    };
    let (st, dir) = opened("gs-ticks", prepare);
    assert!(Card::Stash.done(&st) && Card::Worktrees.done(&st));
    let (fresh, other) = opened("gs-ticks-fresh", |_| {});
    assert!(!Card::Stash.done(&fresh) && !Card::Worktrees.done(&fresh), "one worktree and no stash");
    let _ = std::fs::remove_dir_all(dir.with_file_name("gs-ticks-side"));
    let _ = std::fs::remove_dir_all(dir);
    let _ = std::fs::remove_dir_all(other);
}

#[test]
fn show_me_closes_the_page_and_opens_the_feature() {
    let mut st = state();
    run(&mut st, O::OpenGettingStarted);
    run(&mut st, O::ShowMe(Card::Connect));
    assert!(st.onboarding.getting_started.is_none());
    assert_eq!(st.prefs.as_ref().map(|p| p.page), Some(PrefPage::Integrations));

    let mut st = state();
    run(&mut st, O::OpenGettingStarted);
    assert_eq!(Card::Tour.show_me(&st).err(), Some(st.settings.lang().tr("getting_started.needs_repo")));
    run(&mut st, O::ShowMe(Card::Tour));
    assert!(st.onboarding.getting_started.is_some() && st.onboarding.tour.is_none(), "a card that can't show itself stays put");
    run(&mut st, O::ShowMe(Card::Dashboard));
    assert!(st.dashboard.open && st.onboarding.getting_started.is_none());
}

#[test]
fn show_me_in_a_repository() {
    let (mut st, dir) = opened("gs-show", |_| {});
    run(&mut st, O::OpenGettingStarted);
    run(&mut st, O::ShowMe(Card::Tour));
    assert!(st.onboarding.tour.is_some() && st.onboarding.getting_started.is_none());
    run(&mut st, O::CloseTour);

    run(&mut st, O::OpenGettingStarted);
    run(&mut st, O::ShowMe(Card::Worktrees));
    assert_eq!(st.palette.as_ref().map(|p| p.query.as_str()), Some("worktree"), "the palette lists the feature's commands");
    st.palette = None;

    st.session.selection = Selection::Wip;
    assert_eq!(Card::Rebase.show_me(&st).err(), Some(st.settings.lang().tr("getting_started.needs_commit")));
    let head = st.snap.as_ref().unwrap().head_oid.unwrap();
    st.session.selection = Selection::Commit(head);
    run(&mut st, O::OpenGettingStarted);
    run(&mut st, O::ShowMe(Card::Rebase));
    assert_eq!(st.palette.as_ref().map(|p| p.query.as_str()), Some("rebase"));
    st.palette = None;

    run(&mut st, O::OpenGettingStarted);
    run(&mut st, O::ShowMe(Card::Journal));
    assert!(st.session.journal.view.is_some() && st.onboarding.getting_started.is_none());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn stash_needs_something_to_stash_or_apply() {
    let (mut st, dir) = opened("gs-stash-clean", |d| git(d, &["checkout", "-q", "--", "a.txt"]));
    assert!(!st.snap.as_ref().unwrap().is_dirty());
    let t = st.settings.lang();
    assert_eq!(Card::Stash.show_me(&st).err(), Some(t.tr("getting_started.needs_changes")), "the palette would list no stash command");
    run(&mut st, O::OpenGettingStarted);
    run(&mut st, O::ShowMe(Card::Stash));
    assert!(st.onboarding.getting_started.is_some() && st.palette.is_none(), "the page stays");
    let _ = std::fs::remove_dir_all(dir);

    let (st, dir) = opened("gs-stash-dirty", |_| {});
    assert!(matches!(Card::Stash.show_me(&st), Ok(ShowMe::Search("stash"))), "changes to stash");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn blame_and_worktrees_need_a_first_commit() {
    let (mut st, dir) = opened("gs-unborn", |d| git(d, &["checkout", "-q", "--orphan", "fresh"]));
    assert!(st.snap.as_ref().unwrap().head_oid.is_none(), "an unborn branch");
    let first = Some(st.settings.lang().tr("getting_started.needs_first_commit"));
    assert_eq!(Card::Blame.show_me(&st).err(), first);
    assert_eq!(Card::Worktrees.show_me(&st).err(), first);
    run(&mut st, O::OpenGettingStarted);
    run(&mut st, O::ShowMe(Card::Blame));
    assert!(st.onboarding.getting_started.is_some() && st.palette.is_none(), "the page stays");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn blame_opens_the_file_pick_without_counting_as_palette_use() {
    let (mut st, dir) = opened("gs-blame", |_| {});
    run(&mut st, O::OpenGettingStarted);
    let fx = run(&mut st, O::ShowMe(Card::Blame));
    assert!(matches!(st.palette.as_ref().map(|p| &p.mode), Some(Mode::Pick(_))), "the file pick opens");
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadTrackedPaths { .. })));
    assert!(st.palette_recent.is_empty(), "not recorded among the palette's recent commands");
    let _ = std::fs::remove_dir_all(dir);
}
