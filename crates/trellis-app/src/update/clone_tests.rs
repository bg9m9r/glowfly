//! Headless tests for cloning: the Clone dialog's form (its rules and the request it builds),
//! the job a clone request starts, and a shallow clone end to end.

use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::CloneMsg;
use crate::msg::{FolderPurpose, Msg};
use crate::state::clone::{CloneForm, CloneRequest, LimitKind};
use crate::state::{AppState, Modal, RunningJob};
use chrono::NaiveDate;
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::clone_ops::{CloneOptions, HistoryLimit, Shallow};
use trellis_core::gitcli::{args, run};
use trellis_core::settings::Settings;

const URL: &str = "https://example.com/team/app.git";

fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 2, 1).unwrap()
}

fn request(options: CloneOptions) -> CloneRequest {
    CloneRequest { url: URL.into(), parent: PathBuf::from("C:/work"), name: "app".into(), options }
}

fn shallow(limit: HistoryLimit, branch: Option<&str>) -> CloneOptions {
    CloneOptions { shallow: Some(Shallow { limit, branch: branch.map(str::to_owned) }) }
}

/// The single RunGit job in `fx`: (folder, commands, prompts).
fn job(fx: &[Effect]) -> (PathBuf, Vec<Vec<String>>, bool) {
    match fx {
        [Effect::RunGit { cwd, cmds, prompts, .. }] => (cwd.clone(), cmds.clone(), *prompts),
        other => panic!("unexpected effects {other:?}"),
    }
}

fn form(st: &mut AppState) -> &mut CloneForm {
    match st.modal.as_mut() {
        Some(Modal::Clone(f)) => f,
        _ => panic!("no clone dialog"),
    }
}

/// A filled-in form whose folder check passes.
fn filled() -> CloneForm {
    CloneForm { url: URL.into(), ..CloneForm::new("C:/work".into()) }
}

#[test]
fn clone_dialog_opens_with_shallow_off() {
    let mut st = AppState::new(Settings::default(), vec![PathBuf::from("C:/work/app")], Instant::now());
    update(&mut st, Msg::Clone(CloneMsg::Dialog), Instant::now());
    let f = form(&mut st);
    assert!(!f.shallow);
    assert_eq!(f.limit, LimitKind::Depth);
    assert_eq!(f.depth, "1");
    assert!(f.url.is_empty() && f.since.is_empty() && f.branch.is_empty());
    assert_eq!(Path::new(&f.parent), Path::new("C:/work"));
}

#[test]
fn full_clone_runs_one_job_in_the_parent() {
    let mut st = state();
    let req = request(CloneOptions::default());
    let (cwd, cmds, prompts) = job(&update(&mut st, Msg::Clone(CloneMsg::Start(req.clone())), Instant::now()));
    assert_eq!(cwd, req.parent);
    assert_eq!(cmds, [args(&["clone", "--", URL, "app"])]);
    assert!(prompts, "a clone can ask for credentials");
    let running = st.job.as_ref().unwrap();
    assert_eq!(running.label, "Clone");
    assert_eq!(running.then_open, Some(req.parent.join("app")));
}

#[test]
fn shallow_clone_commands() {
    let mut st = state();
    let req = request(shallow(HistoryLimit::Depth(3), Some("dev")));
    let (_, cmds, _) = job(&update(&mut st, Msg::Clone(CloneMsg::Start(req)), Instant::now()));
    assert_eq!(cmds, [args(&["clone", "--depth=3", "--single-branch", "--branch=dev", "--", URL, "app"])]);

    let mut st = state();
    let req = request(shallow(HistoryLimit::Since(NaiveDate::from_ymd_opt(2026, 1, 5).unwrap()), None));
    let (_, cmds, _) = job(&update(&mut st, Msg::Clone(CloneMsg::Start(req)), Instant::now()));
    assert_eq!(cmds, [args(&["clone", "--shallow-since=2026-01-05T00:00:00", "--single-branch", "--", URL, "app"])]);
}

#[test]
fn a_second_clone_waits_for_the_running_job() {
    let mut st = state();
    st.job = Some(RunningJob {
        label: "Fetch".into(),
        workdir: None,
        quiet: false,
        clear_commit: false,
        then_push: false,
        then_open: None,
        keeps_undo: true,
    });
    assert!(update(&mut st, Msg::Clone(CloneMsg::Start(request(CloneOptions::default()))), Instant::now()).is_empty());
    assert_eq!(st.log.last().unwrap().text, "Another operation is still running.");
}

