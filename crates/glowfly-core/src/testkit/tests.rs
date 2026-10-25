//! The kit's in-process commands leave a repository as the git CLI would: the same commits,
//! refs, reflog messages, index and files.

use super::commit::parse_date;
use super::{git_cli, git_env, set_identity, write};
use git2::Repository;
use std::path::{Path, PathBuf};

const DATE: &str = "2026-01-05T10:00:00+00:00";
const ENV: &[(&str, &str)] = &[("GIT_AUTHOR_DATE", DATE), ("GIT_COMMITTER_DATE", DATE)];

fn folder(name: &str) -> PathBuf {
    let dir = crate::paths::temp_dir().join(format!("glowfly-kit-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Runs `a` with the fixed dates, in-process (`fast`) or through the CLI.
fn step(dir: &Path, a: &[&str], fast: bool) {
    if fast {
        assert_eq!(super::fast::run(dir, a, ENV), Some(true), "git {a:?} ran in-process");
    } else {
        let (ok, out) = crate::gitcli::run_with_input(dir, &crate::gitcli::args(a), "", ENV);
        assert!(ok, "git {a:?}: {out}");
    }
}

/// A history built with every in-process form, the same way twice.
fn build(name: &str, fast: bool) -> PathBuf {
    let dir = folder(name);
    step(&dir, &["init", "-q", "-b", "main"], fast);
    set_identity(&dir);
    super::set_config(&dir, &[("core.autocrlf", "false")]);
    write(&dir, "a.txt", "one\n");
    std::fs::create_dir_all(dir.join("d")).unwrap();
    write(&dir, "d/b.txt", "b\n");
    step(&dir, &["add", "."], fast);
    step(&dir, &["commit", "-q", "-m", "first  ", "-m", "body"], fast);
    step(&dir, &["checkout", "-q", "-b", "side"], fast);
    write(&dir, "a.txt", "one\nside\n");
    step(&dir, &["commit", "-q", "-am", "side work"], fast);
    step(&dir, &["tag", "v1"], fast);
    step(&dir, &["checkout", "-q", "main"], fast);
    step(&dir, &["branch", "later", "side"], fast);
    step(&dir, &["config", "gitflow.branch.develop", "develop"], fast);
    let bare = dir.with_file_name(format!("{}-origin.git", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&bare);
    std::fs::create_dir_all(&bare).unwrap();
    step(&bare, &["init", "-q", "--bare", "-b", "main"], fast);
    step(&dir, &["remote", "add", "origin", &bare.display().to_string()], fast);
    step(&dir, &["push", "-q", "-u", "origin", "main"], fast);
    step(&dir, &["push", "-q", "origin", "side"], fast);
    step(&dir, &["update-ref", "refs/remotes/origin/later", "side"], fast);
    step(&dir, &["remote", "set-url", "origin", "https://example.invalid/r.git"], fast);
    step(&dir, &["checkout", "-q", "-b", "from-side", "side"], fast);
    step(&dir, &["checkout", "-q", "main"], fast);
    std::fs::remove_file(dir.join("d/b.txt")).unwrap();
    write(&dir, "c.txt", "c\n");
    step(&dir, &["add", "-A"], fast);
    step(&dir, &["commit", "-q", "-m", "second"], fast);
    step(&dir, &["-c", "user.name=Ada", "-c", "user.email=ada@example.com", "commit", "-q", "--allow-empty", "-m", "empty"], fast);
    assert!(super::fast::run(&dir, &["commit", "-q", "--amend", "-m", "amended"], ENV).is_none(), "git refuses to empty a commit");
    write(&dir, "c.txt", "changed\n");
    step(&dir, &["checkout", "--", "c.txt"], fast);
    step(&dir, &["update-index", "-q", "--refresh"], fast);
    write(&dir, "e.txt", "e\n");
    step(&dir, &["add", "e.txt"], fast);
    step(&dir, &["commit", "-q", "--author", "Grace Hopper <grace@example.com>", "-m", "by grace"], fast);
    write(&dir, "c.txt", "c2\n");
    step(&dir, &["commit", "-q", "-a", "--amend", "-m", "amended"], fast);
    let side = Repository::open(&dir).unwrap().revparse_single("side").unwrap().id().to_string();
    step(&dir, &["checkout", "-q", &side], fast);
    write(&dir, "a.txt", "local edit\n");
    dir
}

/// Everything about a repository the comparison looks at, with commit ids standing in for the
/// dates and identities they hash.
fn describe(dir: &Path) -> Vec<String> {
    let repo = Repository::open(dir).unwrap();
    let mut out = vec![format!("HEAD {:?} {}", repo.head().unwrap().name(), repo.head_detached().unwrap())];
    for r in repo.references().unwrap().flatten() {
        let name = r.name().unwrap().to_owned();
        out.push(format!("{name} {}", r.target().unwrap()));
        if let Ok(log) = repo.reflog(&name) {
            for e in log.iter() {
                out.push(format!("  {} -> {} {:?}", e.id_old(), e.id_new(), e.message()));
            }
        }
    }
    for e in repo.reflog("HEAD").unwrap().iter() {
        out.push(format!("HEAD log {} -> {} {:?}", e.id_old(), e.id_new(), e.message()));
    }
    for e in repo.index().ok().filter(|_| !repo.is_bare()).iter().flat_map(|i| i.iter()) {
        out.push(format!("index {} {:o} {}", String::from_utf8_lossy(&e.path), e.mode, e.id));
    }
    for f in ["a.txt", "c.txt", "d/b.txt", "e.txt"] {
        out.push(format!("file {f} {:?}", std::fs::read_to_string(dir.join(f)).ok()));
    }
    let text = std::fs::read_to_string(repo.path().join("config")).unwrap();
    out.extend(text.lines().map(|l| format!("config file: {l}")));
    let config = repo.config().unwrap().open_level(git2::ConfigLevel::Local).unwrap();
    for key in [
        "branch.main.remote",
        "branch.main.merge",
        "branch.side.remote",
        "remote.origin.url",
        "remote.origin.fetch",
        "gitflow.branch.develop",
        "core.bare",
        "core.logallrefupdates",
        "core.filemode",
        "core.ignorecase",
        "core.symlinks",
    ] {
        out.push(format!("{key} {:?}", config.get_string(key).ok()));
    }
    out
}

#[test]
fn in_process_commands_match_the_cli() {
    crate::partial::allow_extension();
    let cli = build("match-cli", false);
    let fast = build("match-fast", true);
    let (a, b) = (describe(&cli), describe(&fast));
    for (x, y) in a.iter().zip(&b) {
        assert_eq!(x, y, "the CLI's repository (left) and the kit's (right) differ");
    }
    assert_eq!(a.len(), b.len());
    let origin = |d: &Path| d.with_file_name(format!("{}-origin.git", d.file_name().unwrap().to_string_lossy()));
    let (a, b) = (describe(&origin(&cli)), describe(&origin(&fast)));
    assert_eq!(a, b, "the CLI's origin (left) and the kit's (right) differ");
    for d in [&cli, &fast] {
        let _ = std::fs::remove_dir_all(origin(d));
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn the_cli_runs_what_the_kit_does_not_handle() {
    let dir = folder("falls-back");
    git_env(&dir, &["init", "-q", "-b", "main"], &[]);
    set_identity(&dir);
    write(&dir, "a.txt", "one\n");
    // A hook makes commits go to git, which runs it.
    let hook = dir.join(".git/hooks/pre-commit");
    std::fs::write(&hook, "#!/bin/sh\necho hooked > hook-ran.txt\n").unwrap();
    #[cfg(unix)]
    std::fs::set_permissions(&hook, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    assert!(super::fast::run(&dir, &["add", "a.txt"], &[]).is_none(), "a repository with hooks goes to git");
    git_env(&dir, &["add", "a.txt"], &[]);
    git_env(&dir, &["commit", "-q", "-m", "first"], &[]);
    assert!(dir.join("hook-ran.txt").exists(), "git ran the hook");
    std::fs::remove_file(&hook).unwrap();
    assert!(super::fast::run(&dir, &["commit", "-q", "-m", "nothing"], &[]).is_none(), "git reports there is nothing to commit");
    assert!(super::fast::run(&dir, &["commit", "-q", "-S", "-m", "x"], &[]).is_none(), "unknown options go to git");
    assert!(super::fast::run(&dir, &["merge", "main"], &[]).is_none());
    write(
        &dir, "a.txt", "changed
",
    );
    // With -q, git refreshes what it can and succeeds even when a file changed.
    assert!(crate::gitcli::run(&dir, &crate::gitcli::args(&["update-index", "-q", "--refresh"])).0);
    assert_eq!(super::fast::run(&dir, &["update-index", "-q", "--refresh"], &[]), Some(true));
    write(&dir, ".gitattributes", "*.bin filter=lfs diff=lfs merge=lfs -text\n");
    assert!(super::fast::run(&dir, &["add", "."], &[]).is_none(), "attributes asking for a filter go to git");
    assert!(git_cli(&dir, &["status", "--porcelain"]).contains("?? .gitattributes"), "and nothing was staged");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn dates_in_the_forms_tests_pass() {
    let t = parse_date("2026-01-05T10:00:00+01:00").unwrap();
    assert_eq!((t.seconds(), t.offset_minutes()), (1_767_603_600, 60));
    assert_eq!(parse_date("2026-01-02T10:00:00Z").unwrap().seconds(), 1_767_348_000);
    let t = parse_date("1600000000 -0130").unwrap();
    assert_eq!((t.seconds(), t.offset_minutes()), (1_600_000_000, -90));
    assert_eq!(parse_date("@1600000000 +0000").unwrap().seconds(), 1_600_000_000);
    assert!(parse_date("yesterday").is_none());
}
