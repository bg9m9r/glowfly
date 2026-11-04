//! Headless tests for the tips' rules (state/hints.rs, update/hints.rs): no tip in the welcome's
//! session, one per session, none while something blocks; an ignored tip comes back once; acting,
//! dismissing or using the feature retires it; tips off, Show tips again, and the records kept
//! through a settings read.

use super::hints_tests::{job_done, open_repo, shown};
use super::tests::state;
use super::update;
use crate::effect::Effect;
use crate::effect::SaveEffect;
use crate::msg::{AppMsg, JobMsg, Msg};
use crate::onboarding::OnboardingMsg as O;
use crate::state::hints::{self, HintId};
use crate::state::settings_file::{carry_records, user_view};
use crate::state::{AppState, Modal, PrefPage, PrefsState, RunningJob};
use git2::{Oid, RepositoryState};
use glowfly_core::settings::Settings;
use glowfly_core::undo::{Action, ResetMode};
use std::time::Instant;

fn run(st: &mut AppState, m: O) -> Vec<Effect> {
    update(st, Msg::Onboarding(m), Instant::now())
}

fn saves(fx: &[Effect]) -> bool {
    fx.iter().any(|e| matches!(e, Effect::Save(SaveEffect::Settings(_))))
}

fn reset() -> Action {
    let oid = |n: u8| Oid::from_bytes(&[n; 20]).unwrap();
    Action::Reset { mode: ResetMode::Mixed, branch: Some("main".into()), from: oid(1), to: oid(2), stash: None }
}

fn seen(st: &AppState, id: HintId) -> bool {
    st.settings.hints_seen.contains(id.id())
}

fn ignored(st: &AppState, id: HintId) -> bool {
    st.settings.hints_ignored.contains(id.id())
}

#[test]
fn ids_are_stable() {
    assert_eq!(HintId::ALL.map(HintId::id), ["undo", "tidy-commits", "connect"]);
}

#[test]
fn nothing_shows_in_the_welcomes_session() {
    let (mut st, _) = open_repo("hint-quiet");
    st.settings.welcome_done = false;
    run(&mut st, O::Startup);
    assert!(st.onboarding.quiet_session);
    assert!(!hints::offer(&mut st, HintId::Undo));
    run(&mut st, O::CloseWelcome);
    assert!(!hints::offer(&mut st, HintId::Undo), "the whole launch stays quiet");
}

#[test]
fn the_second_offer_in_a_session_is_dropped() {
    let (mut st, _) = open_repo("hint-second");
    assert!(hints::offer(&mut st, HintId::Undo));
    assert!(!hints::offer(&mut st, HintId::TidyCommits), "one shows already");
    run(&mut st, O::DismissHint(HintId::Undo));
    assert!(!hints::offer(&mut st, HintId::TidyCommits), "one per session");
}

#[test]
fn each_block_drops_the_offer() {
    let (mut st, dir) = open_repo("hint-blocks");
    st.modal = Some(Modal::About);
    assert!(!hints::offer(&mut st, HintId::Undo), "a dialog");
    st.modal = None;
    st.prefs = Some(PrefsState::new(PrefPage::General));
    assert!(!hints::offer(&mut st, HintId::Undo), "Preferences");
    st.prefs = None;
    st.job = Some(RunningJob {
        label: "Fetch".into(),
        workdir: Some(dir.clone()),
        quiet: false,
        clear_commit: false,
        then_open: None,
        then_push: false,
        then_continue: false,
        then_pr: None,
        then_link: false,
        keeps_undo: true,
        fetches: true,
    });
    assert!(!hints::offer(&mut st, HintId::Undo), "a job");
    st.job = None;
    st.snap.as_mut().unwrap().state = RepositoryState::Merge;
    assert!(!hints::offer(&mut st, HintId::Undo), "a merge in progress");
    st.snap.as_mut().unwrap().state = RepositoryState::Clean;
    st.snap.as_mut().unwrap().unstaged[0].status = '!';
    assert!(!hints::offer(&mut st, HintId::Undo), "a conflicted file");
    st.snap.as_mut().unwrap().unstaged[0].status = 'M';
    assert_eq!(st.onboarding.hints_shown_this_session, 0, "dropped, not counted");
    assert!(hints::offer(&mut st, HintId::Undo));
}

#[test]
fn an_ignored_tip_comes_back_once() {
    let (mut st, _) = open_repo("hint-ignore");
    assert!(hints::offer(&mut st, HintId::Undo));
    let fx = update(&mut st, Msg::App(AppMsg::QuitConfirmed), Instant::now());
    assert_eq!(shown(&st), None);
    assert!(ignored(&st, HintId::Undo) && !seen(&st, HintId::Undo) && saves(&fx), "the first ignore is recorded");
    // A later session: it shows again, and leaving it again retires it.
    let (mut later, _) = open_repo("hint-ignore-later");
    later.settings.hints_ignored = st.settings.hints_ignored.clone();
    assert!(hints::offer(&mut later, HintId::Undo));
    later.snap = None;
    run(&mut later, O::TourBack);
    assert_eq!(shown(&later), None);
    assert!(seen(&later, HintId::Undo), "the second ignore retires it");
    assert!(!hints::offer(&mut later, HintId::Undo));
}

