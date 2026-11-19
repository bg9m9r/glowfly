//! Headless tests of the tab a toast names (`Toast::tab`): a toast raised for a repository whose
//! tab is open but not active says that tab first, by the name the tab strip shows, fixed when
//! it is raised; the activity log never has the name, and a clone or an init names no tab.

use super::super::auto_stash::told;
use super::super::jobs::finish_made;
use super::super::update;
use crate::effect::Effect;
use crate::logs::{EarlierLog, LogsEffect, LogsMsg};
use crate::msg::{JobMsg, Msg, TabsMsg};
use crate::state::menu_bar::tab_name;
use crate::state::trust::Created;
use crate::state::{AppState, RunningJob, Tab, Toast};
use glowfly_core::stash_ops::PutBack;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// A repository path no tab needs to exist at.
fn repo(name: &str) -> PathBuf {
    glowfly_core::paths::temp_dir().join(format!("glowfly-toast-tab-{name}"))
}

/// A state with two tabs: `a` (active) and `b`.
fn two_tabs() -> (AppState, PathBuf, PathBuf) {
    let mut st = super::super::tests::state();
    let (a, b) = (repo("a"), repo("b"));
    st.tabs = vec![Tab::unloaded(a.clone()), Tab::unloaded(b.clone())];
    st.active_tab = 0;
    st.snap = None;
    (st, a, b)
}

fn job(label: &str, workdir: &Path) -> RunningJob {
    RunningJob {
        label: label.into(),
        workdir: Some(workdir.to_path_buf()),
        quiet: false,
        clear_commit: false,
        then_open: None,
        then_push: false,
        then_continue: false,
        then_pr: None,
        then_link: false,
        keeps_undo: true,
        fetches: false,
    }
}

fn done(st: &mut AppState, ok: bool, output: &str) -> Vec<Effect> {
    update(st, Msg::Job(JobMsg::JobDone { ok, output: output.into(), undo: None }), Instant::now())
}

fn scoped(st: &mut AppState, workdir: &Path, msg: Msg) -> Vec<Effect> {
    update(st, Msg::Scoped { workdir: workdir.to_path_buf(), msg: Box::new(msg) }, Instant::now())
}

fn failures(st: &AppState) -> Vec<&Toast> {
    st.toasts.iter().filter(|t| !t.ok).collect()
}

/// What toast 0 names and says.
fn first(st: &AppState) -> (Option<String>, String) {
    (st.toasts[0].tab.clone(), st.toasts[0].said(st.settings.lang()).into_owned())
}

#[test]
fn a_failed_job_in_another_tab_names_it() {
    let (mut st, _, b) = two_tabs();
    st.job = Some(job("Push", &b));
    done(&mut st, false, "remote rejected");
    let name = tab_name(&st, 1).to_owned();
    let toasts = failures(&st);
    assert_eq!(toasts.len(), 1);
    let t = toasts[0];
    assert_eq!(t.tab.as_deref(), Some(name.as_str()));
    assert!(t.text.starts_with("Push failed"), "{}", t.text);
    assert!(!t.text.contains(&name));
    assert!(t.said(st.settings.lang()).starts_with(&format!("In {name}: Push failed")));
    let details = t.action.as_ref().or(t.details.as_ref()).expect("Details");
    assert!(matches!(&*details.msg, Msg::Logs(LogsMsg::Details { tab, .. }) if *tab == st.tabs[1].id));
}

#[test]
fn the_same_failure_in_the_active_tab_has_no_name() {
    let (mut st, a, _) = two_tabs();
    st.job = Some(job("Push", &a));
    done(&mut st, false, "remote rejected");
    let toasts = failures(&st);
    assert_eq!(toasts.len(), 1);
    assert_eq!(toasts[0].tab, None);
    assert_eq!(toasts[0].said(st.settings.lang()), toasts[0].text);
}

#[test]
fn a_success_from_another_tab_is_named() {
    let (mut st, _, b) = two_tabs();
    st.job = Some(job("Fetch", &b));
    done(&mut st, true, "");
    let name = tab_name(&st, 1).to_owned();
    let t = st.toasts.last().expect("a success toast");
    assert!(t.ok);
    assert_eq!(t.tab.as_deref(), Some(name.as_str()));

    let (mut st, _, b) = two_tabs();
    st.job = Some(RunningJob { quiet: true, ..job("Fetch", &b) });
    done(&mut st, true, "");
    assert!(st.toasts.is_empty(), "a quiet job raises no toast");
}

#[test]
fn an_announced_failure_for_another_tab_is_named() {
    let (mut st, _, b) = two_tabs();
    scoped(&mut st, &b, Msg::Logged { ok: false, text: "boom".into() });
    let t = failures(&st)[0];
    assert_eq!(t.text, "boom");
    assert_eq!(t.tab.as_deref(), Some(tab_name(&st, 1)));

    let (mut st, _, _) = two_tabs();
    update(&mut st, Msg::Logged { ok: false, text: "boom".into() }, Instant::now());
    assert_eq!(failures(&st)[0].tab, None);
}

