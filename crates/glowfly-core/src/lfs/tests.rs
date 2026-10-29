use super::*;
use crate::gitcli::{run, run_with_input};
use crate::testutil::{git, scratch_repo, with_origin, write};

fn displays(s: &LfsStatus) -> Vec<&str> {
    s.patterns.iter().map(|p| p.display.as_str()).collect()
}

pub(super) fn pat(raw: &str) -> LfsPattern {
    LfsPattern::root(raw, false)
}

pub(super) use crate::testutil::lfs_or_skip;

pub(super) fn ok(dir: &Path, a: Vec<String>) -> String {
    let (ok, out) = run(dir, &a);
    assert!(ok, "git {a:?} failed: {out}");
    out
}

#[test]
fn version_parsing() {
    assert_eq!(parse_version("git-lfs/3.7.1 (GitHub; windows amd64; go 1.25.1; git b84b3384)"), Some("3.7.1".into()));
    assert_eq!(parse_version("git: 'lfs' is not a git command. See 'git --help'."), None);
    assert_eq!(parse_version(""), None);
}

#[test]
fn attributes_parsing() {
    let text = "# comment\r\n\r\n[attr]binary -diff -merge -text\r\n*.psd filter=lfs diff=lfs merge=lfs -text\r\n\
                *.blend filter=lfs diff=lfs merge=lfs -text lockable\n*.txt text\n\
                my[[:space:]]dir/*.bin filter=lfs diff=lfs merge=lfs -text\n\
                \"quoted \\\"name\\\".dat\" filter=lfs\n";
    let p = parse_attributes(text);
    let raws: Vec<_> = p.iter().map(|p| p.raw.as_str()).collect();
    assert_eq!(raws, ["*.psd", "*.blend", "my[[:space:]]dir/*.bin", "quoted \"name\".dat"]);
    assert!(!p[0].lockable);
    assert!(p[1].lockable);
    assert_eq!(p[2].display, "my dir/*.bin");
    let hashed = parse_attributes("my[[:space:]]x\\#y filter=lfs\n");
    assert_eq!((hashed[0].raw.as_str(), hashed[0].display.as_str()), (r"my[[:space:]]x\#y", "my x#y"));
    assert_eq!(escape_pattern("my x#y"), hashed[0].raw);

    let later = "*.psd filter=lfs -text\n*.zip filter=lfs\n*.psd -filter\n*.zip filter=lfs lockable\n*.a filter=lfs\n*.a filter=other\n*.b filter=lfs\n*.b !filter\n";
    let p = parse_attributes(later);
    assert_eq!(p, vec![LfsPattern::root("*.zip", true)]);

    let replaced = parse_attributes("*.a filter=lfs\n*.b filter=lfs\n*.a filter=lfs lockable\n");
    assert_eq!(replaced.iter().map(|p| (p.raw.as_str(), p.lockable)).collect::<Vec<_>>(), [("*.a", true), ("*.b", false)]);
}

#[test]
fn pattern_checks() {
    assert_eq!(check_pattern("  ", &[]), Err(PatternProblem::Empty));
    assert_eq!(check_pattern("a\nb", &[]), Err(PatternProblem::Control));
    assert_eq!(check_pattern("!*.bin", &[]), Err(PatternProblem::Negated));
    let folder = check_pattern("assets/", &[]).unwrap_err();
    assert_eq!(folder, PatternProblem::Folder("assets/".into()));
    assert!(folder.message(Lang::En).contains("assets/**"), "{}", folder.message(Lang::En));
    assert_eq!(check_pattern(".gitattributes", &[]), Err(PatternProblem::GitFile));
    let existing = [pat("my[[:space:]]dir/*.bin")];
    let dup = check_pattern("my dir/*.bin", &existing).unwrap_err();
    assert_eq!(dup, PatternProblem::AlreadyTracked("my dir/*.bin".into()));
    assert_eq!(dup.message(Lang::En), "my dir/*.bin is already tracked.");
    assert_eq!(check_pattern(" *.zip ", &existing), Ok("*.zip".into()));
    let hashed = [pat(r"a\#b")];
    assert_eq!(hashed[0].display, "a#b");
    assert_eq!(check_pattern("a#b", &hashed), Err(PatternProblem::AlreadyTracked("a#b".into())));
    assert_eq!(PatternProblem::Empty.message(Lang::En), "Enter a pattern.");
}