#[test]
fn a_bad_request_is_refused() {
    let mut st = state();
    let req = CloneRequest { url: "-oops".into(), ..request(CloneOptions::default()) };
    assert!(update(&mut st, Msg::Clone(CloneMsg::Start(req)), Instant::now()).is_empty());
    assert!(st.job.is_none());
    assert!(st.log.last().unwrap().text.starts_with("Could not clone"));
}

#[test]
fn picked_folder_fills_the_form_and_keeps_its_options() {
    let mut st = state();
    st.modal = Some(Modal::Clone(CloneForm { shallow: true, depth: "7".into(), branch: "dev".into(), ..filled() }));
    update(&mut st, Msg::FolderPicked { purpose: FolderPurpose::CloneParent, path: Some(PathBuf::from("D:/src")) }, Instant::now());
    let f = form(&mut st);
    assert_eq!(Path::new(&f.parent), Path::new("D:/src"));
    assert!(f.shallow);
    assert_eq!((f.depth.as_str(), f.branch.as_str(), f.url.as_str()), ("7", "dev", URL));
}

#[test]
fn form_rules() {
    let t = today();
    // Empty fields block Clone without saying anything.
    let p = CloneForm::new(String::new()).problems(t, false);
    assert!(p.blocked);
    assert_eq!((p.url, p.folder, p.depth, p.since, p.branch), (None, None, None, None, None));

    assert!(!filled().problems(t, true).blocked);
    let p = CloneForm { url: "-oops".into(), ..filled() }.problems(t, true);
    assert_eq!(p.url.as_deref(), Some("URLs can't start with a dash."));
    assert!(p.blocked);
    let p = CloneForm { url: "https://example.com/.git".into(), ..filled() }.problems(t, true);
    assert_eq!(p.url.as_deref(), Some("Can't tell the new folder's name from this URL."));
    let p = filled().problems(t, false);
    assert_eq!(p.folder.as_deref(), Some("That folder doesn't exist."));
    assert!(p.blocked);

    let p = CloneForm { shallow: true, depth: "0".into(), ..filled() }.problems(t, true);
    assert_eq!(p.depth.as_deref(), Some("Download at least one commit."));
    assert!(p.blocked);
    let p = CloneForm { shallow: true, limit: LimitKind::Since, since: "2026-03-01".into(), ..filled() }.problems(t, true);
    assert_eq!(p.since.as_deref(), Some("Choose today or an earlier date."));
    assert!(p.blocked);

    // Only the selected mode's field is checked.
    let p =
        CloneForm { shallow: true, limit: LimitKind::Since, since: "2026-01-05".into(), depth: "x".into(), ..filled() }.problems(t, true);
    assert_eq!(p, Default::default());
    let p = CloneForm { shallow: true, depth: "5".into(), since: "nope".into(), ..filled() }.problems(t, true);
    assert_eq!(p, Default::default());

    let p = CloneForm { shallow: true, branch: "a b".into(), ..filled() }.problems(t, true);
    assert_eq!(p.branch.as_deref(), Some("Not a valid branch or tag name."));
    assert!(p.blocked);

    // With shallow off the shallow fields don't matter.
    let off = CloneForm { depth: "0".into(), since: "x".into(), branch: "a b".into(), limit: LimitKind::Since, ..filled() };
    assert_eq!(off.problems(t, true), Default::default());
}

#[test]
fn form_builds_the_request() {
    let t = today();
    let f = CloneForm { url: format!("  {URL} "), ..filled() };
    let req = f.request(t).unwrap();
    assert_eq!(req.url, URL);
    assert_eq!(req.name, "app");
    assert_eq!(req.parent, PathBuf::from("C:/work"));
    assert_eq!(req.options, CloneOptions::default());

    let req = CloneForm { shallow: true, ..filled() }.request(t).unwrap();
    assert_eq!(req.options, shallow(HistoryLimit::Depth(1), None));
    let req = CloneForm { shallow: true, branch: " release/2.0 ".into(), ..filled() }.request(t).unwrap();
    assert_eq!(req.options, shallow(HistoryLimit::Depth(1), Some("release/2.0")));
    let since = CloneForm { shallow: true, limit: LimitKind::Since, since: "2026-01-05".into(), ..filled() };
    assert_eq!(since.request(t).unwrap().options, shallow(HistoryLimit::Since(NaiveDate::from_ymd_opt(2026, 1, 5).unwrap()), None));
    assert_eq!(CloneForm { shallow: true, depth: "0".into(), ..filled() }.request(t), None);
}