#[test]
fn the_name_is_fixed_when_raised() {
    let (mut st, _, b) = two_tabs();
    st.job = Some(job("Push", &b));
    done(&mut st, false, "remote rejected");
    let before = first(&st);
    assert!(before.0.is_some());
    update(&mut st, Msg::Tabs(TabsMsg::SelectTab(1)), Instant::now());
    assert_eq!(first(&st), before);
    st.tabs[1].label = Some("Renamed".into());
    assert_eq!(first(&st), before);

    let (mut st, a, _) = two_tabs();
    st.job = Some(job("Fetch", &a));
    done(&mut st, true, "");
    update(&mut st, Msg::Tabs(TabsMsg::SelectTab(1)), Instant::now());
    assert_eq!(st.toasts[0].tab, None);
}

#[test]
fn tabs_with_the_same_folder_name_are_told_apart_as_the_strip_does() {
    let mut st = super::super::tests::state();
    let (x, y) = (repo("x").join("app"), repo("y").join("app"));
    st.tabs =
        vec![Tab::unloaded(repo("other")), Tab { label: Some("Frontend".into()), ..Tab::unloaded(x.clone()) }, Tab::unloaded(y.clone())];
    st.active_tab = 0;
    st.snap = None;
    scoped(&mut st, &x, Msg::Logged { ok: false, text: "boom".into() });
    scoped(&mut st, &y, Msg::Logged { ok: false, text: "boom".into() });
    let names: Vec<Option<&str>> = failures(&st).iter().map(|t| t.tab.as_deref()).collect();
    assert_eq!(names, [Some("Frontend"), Some("app")]);
    assert_eq!(names, [Some(tab_name(&st, 1)), Some(tab_name(&st, 2))]);

    // Without the label, equal strip names are equal toast names, and one toast restarts.
    st.toasts.clear();
    st.tabs[1].label = None;
    scoped(&mut st, &x, Msg::Logged { ok: false, text: "boom".into() });
    scoped(&mut st, &y, Msg::Logged { ok: false, text: "boom".into() });
    let names: Vec<Option<&str>> = failures(&st).iter().map(|t| t.tab.as_deref()).collect();
    assert_eq!(names, [Some("app")]);
}

#[test]
fn the_activity_line_is_unchanged() {
    let (mut st, _, b) = two_tabs();
    let loaded = EarlierLog { dir: glowfly_core::paths::temp_dir().join("glowfly-toast-tab-logs"), entries: vec![] };
    update(&mut st, Msg::Logs(LogsMsg::ActivityLoaded(Ok(loaded))), Instant::now());
    let before = st.log.len();
    st.job = Some(job("Push", &b));
    let fx = done(&mut st, false, "remote rejected");
    let name = tab_name(&st, 1).to_owned();
    let said = st.toasts[0].said(st.settings.lang()).into_owned();
    assert!(st.tabs[1].log.last().is_some_and(|e| e.text.starts_with("Push FAILED")));
    assert_eq!(st.log.len(), before, "the active tab's log gained no line");
    let lines: Vec<_> = fx
        .iter()
        .filter_map(|e| match e {
            Effect::Logs(LogsEffect::Append(lines)) => Some(lines),
            _ => None,
        })
        .flatten()
        .collect();
    assert!(!lines.is_empty());
    for l in &lines {
        assert_eq!(l.repo.as_deref(), Some(b.as_path()));
        assert!(l.text.starts_with("Push FAILED"));
        assert!(!l.text.contains(&said) && !l.text.contains(&format!("In {name}:")));
    }
    for e in st.log.iter().chain(&st.tabs[1].log) {
        assert!(!e.text.contains(&format!("In {name}:")));
    }
}

#[test]
fn a_clone_names_no_tab() {
    let (mut st, _, b) = two_tabs();
    st.job = Some(RunningJob { then_open: Some((repo("made"), Created::Clone)), ..job("Clone", &b) });
    done(&mut st, false, "fatal: repository not found");
    let toasts = failures(&st);
    assert_eq!(toasts.len(), 1);
    assert_eq!(toasts[0].tab, None);
}

#[test]
fn a_repository_made_by_a_failed_clone_names_no_tab() {
    let (mut st, _, b) = two_tabs();
    st.job = Some(RunningJob { then_open: Some((repo("made"), Created::Clone)), ..job("Clone", &b) });
    let mut fx = Vec::new();
    finish_made(&mut st, true, false, "boom".into(), Some("headline".into()), Instant::now(), &mut fx);
    assert!(!failures(&st).is_empty());
    assert!(st.toasts.iter().all(|t| t.tab.is_none()));
}

#[test]
fn an_auto_stash_put_back_for_another_tab_is_named() {
    let (mut st, a, b) = two_tabs();
    let now = Instant::now();
    told(&mut st, "stash name", PutBack::Restored, "", git2::Oid::zero(), b.clone(), now);
    assert_eq!(st.toasts[0].tab.as_deref(), Some(tab_name(&st, 1)));
    st.toasts.clear();
    told(&mut st, "stash name", PutBack::Failed, "", git2::Oid::zero(), b.clone(), now);
    let t = &st.toasts[0];
    assert!(!t.ok);
    assert_eq!(t.tab.as_deref(), Some(tab_name(&st, 1)));
    assert_eq!(t.action.as_ref().and_then(|a| a.workdir.as_deref()), Some(b.as_path()));
    st.toasts.clear();
    told(&mut st, "stash name", PutBack::Restored, "", git2::Oid::zero(), a.clone(), now);
    told(&mut st, "stash name", PutBack::Failed, "", git2::Oid::zero(), a, now);
    assert!(st.toasts.iter().all(|t| t.tab.is_none()));
}
