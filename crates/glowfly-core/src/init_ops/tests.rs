use super::*;
use crate::gitcli::{args, run as git_run};
use crate::testutil::scratch_repo;

#[test]
fn templates_are_clean_text() {
    for t in GitignoreTemplate::ALL {
        let body = t.body();
        assert!(body.trim().lines().count() >= 3, "{t:?}");
        assert!(!body.contains('\r') && body.ends_with('\n') && !body.ends_with("\n\n"), "{t:?}");
    }
    let os = lf(OS_EDITOR);
    assert!(!os.trim().is_empty() && !os.contains('\r') && os.ends_with('\n'));
    let labels: Vec<&str> = GitignoreTemplate::ALL.iter().map(|t| t.label()).collect();
    assert_eq!(labels, ["Rust", "Node.js", "Python", "Java / Kotlin", ".NET", "Go", "C / C++"]);
}

#[test]
fn gitignore_sections() {
    assert_eq!(gitignore_text(None, false), None);
    let both = gitignore_text(Some(GitignoreTemplate::Rust), true).unwrap();
    assert!(both.starts_with("# Rust\n") && both.contains("/target\n") && both.contains(".DS_Store\n"), "{both}");
    assert!(both.contains("\n\n# Operating system and editor files\n"), "{both}");
    assert!(both.ends_with('\n') && !both.ends_with("\n\n"));
    let os = gitignore_text(None, true).unwrap();
    assert!(os.starts_with("# Operating system and editor files\n") && !os.contains("/target"), "{os}");
}

#[test]
fn license_texts() {
    let mit = license_text(License::Mit, 2026, "Ada Lovelace");
    assert!(mit.starts_with("MIT License\n"));
    assert!(mit.contains("Copyright (c) 2026 Ada Lovelace\n"));
    let bsd3 = license_text(License::Bsd3, 2026, "Ada Lovelace");
    assert!(bsd3.contains("Copyright (c) 2026, Ada Lovelace\n"));
    for n in ["1. Redistributions", "2. Redistributions", "3. Neither"] {
        assert!(bsd3.contains(n), "{n}");
    }
    let bsd2 = license_text(License::Bsd2, 2026, "Ada");
    assert!(bsd2.contains("2. Redistributions") && !bsd2.contains("3. Neither"));
    let apache = license_text(License::Apache2, 2026, "Ada");
    let first: Vec<&str> = apache.lines().take(2).map(str::trim).collect();
    assert_eq!(first, ["Apache License", "Version 2.0, January 2004"]);
    let gpl = license_text(License::Gpl3, 2026, "Ada");
    assert!(gpl.contains("Version 3, 29 June 2007"));
    assert_eq!(gpl.lines().count(), 674);
    assert!(license_text(License::Mpl2, 2026, "Ada").starts_with("Mozilla Public License Version 2.0\n"));
    assert!(license_text(License::Unlicense, 2026, "Ada").contains("unencumbered software released into the public domain"));
    for l in License::ALL {
        let text = license_text(l, 2026, "Ada");
        assert!(!text.contains('\r') && !text.contains("{year}") && !text.contains("{holder}"), "{l:?}");
        assert!(text.ends_with('\n') && !text.ends_with("\n\n"), "{l:?}");
        assert_eq!(text.contains("Ada"), l.needs_holder(), "{l:?}");
    }
    let table: Vec<(&str, bool)> = License::ALL.iter().map(|l| (l.spdx(), l.needs_holder())).collect();
    assert_eq!(
        table,
        [
            ("MIT", true),
            ("Apache-2.0", false),
            ("BSD-2-Clause", true),
            ("BSD-3-Clause", true),
            ("GPL-3.0-only", false),
            ("MPL-2.0", false),
            ("Unlicense", false)
        ]
    );
}

#[test]
fn holder_and_init_command() {
    assert_eq!(check_holder(""), Err(HolderProblem::Empty));
    assert_eq!(check_holder("   "), Err(HolderProblem::Empty));
    assert_eq!(check_holder("a\nb"), Err(HolderProblem::NotOneLine));
    assert_eq!(check_holder("a\u{7}b"), Err(HolderProblem::NotOneLine));
    assert_eq!(check_holder(&"x".repeat(201)), Err(HolderProblem::TooLong));
    assert_eq!(check_holder(&"x".repeat(200)), Ok("x".repeat(200)));
    assert_eq!(check_holder("  Ada "), Ok("Ada".into()));
    assert_eq!(init_cmd(None), ["init"]);
    assert_eq!(init_cmd(Some("main")), ["init", "--initial-branch=main"]);
}

