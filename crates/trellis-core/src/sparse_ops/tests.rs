use super::*;
use crate::testutil::*;
use std::path::PathBuf;

fn cone(text: &str) -> std::result::Result<Vec<String>, PathProblem> {
    parse_paths(text, SparseMode::Cone)
}

fn strings(a: &[&str]) -> Vec<String> {
    a.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn cone_paths_are_cleaned_up() {
    assert_eq!(cone(" src/ \n\n./docs\\guide\nsrc"), Ok(strings(&["src", "docs/guide"])));
    assert_eq!(cone("/lib//\r\n"), Ok(strings(&["lib"])));
    assert_eq!(cone(""), Ok(vec![]));
    assert_eq!(cone("  \n\n"), Ok(vec![]));
}

#[test]
fn cone_refuses_patterns_and_paths_outside() {
    for p in ["*.md", "assets/[ab]", "!docs", "src/?"] {
        let e = cone(&format!("src\n{p}")).unwrap_err();
        assert_eq!(e, PathProblem::Pattern(p.into()));
        assert!(e.message().contains(p) && e.message().contains("Patterns"), "{}", e.message());
    }
    for p in ["a/../b", ".", "C:/x", "/../x", "..", "\\\\server\\share"] {
        let e = cone(p).unwrap_err();
        assert_eq!(e, PathProblem::Outside(p.into()), "{p}");
        assert!(e.message().contains(p), "{}", e.message());
    }
}

#[test]
fn patterns_are_kept_as_typed() {
    let text = "/*  \r\n!/docs/\n\n*.md\t\n";
    assert_eq!(parse_paths(text, SparseMode::Patterns), Ok(strings(&["/*", "!/docs/", "*.md"])));
    assert_eq!(parse_paths("", SparseMode::Patterns), Err(PathProblem::NoPatterns));
    assert_eq!(parse_paths(" \n\t\n", SparseMode::Patterns), Err(PathProblem::NoPatterns));
    assert!(PathProblem::NoPatterns.message().contains("every file"));
}

#[test]
fn command_lines_and_input() {
    let set = SparseCommand::Set { mode: SparseMode::Cone, paths: strings(&["src", "docs"]) };
    assert_eq!(set.args(), strings(&["sparse-checkout", "set", "--cone", "--no-sparse-index", "--stdin"]));
    assert_eq!(set.input(), "src\ndocs\n");
    let patterns = SparseCommand::Set { mode: SparseMode::Patterns, paths: strings(&["/*"]) };
    assert_eq!(patterns.args(), strings(&["sparse-checkout", "set", "--no-cone", "--no-sparse-index", "--stdin"]));
    assert_eq!(SparseCommand::Set { mode: SparseMode::Cone, paths: vec![] }.input(), "");
    assert_eq!(SparseCommand::Reapply.args(), strings(&["sparse-checkout", "reapply"]));
    assert_eq!(SparseCommand::Disable.args(), strings(&["sparse-checkout", "disable"]));
    assert_eq!(SparseCommand::Reapply.input(), "");
}

#[test]
fn unquoting_and_left_behind_files() {
    assert_eq!(unquote("\"q\\\"z\""), "q\"z");
    assert_eq!(unquote("\"caf\\303\\251\""), "café");
    assert_eq!(unquote("\"a\\tb\""), "a\tb");
    assert_eq!(unquote("\"back\\\\slash\""), "back\\slash");
    assert_eq!(unquote("plain/path"), "plain/path");
    let out = "$ git sparse-checkout set --cone --no-sparse-index --stdin\n  src\nwarning: The following paths are not up to date \
               and were left despite sparse patterns:\n\tdocs/d.txt\n\nAfter fixing the above paths, you may want to run `git \
               sparse-checkout reapply`.";
    assert_eq!(left_behind(out), strings(&["docs/d.txt"]));
    assert!(left_behind("$ git sparse-checkout reapply").is_empty());
}

/// A repository with r.txt, src/a/x.txt and docs/d.txt committed.
fn fixture(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    std::fs::create_dir_all(dir.join("src/a")).unwrap();
    std::fs::create_dir_all(dir.join("docs")).unwrap();
    write(&dir, "r.txt", "r\n");
    write(&dir, "src/a/x.txt", "x\n");
    write(&dir, "docs/d.txt", "d\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    dir
}

fn set(dir: &Path, mode: SparseMode, paths: &[&str]) -> String {
    let (ok, out) = run(dir, &SparseCommand::Set { mode, paths: strings(paths) });
    assert!(ok, "{out}");
    out
}

#[test]
fn turning_on_with_folders_and_off_again() {
    let dir = fixture("sparse-cone");
    let s = status(&dir).unwrap();
    assert_eq!((s.enabled, s.paths.len(), s.tracked, s.skipped), (false, 0, 3, 0));

    let out = set(&dir, SparseMode::Cone, &["src"]);
    assert!(out.starts_with("$ git sparse-checkout set --cone --no-sparse-index --stdin\n  src"), "{out}");
    assert!(!dir.join("docs/d.txt").exists());
    assert!(dir.join("r.txt").exists() && dir.join("src/a/x.txt").exists());
    let s = status(&dir).unwrap();
    assert_eq!(s, SparseStatus { enabled: true, mode: SparseMode::Cone, paths: strings(&["src"]), tracked: 3, skipped: 1, ..s.clone() });
    assert!(!s.fell_back && !s.sparse_index);

    let out = set(&dir, SparseMode::Cone, &[]);
    assert!(out.contains("(no paths: only files at the root)"), "{out}");
    assert!(dir.join("r.txt").exists() && !dir.join("src/a/x.txt").exists());
    let s = status(&dir).unwrap();
    assert!(s.enabled && s.paths.is_empty() && s.skipped == 2, "{s:?}");

    let (ok, out) = run(&dir, &SparseCommand::Disable);
    assert!(ok, "{out}");
    assert!(dir.join("docs/d.txt").exists() && dir.join("src/a/x.txt").exists());
    let s = status(&dir).unwrap();
    assert!(!s.enabled && s.skipped == 0, "{s:?}");
    let (ok, _) = run(&dir, &SparseCommand::Reapply);
    assert!(!ok, "git refuses reapply when sparse checkout is off");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn patterns_mode_and_cone_fallback() {
    let dir = fixture("sparse-patterns");
    set(&dir, SparseMode::Patterns, &["/*", "!/docs/"]);
    assert!(!dir.join("docs/d.txt").exists() && dir.join("src/a/x.txt").exists());
    let s = status(&dir).unwrap();
    assert_eq!((s.mode, s.paths.clone(), s.fell_back), (SparseMode::Patterns, strings(&["/*", "!/docs/"]), false));

    git(&dir, &["config", "--worktree", "core.sparseCheckoutCone", "true"]);
    let s = status(&dir).unwrap();
    assert_eq!((s.mode, s.fell_back), (SparseMode::Patterns, true));

    git(&dir, &["config", "--worktree", "--unset", "core.sparseCheckoutCone"]);
    assert_eq!(status(&dir).unwrap().mode, SparseMode::Patterns, "unset reads as patterns, as git does");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unusual_folder_names_round_trip() {
    let dir = fixture("sparse-names");
    std::fs::create_dir_all(dir.join("café")).unwrap();
    write(&dir, "café/f.txt", "c\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "cafe"]);
    set(&dir, SparseMode::Cone, &["q\"z", "café"]);
    let s = status(&dir).unwrap();
    assert_eq!(s.mode, SparseMode::Cone);
    assert!(s.paths.contains(&"q\"z".to_owned()) && s.paths.contains(&"café".to_owned()), "{:?}", s.paths);
    assert!(dir.join("café/f.txt").exists() && !dir.join("docs/d.txt").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn changed_files_outside_the_paths_are_kept() {
    let dir = fixture("sparse-kept");
    set(&dir, SparseMode::Cone, &["docs"]);
    write(&dir, "docs/d.txt", "changed\n");
    let out = set(&dir, SparseMode::Cone, &["src"]);
    assert_eq!(left_behind(&out), strings(&["docs/d.txt"]), "{out}");
    assert!(dir.join("docs/d.txt").exists());

    git(&dir, &["checkout", "--", "docs/d.txt"]);
    let (ok, out) = run(&dir, &SparseCommand::Reapply);
    assert!(ok, "{out}");
    assert!(out.starts_with("$ git sparse-checkout reapply"), "{out}");
    assert!(!dir.join("docs/d.txt").exists());
    let _ = std::fs::remove_dir_all(&dir);
}