#[test]
fn hiding_the_left_panel_ignores_connect() {
    let (mut st, _) = open_repo("hint-ignore-panel");
    assert!(hints::offer(&mut st, HintId::Connect));
    st.settings.show_left_panel = false;
    run(&mut st, O::TourBack);
    assert_eq!(shown(&st), None);
    assert!(ignored(&st, HintId::Connect));
}

#[test]
fn acting_on_a_tip_retires_it_and_does_its_action() {
    let (mut st, _) = open_repo("hint-act");
    assert!(hints::offer(&mut st, HintId::Undo));
    let fx = run(&mut st, O::HintAct(HintId::Undo));
    assert!(seen(&st, HintId::Undo) && saves(&fx) && shown(&st).is_none());
    assert!(st.session.journal.view.is_some(), "Open journal opened it");
}

#[test]
fn dismissing_a_tip_retires_it() {
    let (mut st, _) = open_repo("hint-dismiss");
    assert!(hints::offer(&mut st, HintId::Connect));
    let fx = run(&mut st, O::DismissHint(HintId::Connect));
    assert!(seen(&st, HintId::Connect) && saves(&fx) && shown(&st).is_none());
}

#[test]
fn running_undo_retires_the_undo_tip_while_hidden() {
    let (mut st, dir) = open_repo("hint-used");
    st.onboarding.quiet_session = true;
    job_done(&mut st, &dir, "Reset", Some(reset()));
    assert_eq!(shown(&st), None, "quiet");
    let fx = update(&mut st, Msg::Job(JobMsg::Undo), Instant::now());
    assert!(seen(&st, HintId::Undo) && saves(&fx));
}

#[test]
fn tips_off_stops_every_offer() {
    let (mut st, _) = open_repo("hint-off");
    assert!(hints::offer(&mut st, HintId::Undo));
    let fx = run(&mut st, O::HintsOff);
    assert!(!st.settings.hints && saves(&fx) && shown(&st).is_none());
    assert!(st.settings.hints_seen.is_empty(), "nothing retired");
    st.onboarding.hints_shown_this_session = 0;
    assert!(!hints::offer(&mut st, HintId::TidyCommits));
    run(&mut st, O::SetHints(true));
    assert!(st.settings.hints && hints::offer(&mut st, HintId::TidyCommits));
}

#[test]
fn show_tips_again_clears_the_records_and_the_count() {
    let (mut st, _) = open_repo("hint-reset");
    assert!(hints::offer(&mut st, HintId::Undo));
    run(&mut st, O::DismissHint(HintId::Undo));
    st.settings.hints_ignored.insert("connect".into());
    st.settings.status_tip_shown.insert("C:/x/".into(), true);
    st.settings.detached_head_warned = true;
    let fx = run(&mut st, O::ResetHints);
    let s = &st.settings;
    assert!(s.hints_seen.is_empty() && s.hints_ignored.is_empty() && s.status_tip_shown.is_empty() && !s.detached_head_warned);
    assert!(saves(&fx) && st.onboarding.hints_shown_this_session == 0);
    assert!(st.toasts.iter().any(|t| t.text == "Tips will show again as you work."));
    assert!(hints::offer(&mut st, HintId::Undo));
}

#[test]
fn the_records_are_merged_and_not_user_settings() {
    let mut ours = Settings::default();
    ours.hints_seen.insert("undo".into());
    ours.hints_ignored.insert("connect".into());
    let mut file = Settings::default();
    file.hints_seen.insert("tidy-commits".into());
    assert!(carry_records(&ours, &mut file));
    assert_eq!(file.hints_seen.iter().map(String::as_str).collect::<Vec<_>>(), ["tidy-commits", "undo"]);
    assert!(file.hints_ignored.contains("connect"));
    assert!(!carry_records(&ours, &mut file), "nothing new the second time");
    assert_eq!(user_view(&ours), user_view(&Settings::default()), "records aren't edits");
    let off = Settings { hints: false, ..Settings::default() };
    assert_ne!(user_view(&off), user_view(&Settings::default()), "the switch is the user's");
}

#[test]
fn the_palette_shows_the_toggle_that_applies() {
    let mut st = state();
    let labels = |st: &AppState| crate::state::palette::entries(st).into_iter().map(|e| e.label).collect::<Vec<_>>();
    assert!(labels(&st).iter().any(|l| l == "Turn off tips") && !labels(&st).iter().any(|l| l == "Turn on tips"));
    assert!(labels(&st).iter().any(|l| l == "Show tips again"));
    st.settings.hints = false;
    assert!(labels(&st).iter().any(|l| l == "Turn on tips") && !labels(&st).iter().any(|l| l == "Turn off tips"));
}