#[test]
fn command_lines() {
    assert_eq!(LfsCommand::Push.args(Some("origin")), ["lfs", "push", "--all", "origin"]);
    assert_eq!(LfsCommand::Fetch.args(None), ["lfs", "fetch"]);
    assert_eq!(LfsCommand::Pull.args(Some("x")), ["lfs", "pull"]);
    assert_eq!(LfsCommand::Prune.args(None), ["lfs", "prune"]);
    assert_eq!(LfsCommand::Checkout.args(None), ["lfs", "checkout"]);
    assert_eq!(track_args("-x"), ["lfs", "track", "--", "-x"]);
    assert_eq!(untrack_args("*.a"), ["lfs", "untrack", "--", "*.a"]);
    assert_eq!(untrack_args(r"my[[:space:]]x\#y"), ["lfs", "untrack", "--", "my x#y"]);
    assert_eq!(install_args(), ["lfs", "install", "--local"]);
    let remote: Vec<_> = LfsCommand::ALL.iter().filter(|c| c.needs_remote()).copied().collect();
    assert_eq!(remote, [LfsCommand::Fetch, LfsCommand::Pull, LfsCommand::Push]);
    assert_eq!(LfsCommand::Prune.label(), "LFS prune");
    assert_eq!(LfsCommand::Checkout.name(), "Checkout");
}

