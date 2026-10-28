//! Repository trust: a trusted repository that gains settings or hooks since it was trusted is
//! asked about again (only what is new), and held like an undecided one until then; decisions
//! saved before prints were kept, and what Glowfly's own `git lfs install` adds, don't ask.

use super::jobs::{JobSpec, start_job};
use super::tests::{finish, finish_keeping, git, state};
use super::trust_tests::{asking, done, drive, open, real, run, untrusted_repo};
use crate::effect::Effect;
use crate::msg::{Msg, PaletteMsg, RepoMsg, StageMsg, TrustMsg};
use crate::state::palette::{Palette, PaletteCommand, entries};
use crate::state::trust::{Standing, TrustPrompt, record};
use crate::state::{AppState, Modal};
use glowfly_core::settings::RepoTrust;
use std::path::Path;
use std::time::Duration;

/// Writes a hook script `name` that does nothing into the repository at `dir`.
pub(super) fn add_hook(dir: &Path, name: &str) {
    let hook = dir.join(".git").join("hooks").join(name);
    std::fs::write(&hook, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
}

/// `untrusted_repo` opened and trusted from its first prompt.
pub(super) fn trusted_repo(name: &str) -> (AppState, std::path::PathBuf) {
    let dir = untrusted_repo(name);
    let mut st = state();
    open(&mut st, &dir);
    assert!(asking(&st));
    let real = real(&st);
    drive(&mut st, Msg::Trust(TrustMsg::Decide { real, trusted: true }));
    assert_eq!(st.trust_standing(), Some(Standing::Trusted));
    (st, dir)
}

fn prompt(st: &AppState) -> &TrustPrompt {
    let Some(Modal::Trust(p)) = &st.modal else { panic!("expected the trust prompt") };
    p
}

fn reload(st: &mut AppState) {
    drive(st, Msg::Repo(RepoMsg::Refresh));
}

fn saved_print(st: &AppState) -> Option<glowfly_core::trust::TrustPrint> {
    st.settings.saved_trust(&real(st)).and_then(|t| t.print)
}

#[test]
fn a_new_hook_asks_again_about_it_alone() {
    let (mut st, dir) = trusted_repo("print-new-hook");
    assert_eq!(saved_print(&st), Some(st.snap.as_ref().unwrap().trust_print.clone()), "the print of what was shown");
    add_hook(&dir, "post-merge");
    reload(&mut st);
    assert!(asking(&st), "it asks again");
    assert_eq!(st.trust_standing(), Some(Standing::Changed));
    let gained = prompt(&st).gained.clone().expect("what is new");
    assert_eq!(gained.hooks, ["post-merge"]);
    assert!(gained.config.is_empty() && gained.submodules.is_empty(), "{gained:?}");
    assert!(st.held());

    // Held like an undecided repository.
    drive(&mut st, Msg::CloseModal);
    st.settings.auto_fetch_minutes = 1;
    let at = st.last_fetch + Duration::from_secs(61);
    assert!(!run(&mut st, Msg::Tick(at)).iter().any(|e| matches!(e, Effect::RunGit { .. })), "no auto-fetch");
    let fx = run(&mut st, Msg::Stage(StageMsg::LocalOpDone(Ok(None))));
    let git_status = fx.iter().find_map(|e| if let Effect::LoadStatus { git_status, .. } = e { Some(*git_status) } else { None });
    assert_eq!(git_status, Some(false), "git doesn't read its status");
    finish(&mut st, fx);
    let fx = run(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadRepo { opts, .. } if !opts.trusted)), "{fx:?}");
    finish(&mut st, fx);
    assert!(!asking(&st), "asked once per session");

    // Trusting it again takes in the new hook.
    let real = real(&st);
    drive(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::ReviewTrust)));
    assert!(asking(&st));
    drive(&mut st, Msg::Trust(TrustMsg::Decide { real, trusted: true }));
    assert_eq!(saved_print(&st), Some(st.snap.as_ref().unwrap().trust_print.clone()));
    reload(&mut st);
    assert!(!asking(&st) && !st.held());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn removing_a_hook_doesn_t_ask() {
    let (mut st, dir) = trusted_repo("print-removed-hook");
    std::fs::remove_file(dir.join(".git").join("hooks").join("pre-commit")).unwrap();
    reload(&mut st);
    assert!(!asking(&st) && !st.held());
    assert_eq!(st.trust_standing(), Some(Standing::Trusted));
    // Putting it back doesn't ask either: the print still has it.
    add_hook(&dir, "pre-commit");
    reload(&mut st);
    assert!(!asking(&st) && !st.held());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_decision_saved_before_prints_asks_once() {
    let dir = untrusted_repo("print-legacy");
    let mut st = state();
    let canonical = glowfly_core::paths::resolve(&dir);
    record(&mut st.settings, &canonical, RepoTrust::Trusted);
    assert!(st.settings.repo_trust_prints.is_empty());
    let fx = run(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    finish_keeping(&mut st, fx);
    // Without a print it covers nothing that runs: whatever is there now wasn't seen.
    assert!(st.held() && asking(&st), "asked again");
    assert_eq!(st.trust_standing(), Some(Standing::Changed));
    assert_eq!(saved_print(&st), None, "nothing adopted unseen");
    let real = real(&st);
    drive(&mut st, Msg::Trust(TrustMsg::Decide { real, trusted: true }));
    assert!(!st.held(), "trusted on what the prompt showed");
    assert_eq!(saved_print(&st), Some(st.snap.as_ref().unwrap().trust_print.clone()));
    // From then on, something new asks.
    git(&dir, &["config", "credential.helper", "store"]);
    reload(&mut st);
    assert!(asking(&st));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn trust_saves_what_the_prompt_showed() {
    let dir = untrusted_repo("print-prompt-scan");
    let mut st = state();
    open(&mut st, &dir);
    assert!(asking(&st));
    let shown = prompt(&st).scan.print();
    // A hook appears and a load lands while the prompt is open.
    add_hook(&dir, "post-merge");
    reload(&mut st);
    assert_ne!(st.snap.as_ref().unwrap().trust_print, shown, "the snapshot moved on");
    let real = real(&st);
    drive(&mut st, Msg::Trust(TrustMsg::Decide { real, trusted: true }));
    assert_eq!(saved_print(&st), Some(shown), "what the user saw, not the newer scan");
    assert_eq!(st.trust_standing(), Some(Standing::Changed), "the hook they didn't see still asks");
    assert!(asking(&st));
    assert_eq!(prompt(&st).gained.as_ref().map(|g| g.hooks.clone()), Some(vec!["post-merge".to_string()]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_changed_repository_holds_jobs_and_offers_the_review() {
    let (mut st, dir) = trusted_repo("print-held-job");
    super::tests::clean_tree(&dir); // nothing for the auto-stash prompt to ask about
    git(&dir, &["config", "credential.helper", "store"]);
    reload(&mut st);
    drive(&mut st, Msg::CloseModal);
    let review = entries(&st).into_iter().find(|e| e.command == PaletteCommand::ReviewTrust).map(|e| e.detail);
    assert_eq!(review, Some(Some("changed".into())));

    let fx = run(&mut st, Msg::git("Pull", &["pull", "--ff"]));
    assert!(!fx.iter().any(|e| matches!(e, Effect::RunGit { .. })), "{fx:?}");
    assert_eq!(prompt(&st).held.as_deref(), Some("Pull"));
    assert!(prompt(&st).gained.is_some());
    run(&mut st, Msg::CloseModal);
    assert!(st.trust.held.is_none() && st.job.is_none(), "Cancel drops it");
    assert!(st.log.last().is_some_and(|l| l.text.contains("Pull wasn't run")));

    run(&mut st, Msg::git("Pull", &["pull", "--ff"]));
    let real = real(&st);
    let fx = run(&mut st, Msg::Trust(TrustMsg::Decide { real, trusted: true }));
    assert_eq!(fx.iter().filter(|e| matches!(e, Effect::RunGit { .. })).count(), 1, "Trust and run runs it: {fx:?}");
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Pull"));
    st.job = None;
    st.palette = Some(Palette::default());
    assert!(entries(&st).iter().all(|e| e.command != PaletteCommand::ReviewTrust));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn lfs_install_from_glowfly_doesn_t_ask_again() {
    let (mut st, dir) = trusted_repo("print-lfs-install");
    let mut fx = Vec::new();
    start_job(&mut st, JobSpec::new("Initialize Git LFS".into(), vec![glowfly_core::lfs::install_args()]), &mut fx);
    assert!(st.job.is_some());
    // What git lfs install writes, without needing git-lfs here.
    git(&dir, &["config", "filter.lfs.clean", "git-lfs clean -- %f"]);
    add_hook(&dir, "pre-push");
    done(&mut st, true, "Updated Git hooks.\nGit LFS initialized.");
    reload(&mut st);
    assert!(!asking(&st), "adopted without asking");
    assert!(!st.held());
    assert_eq!(saved_print(&st), Some(st.snap.as_ref().unwrap().trust_print.clone()));

    // Only once: a later change asks.
    add_hook(&dir, "post-merge");
    reload(&mut st);
    assert!(asking(&st));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Writes the repository's commands file with one shell command running `template`.
fn write_commands(dir: &Path, template: &str) {
    std::fs::create_dir_all(dir.join(".glowfly")).unwrap();
    let text = format!("[[command]]\nname = \"Hi\"\nkind = \"shell\"\ntemplate = \"{template}\"\n");
    std::fs::write(dir.join(".glowfly").join("commands.toml"), text).unwrap();
}

#[test]
fn a_changed_commands_file_asks_again_and_trusting_keeps_the_earlier_one() {
    let (mut st, dir) = trusted_repo("print-commands");
    write_commands(&dir, "echo one");
    reload(&mut st);
    assert!(asking(&st), "a new commands file in a trusted repository");
    assert!(prompt(&st).gained.as_ref().is_some_and(|g| g.commands.is_some()));
    let real = real(&st);
    drive(&mut st, Msg::Trust(TrustMsg::Decide { real: real.clone(), trusted: true }));
    write_commands(&dir, "echo two");
    reload(&mut st);
    assert_eq!(st.trust_standing(), Some(Standing::Changed), "the file changed");
    drive(&mut st, Msg::Trust(TrustMsg::Decide { real, trusted: true }));
    // Back to the first one, as on another branch: both were trusted.
    write_commands(&dir, "echo one");
    reload(&mut st);
    assert_eq!(st.trust_standing(), Some(Standing::Trusted));
    assert!(!asking(&st));
    let _ = std::fs::remove_dir_all(&dir);
}
