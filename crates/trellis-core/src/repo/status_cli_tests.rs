use super::status::libgit2_status;
use super::status_cli::{parse, status};
use super::*;
use crate::testutil::*;
use git2::Repository;

#[test]
fn git_and_libgit2_read_the_same_status() {
    let dir = scratch_repo("status-cli");
    for f in ["a.txt", "b.txt", "c.txt", "d.txt", "e.txt"] {
        write(&dir, f, &format!("{f}\n"));
    }
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    write(&dir, "a.txt", "changed\n");
    write(&dir, "b.txt", "staged\n");
    git(&dir, &["add", "b.txt"]);
    write(&dir, "b.txt", "and changed again\n");
    std::fs::remove_file(dir.join("c.txt")).unwrap();
    git(&dir, &["rm", "-q", "d.txt"]);
    git(&dir, &["mv", "e.txt", "renamed.txt"]);
    write(&dir, "new.txt", "new\n");
    git(&dir, &["add", "new.txt"]);
    std::fs::create_dir_all(dir.join("deep/er")).unwrap();
    write(&dir, "deep/er/untracked file.txt", "u\n");

    let repo = Repository::open(&dir).unwrap();
    let ours = libgit2_status(&repo).unwrap();
    let git_read = status(&dir).expect("git ran");
    assert_eq!(git_read, ours);
    assert!(git_read.1.iter().any(|f| f.path == "renamed.txt" && f.status == 'R'));
    assert!(git_read.0.iter().any(|f| f.path == "deep/er/untracked file.txt" && f.status == '?'));

    // A repository set up for git's fast status reads it through git; the lists don't change.
    git(&dir, &["config", "core.untrackedCache", "true"]);
    assert_eq!(status_at(&dir, true).map(|(u, s, _)| (u, s)).unwrap(), ours);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn conflicts_and_odd_records_parse() {
    let text = "u UU N... 100644 100644 100644 100644 a b c both.txt\0? x y.txt\0! ignored\0# branch.oid abc\0";
    let (unstaged, staged) = parse(text);
    let paths: Vec<_> = unstaged.iter().map(|f| (f.path.as_str(), f.status)).collect();
    assert_eq!(paths, [("both.txt", '!'), ("x y.txt", '?')]);
    assert!(staged.is_empty());
}

#[test]
fn loads_report_what_runs_programs() {
    let dir = scratch_repo("load-trust-scan");
    assert!(load(&dir, &LoadOptions::all(100)).unwrap().trust_scan.is_empty());
    git(&dir, &["config", "core.sshCommand", "ssh -v"]);
    let scan = load(&dir, &LoadOptions::all(100)).unwrap().trust_scan;
    assert_eq!(scan.config.iter().map(|c| c.key.as_str()).collect::<Vec<_>>(), ["core.sshCommand"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_untrusted_fsmonitor_hook_never_runs() {
    let dir = scratch_repo("status-fsmonitor-hook");
    write(&dir, "a.txt", "a\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    let marker = dir.join("ran.txt");
    let slashes = |p: &std::path::Path| p.display().to_string().replace('\\', "/");
    let hook = dir.join(".git/fsmon.sh");
    std::fs::write(&hook, format!("#!/bin/sh\ntouch \"{}\"\nexit 1\n", slashes(&marker))).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    git(&dir, &["config", "core.fsmonitor", &slashes(&hook)]);

    status_at(&dir, false).unwrap();
    load(&dir, &LoadOptions::all(100)).unwrap();
    assert!(!marker.exists(), "neither a libgit2 status nor an untrusted load runs the hook");
    // The same read with git allowed does run it, so the check above means something.
    status_at(&dir, true).unwrap();
    assert!(marker.exists(), "git ran the fsmonitor hook");
    let _ = std::fs::remove_dir_all(&dir);
}
