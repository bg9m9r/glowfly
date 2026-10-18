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
    assert_eq!(super::status_cli::status_within(&dir, crate::gitcli::READ_TIMEOUT), Ok(git_read.clone()), "the same read");
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
    let (unstaged, staged) = parse(text.as_bytes());
    let paths: Vec<_> = unstaged.iter().map(|f| (f.path.as_str(), f.status)).collect();
    assert_eq!(paths, [("both.txt", '!'), ("x y.txt", '?')]);
    assert!(staged.is_empty());
}

#[test]
fn untracked_names_keep_their_leading_spaces() {
    let (unstaged, _) = parse(b"?  a.txt\0? a.txt\0?   \0");
    let paths: Vec<_> = unstaged.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, ["  ", " a.txt", "a.txt"]);
}

#[test]
fn git_s_status_tells_a_leading_space_apart() {
    let dir = scratch_repo("status-cli-space");
    write(&dir, "t.txt", "t\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    git(&dir, &["config", "core.untrackedCache", "true"]);
    write(&dir, " a.txt", "space\n");
    write(&dir, "a.txt", "plain\n");
    let (unstaged, _, _) = status_at(&dir, true).unwrap();
    let paths: Vec<_> = unstaged.iter().map(|f| (f.path.as_str(), f.status)).collect();
    assert_eq!(paths, [(" a.txt", '?'), ("a.txt", '?')]);
    let repo = Repository::open(&dir).unwrap();
    assert_eq!(libgit2_status(&repo).unwrap().0, unstaged, "libgit2 reads the same names");
    let _ = std::fs::remove_dir_all(&dir);
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
    let hook = marker_script(&dir.join(".git/fsmon.sh"), &marker);
    git(&dir, &["config", "core.fsmonitor", &hook]);

    status_at(&dir, false).unwrap();
    load(&dir, &LoadOptions::all(100)).unwrap();
    assert!(!marker.exists(), "neither a libgit2 status nor an untrusted load runs the hook");
    // The same read with git allowed does run it, so the check above means something.
    status_at(&dir, true).unwrap();
    assert!(marker.exists(), "git ran the fsmonitor hook");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Writes a hook script that creates `marker`, and returns its path with forward slashes.
pub(super) fn marker_script(path: &std::path::Path, marker: &std::path::Path) -> String {
    let slashes = |p: &std::path::Path| p.display().to_string().replace('\\', "/");
    std::fs::write(path, format!("#!/bin/sh\ntouch \"{}\"\nexit 1\n", slashes(marker))).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    slashes(path)
}

#[test]
fn a_submodule_s_fsmonitor_hook_never_runs_untrusted() {
    let (top, sub) = (scratch_repo("status-sub-top"), scratch_repo("status-sub-lib"));
    write(&sub, "lib.txt", "lib\n");
    git(&sub, &["add", "."]);
    git(&sub, &["commit", "-q", "-m", "lib"]);
    git(&top, &["-c", "protocol.file.allow=always", "submodule", "add", "-q", &sub.display().to_string(), "lib"]);
    git(&top, &["commit", "-q", "-m", "add lib"]);
    // Nothing flagged in the top repository, and set up so that git reads its status.
    git(&top, &["config", "core.untrackedCache", "true"]);
    let marker = top.join("ran.txt");
    let hook = marker_script(&top.join(".git/modules/lib/fsmon.sh"), &marker);
    git(&top.join("lib"), &["config", "core.fsmonitor", &hook]);

    let snap = load(&top, &LoadOptions::all(100)).unwrap();
    assert_eq!(snap.trust_scan.submodules.len(), 1, "{:?}", snap.trust_scan);
    assert!(!marker.exists(), "an untrusted load doesn't let git step into the submodule");
    status_at(&top, true).unwrap();
    assert!(marker.exists(), "git's status ran the submodule's hook");
    let _ = std::fs::remove_dir_all(&top);
    let _ = std::fs::remove_dir_all(&sub);
}

#[test]
fn a_hasconfig_include_s_fsmonitor_hook_never_runs_untrusted() {
    let dir = scratch_repo("status-hasconfig");
    write(&dir, "a.txt", "a\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    git(&dir, &["config", "remote.origin.url", "https://example.invalid/r.git"]);
    git(&dir, &["config", "core.untrackedCache", "true"]);
    let marker = dir.join("ran.txt");
    let hook = marker_script(&dir.join(".git/fsmon.sh"), &marker);
    std::fs::write(dir.join(".git/evil.cfg"), format!("[core]\n\tfsmonitor = {hook}\n")).unwrap();
    git(&dir, &["config", "includeIf.hasconfig:remote.*.url:https://**.path", "evil.cfg"]);

    assert!(!load(&dir, &LoadOptions::all(100)).unwrap().trust_scan.is_empty());
    assert!(!marker.exists(), "an untrusted load doesn't let git read the status");
    status_at(&dir, true).unwrap();
    assert!(marker.exists(), "git followed the include and ran the hook");
    let _ = std::fs::remove_dir_all(&dir);
}