#[test]
fn shallow_clone_end_to_end_opens_the_new_repository() {
    let src = scratch_repo("clone-src");
    for text in ["two", "three"] {
        std::fs::write(src.join("a.txt"), text).unwrap();
        git(&src, &["commit", "-q", "-am", text]);
    }
    let parent = src.with_file_name(format!("{}-into", src.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&parent);
    std::fs::create_dir_all(&parent).unwrap();
    let mut st = state();
    let req = CloneRequest {
        url: src.display().to_string(),
        parent: parent.clone(),
        name: "copy".into(),
        options: shallow(HistoryLimit::Depth(1), None),
    };
    drive(&mut st, Msg::Clone(CloneMsg::Start(req)));
    let dest = parent.join("copy");
    assert!(dest.is_dir(), "log: {:?}", st.log.iter().map(|l| &l.text).collect::<Vec<_>>());
    let (ok, out) = run(&dest, &args(&["rev-parse", "--is-shallow-repository"]));
    assert!(ok && out.trim() == "true", "{out}");
    let same = |p: &Path| p.canonicalize().ok() == dest.canonicalize().ok();
    assert!(st.tabs.iter().any(|t| t.path.as_deref().is_some_and(same)), "a tab for the clone");
    let snap = st.snap.as_ref().expect("the clone is loaded");
    assert!(same(&snap.workdir));
    assert_eq!(snap.commits.len(), 1, "only the newest commit was downloaded");
    let _ = std::fs::remove_dir_all(&parent);
    let _ = std::fs::remove_dir_all(&src);
}

#[test]
fn the_parent_folder_is_checked_by_an_effect_not_the_view() {
    let dir = scratch_repo("clone-parent");
    let now = Instant::now();
    let mut st = AppState::new(Settings::default(), vec![dir.join("app")], now);
    // Opening the dialog checks its prefilled folder at once.
    let fx = update(&mut st, Msg::Clone(CloneMsg::Dialog), now);
    assert!(matches!(&fx[..], [Effect::CheckCloneParent { delay, .. }] if delay.is_zero()), "{fx:?}");
    form(&mut st).url = URL.into();
    form(&mut st).parent = format!(" {} ", dir.display());
    let p = form(&mut st).problems_now(today());
    assert!(p.blocked && p.folder.is_none(), "unchecked text blocks Clone, without a message");

    // An edit asks again after a pause.
    let fx = update(&mut st, Msg::Clone(CloneMsg::ParentEdited), now);
    let [Effect::CheckCloneParent { parent, delay }] = &fx[..] else { panic!("expected a folder check, got {fx:?}") };
    assert_eq!((parent.clone(), *delay), (dir.display().to_string(), super::clone::PARENT_CHECK_DELAY));
    let answer = crate::runner::execute(Effect::CheckCloneParent { parent: parent.clone(), delay: Default::default() }).unwrap();
    // An answer for text that has changed since is dropped.
    update(&mut st, Msg::Clone(CloneMsg::ParentChecked { parent: "elsewhere".into(), is_dir: true }), now);
    assert_eq!(form(&mut st).parent_is_dir(), None);
    update(&mut st, answer, now);
    assert_eq!(form(&mut st).parent_is_dir(), Some(true));
    assert!(!form(&mut st).problems_now(today()).blocked);

    // A folder that isn't there gets its red line once checked.
    let gone = dir.join("missing");
    form(&mut st).parent = gone.display().to_string();
    let fx = update(&mut st, Msg::Clone(CloneMsg::ParentEdited), now);
    let [Effect::CheckCloneParent { parent, .. }] = &fx[..] else { panic!("expected a folder check, got {fx:?}") };
    let answer = crate::runner::execute(Effect::CheckCloneParent { parent: parent.clone(), delay: Default::default() }).unwrap();
    update(&mut st, answer, now);
    assert_eq!(form(&mut st).problems_now(today()).folder.as_deref(), Some("That folder doesn't exist."));

    // A picked folder is checked at once.
    let fx = update(&mut st, Msg::FolderPicked { purpose: FolderPurpose::CloneParent, path: Some(dir.clone()) }, now);
    assert!(matches!(&fx[..], [Effect::CheckCloneParent { delay, .. }] if delay.is_zero()), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