/// A fresh, empty temporary folder for `name`.
fn temp(name: &str) -> PathBuf {
    let dir = crate::paths::temp_dir().join(format!("glowfly-test-init-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn inspects_the_folder() {
    let dir = temp("inspect");
    assert_eq!(inspect_folder("relative/x", None), None);
    let missing = inspect_folder(&format!(" {} ", dir.join("new").display()), None).unwrap();
    assert!(!missing.exists && !missing.repo);
    assert_eq!(missing.path, crate::paths::resolve(&dir).join("new"));
    assert_eq!(inspect_folder(&dir.join("no/such").display().to_string(), None), None);
    std::fs::write(dir.join("file.txt"), "x").unwrap();
    assert_eq!(inspect_folder(&dir.join("file.txt").display().to_string(), None), None);

    let empty = dir.join("empty");
    std::fs::create_dir_all(&empty).unwrap();
    let f = inspect_folder(&empty.display().to_string(), None).unwrap();
    assert!(f.exists && !f.repo && !f.has_files && !f.gitignore && f.license.is_none());
    std::fs::create_dir_all(empty.join(".git")).unwrap();
    std::fs::write(empty.join("License.md"), "x").unwrap();
    std::fs::write(empty.join(".gitignore"), "x").unwrap();
    let f = inspect_folder(&empty.display().to_string(), None).unwrap();
    assert!(f.repo && f.has_files && f.gitignore);
    assert_eq!(f.license.as_deref(), Some("License.md"));

    let f = inspect_folder("~/empty", Some(&dir)).unwrap();
    assert!(f.exists && f.repo);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A fixed identity and no signing, for the commits these tests make (whatever the developer's
/// global config says).
const IDENTITY: [(&str, &str); 7] = [
    ("GIT_AUTHOR_NAME", "Test"),
    ("GIT_AUTHOR_EMAIL", "test@example.com"),
    ("GIT_COMMITTER_NAME", "Test"),
    ("GIT_COMMITTER_EMAIL", "test@example.com"),
    ("GIT_CONFIG_COUNT", "1"),
    ("GIT_CONFIG_KEY_0", "commit.gpgsign"),
    ("GIT_CONFIG_VALUE_0", "false"),
];

fn files(year: i32) -> Vec<NewFile> {
    vec![
        NewFile { name: ".gitignore".into(), contents: gitignore_text(Some(GitignoreTemplate::Rust), false).unwrap() },
        NewFile { name: LICENSE_FILE.into(), contents: license_text(License::Mit, year, "Ada") },
    ]
}

fn out(dir: &Path, a: &[&str]) -> (bool, String) {
    let (ok, o) = git_run(dir, &args(a));
    (ok, o.trim().to_owned())
}

#[test]
fn init_writes_and_commits_the_chosen_files() {
    let dir = temp("run-commit");
    let folder = dir.join("nested/app");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("notes.txt"), "mine").unwrap();
    let plan = InitPlan { branch: Some("main".into()), files: files(2026), commit: Some(INITIAL_COMMIT.into()) };
    let o = run_env(&folder, &plan, &IDENTITY);
    assert!(o.initialized && o.ok, "{}", o.output);
    assert!(o.output.contains("Initialized empty Git repository"), "{}", o.output);
    assert_eq!(out(&folder, &["log", "-1", "--format=%s"]).1, "Initial commit");
    assert_eq!(out(&folder, &["rev-list", "--count", "HEAD"]).1, "1");
    let tree = out(&folder, &["ls-tree", "--name-only", "HEAD"]).1;
    assert_eq!(tree.lines().collect::<Vec<_>>(), [".gitignore", "LICENSE"]);
    assert_eq!(out(&folder, &["status", "--porcelain"]).1, "?? notes.txt", "a file already there stays untracked");

    // Run again: git reinitializes, both files are kept and nothing new is committed.
    let o = run_env(&folder, &plan, &IDENTITY);
    assert!(o.initialized && o.ok, "{}", o.output);
    assert!(o.output.contains("Reinitialized") && o.output.contains("Kept the existing LICENSE."), "{}", o.output);
    assert_eq!(out(&folder, &["rev-list", "--count", "HEAD"]).1, "1");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn without_the_commit_option_nothing_is_committed() {
    let dir = temp("run-nocommit");
    let folder = dir.join("a/b");
    let o = run_env(&folder, &InitPlan { branch: Some("trunk".into()), files: files(2026), commit: None }, &IDENTITY);
    assert!(o.initialized && o.ok, "{}", o.output);
    assert!(o.output.contains("Initialized empty Git repository") && o.output.contains("Wrote LICENSE."), "{}", o.output);
    assert!(folder.join(".gitignore").is_file() && folder.join("LICENSE").is_file());
    assert!(!out(&folder, &["rev-parse", "--verify", "-q", "HEAD"]).0, "no commit");
    assert_eq!(out(&folder, &["symbolic-ref", "HEAD"]).1, "refs/heads/trunk");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_existing_gitignore_is_kept() {
    let dir = temp("run-keep");
    std::fs::write(dir.join(".gitignore"), b"keep\r\nme\n").unwrap();
    let o = run_env(&dir, &InitPlan { branch: None, files: files(2026), commit: None }, &IDENTITY);
    assert!(o.initialized && o.ok, "{}", o.output);
    assert!(o.output.contains("Kept the existing .gitignore."), "{}", o.output);
    assert_eq!(std::fs::read(dir.join(".gitignore")).unwrap(), b"keep\r\nme\n");
    assert!(dir.join("LICENSE").is_file());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failing_commit_still_leaves_the_repository() {
    let dir = scratch_repo("init-hook");
    let hooks = dir.join(".git/hooks");
    std::fs::create_dir_all(&hooks).unwrap();
    std::fs::write(hooks.join("pre-commit"), "#!/bin/sh\necho no commits here >&2\nexit 1\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(hooks.join("pre-commit"), std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let o = run(&dir, &InitPlan { branch: None, files: files(2026), commit: Some(INITIAL_COMMIT.into()) });
    assert!(o.initialized && !o.ok, "{}", o.output);
    assert_eq!(o.failed.as_deref(), Some("the commit"));
    assert!(dir.join(".gitignore").is_file() && dir.join("LICENSE").is_file());
    let _ = std::fs::remove_dir_all(&dir);
}