#[test]
fn hook_states() {
    let dir = crate::paths::temp_dir().join(format!("glowfly-test-lfs-hooks-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    assert_eq!(hook_state(&dir), (false, vec![]));
    std::fs::write(dir.join("pre-push"), "#!/bin/sh\ngit lfs pre-push \"$@\"\n").unwrap();
    assert_eq!(hook_state(&dir), (true, vec![]));
    std::fs::write(dir.join("pre-push"), "#!/bin/sh\necho checks\n").unwrap();
    std::fs::write(dir.join("post-commit"), "#!/bin/sh\ncommand -v git-lfs >/dev/null\ngit lfs post-commit \"$@\"\n").unwrap();
    assert_eq!(hook_state(&dir), (false, vec!["pre-push".to_owned()]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn status_writes_nothing() {
    if lfs_or_skip("status_writes_nothing").is_none() {
        return;
    }
    let dir = scratch_repo("lfs-status-fresh");
    let s = status(&dir).unwrap();
    assert!(s.installed());
    assert!(!s.initialized);
    assert!(s.patterns.is_empty());
    assert!(!s.has_gitattributes);
    assert!(!dir.join(".git/hooks/pre-push").exists(), "status must not install hooks");
    let config = std::fs::read_to_string(dir.join(".git/config")).unwrap();
    assert!(!config.contains("lfs"), "status must not write LFS config:\n{config}");
}

#[test]
fn install_track_untrack() {
    if lfs_or_skip("install_track_untrack").is_none() {
        return;
    }
    let dir = scratch_repo("lfs-track");
    ok(&dir, install_args());
    assert!(status(&dir).unwrap().initialized);
    ok(&dir, track_args("*.bin"));
    ok(&dir, track_args("my dir/*.dat"));
    let s = status(&dir).unwrap();
    assert_eq!(displays(&s), ["*.bin", "my dir/*.dat"]);
    assert!(s.has_gitattributes);
    ok(&dir, untrack_args(&s.patterns[1].raw));
    assert_eq!(displays(&status(&dir).unwrap()), ["*.bin"]);
}

#[test]
fn track_and_untrack_a_hash_pattern() {
    if lfs_or_skip("track_and_untrack_a_hash_pattern").is_none() {
        return;
    }
    let dir = scratch_repo("lfs-track-hash");
    ok(&dir, install_args());
    ok(&dir, track_args("a#b"));
    ok(&dir, track_args("my dir/x#y z"));
    let s = status(&dir).unwrap();
    assert_eq!(displays(&s), ["a#b", "my dir/x#y z"]);
    assert_eq!(check_pattern("a#b", &s.patterns), Err(PatternProblem::AlreadyTracked("a#b".into())));
    ok(&dir, untrack_args(&s.patterns[0].raw));
    assert_eq!(displays(&status(&dir).unwrap()), ["my dir/x#y z"]);
    ok(&dir, untrack_args(&status(&dir).unwrap().patterns[0].raw));
    assert!(status(&dir).unwrap().patterns.is_empty());
}

#[test]
fn foreign_hook_stops_install() {
    if lfs_or_skip("foreign_hook_stops_install").is_none() {
        return;
    }
    let dir = scratch_repo("lfs-foreign");
    let hook = dir.join(".git/hooks/pre-push");
    std::fs::create_dir_all(hook.parent().unwrap()).unwrap();
    let text = "#!/bin/sh\necho my checks\n";
    std::fs::write(&hook, text).unwrap();
    assert_eq!(status(&dir).unwrap().foreign_hooks, ["pre-push"]);
    let (installed_ok, _) = run(&dir, &install_args());
    assert!(!installed_ok);
    assert_eq!(std::fs::read_to_string(&hook).unwrap(), text);
}

#[test]
fn push_and_pull_through_a_local_origin() {
    if lfs_or_skip("push_and_pull_through_a_local_origin").is_none() {
        return;
    }
    let (work, bare) = with_origin("lfs-push");
    ok(&work, install_args());
    ok(&work, track_args("*.bin"));
    write(&work, "a.bin", "real contents\n");
    git(&work, &["add", "."]);
    git(&work, &["commit", "-q", "-m", "lfs file"]);
    let (pushed, out) = run(&work, &LfsCommand::Push.args(Some("origin")));
    if !pushed {
        eprintln!("push_and_pull_through_a_local_origin: local LFS transfer unsupported, skipping: {out}");
        return;
    }
    git(&work, &["push", "-q", "origin", "main"]);

    let clone = work.with_file_name(format!("{}-lfs-clone", work.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&clone);
    let a = args(&["clone", "-q", &bare.display().to_string(), &clone.display().to_string()]);
    let (cloned, out) = run_with_input(clone.parent().unwrap(), &a, "", &[("GIT_LFS_SKIP_SMUDGE", "1")]);
    assert!(cloned, "{out}");
    let pointer = std::fs::read_to_string(clone.join("a.bin")).unwrap();
    assert!(pointer.starts_with("version https://git-lfs"), "{pointer}");
    ok(&clone, install_args());
    ok(&clone, LfsCommand::Pull.args(None));
    assert_eq!(std::fs::read_to_string(clone.join("a.bin")).unwrap(), "real contents\n");
    let _ = std::fs::remove_dir_all(&clone);
}

#[test]
fn pattern_problems_follow_the_language() {
    let folder = PatternProblem::Folder("assets/".into());
    assert_eq!(folder.message(Lang::En), "A pattern ending in / matches nothing in .gitattributes. To track a folder, use assets/**.");
    assert_eq!(PatternProblem::AlreadyTracked("*.psd".into()).message(Lang::En), "*.psd is already tracked.");
    let de = folder.message(Lang::De);
    assert!(de.contains("assets/**") && de != folder.message(Lang::En), "{de}");
    assert!(PatternProblem::AlreadyTracked("*.psd".into()).message(Lang::De).contains("*.psd"));
    assert_ne!(PatternProblem::GitFile.message(Lang::De), PatternProblem::GitFile.message(Lang::En));
}
