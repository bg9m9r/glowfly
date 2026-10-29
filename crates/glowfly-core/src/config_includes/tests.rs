//! Include resolution on fixture repositories, checked against what git itself reads: each
//! included file sets `mark.name` to its own name, and `git config --includes --get-all` lists
//! the names git read. Then `~/`, conditions git can't be asked about here, cycles and bounds.

use super::*;
use crate::testkit::git_cli;
use crate::testutil::{git, scratch_repo, write};
use std::io::Write as _;

/// `p` with forward slashes, as a config value or pattern names it on every OS.
fn fwd(p: &Path) -> String {
    p.display().to_string().replace('\\', "/")
}

fn append(file: &Path, text: &str) {
    std::fs::OpenOptions::new().append(true).open(file).unwrap().write_all(text.as_bytes()).unwrap();
}

/// Writes `dir/name`, which sets `mark.name` to `name`, then `extra`.
fn mark(dir: &Path, name: &str, extra: &str) -> PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    let p = dir.join(name);
    std::fs::write(&p, format!("[mark]\n\tname = {name}\n{extra}")).unwrap();
    p
}

fn names(found: &Included) -> Vec<String> {
    found.files.iter().map(|f| f.file_name().unwrap().to_string_lossy().into_owned()).collect()
}

/// The marks git reads for the repository at `dir`, in its order.
fn git_reads(dir: &Path) -> Vec<String> {
    git_cli(dir, &["config", "--local", "--includes", "--get-all", "mark.name"]).lines().map(str::to_owned).collect()
}

/// A folder of its own next to the scratch repositories, emptied.
fn folder(name: &str) -> PathBuf {
    let dir = crate::paths::temp_dir().join(format!("glowfly-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn relative_paths_resolve_from_the_including_file_inside_and_outside_the_git_dir() {
    let dir = scratch_repo("incl-relative");
    let gd = dir.join(".git");
    let outside = folder("incl-relative-outside");
    let b = mark(&outside, "b.cfg", "");
    let a = mark(&outside, "a.cfg", "[include]\n\tpath = b.cfg\n");
    let c = mark(&gd, "c.cfg", "");
    append(&gd.join("config"), &format!("[include]\n\tpath = {}\n\tpath = c.cfg\n", fwd(&a)));
    let found = of_repo(&gd, None);
    assert_eq!(found.files, [&a, &b, &c].map(|p| crate::paths::resolve(p)));
    assert_eq!(names(&found), git_reads(&dir));
    assert!(!found.on_branch);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&outside);
}

#[test]
fn conditions_match_what_git_includes() {
    let dir = scratch_repo("incl-cond");
    write(&dir, "a.txt", "a\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "a"]);
    let gd = dir.join(".git");
    let name = dir.file_name().unwrap().to_string_lossy().into_owned();
    for f in ["yes-gitdir", "no-gitdir", "yes-anywhere", "yes-icase", "yes-branch", "no-branch", "yes-feature", "never"] {
        mark(&gd, &format!("{f}.cfg"), "");
    }
    mark(&dir, "yes-dot.cfg", "");
    mark(&dir, "inner.cfg", "[includeIf \"gitdir:./.git\"]\n\tpath = yes-dot.cfg\n");
    let at = fwd(&dir);
    append(
        &gd.join("config"),
        &format!(
            "[includeIf \"gitdir:{at}/\"]\n\tpath = yes-gitdir.cfg\n\
             [includeIf \"gitdir:/nowhere/\"]\n\tpath = no-gitdir.cfg\n\
             [includeIf \"gitdir:{name}/.git\"]\n\tpath = yes-anywhere.cfg\n\
             [includeIf \"gitdir/i:{upper}/\"]\n\tpath = yes-icase.cfg\n\
             [includeIf \"onbranch:main\"]\n\tpath = yes-branch.cfg\n\
             [includeIf \"onbranch:other\"]\n\tpath = no-branch.cfg\n\
             [includeIf \"onbranch:feature/\"]\n\tpath = yes-feature.cfg\n\
             [includeIf \"nonsense:x\"]\n\tpath = never.cfg\n\
             [include]\n\tpath = ../inner.cfg\n",
            upper = at.to_uppercase()
        ),
    );
    let found = of_repo(&gd, None);
    assert_eq!(names(&found), ["yes-gitdir.cfg", "yes-anywhere.cfg", "yes-icase.cfg", "yes-branch.cfg", "inner.cfg", "yes-dot.cfg"]);
    assert_eq!(names(&found), git_reads(&dir));
    assert!(found.on_branch, "a branch switch can change the files");

    git(&dir, &["checkout", "-q", "-b", "feature/x"]);
    let found = of_repo(&gd, None);
    assert_eq!(names(&found), ["yes-gitdir.cfg", "yes-anywhere.cfg", "yes-icase.cfg", "yes-feature.cfg", "inner.cfg", "yes-dot.cfg"]);
    assert_eq!(names(&found), git_reads(&dir));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_linked_worktree_reads_the_common_config_with_its_own_git_dir() {
    let dir = scratch_repo("incl-worktree");
    write(&dir, "a.txt", "a\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "a"]);
    let wt = dir.with_file_name(format!("{}-wt", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&wt);
    git(&dir, &["worktree", "add", "-q", "-b", "side", &fwd(&wt)]);
    let gd = dir.join(".git");
    let linked = gd.join("worktrees").join(wt.file_name().unwrap());
    mark(&gd, "main-only.cfg", "");
    mark(&gd, "side-only.cfg", "");
    append(
        &gd.join("config"),
        "[includeIf \"onbranch:main\"]\n\tpath = main-only.cfg\n[includeIf \"gitdir:**/worktrees/**\"]\n\tpath = side-only.cfg\n",
    );
    assert_eq!(names(&of_repo(&gd, None)), ["main-only.cfg"]);
    let found = of_repo(&linked, Some(&gd));
    assert_eq!(names(&found), ["side-only.cfg"]);
    assert_eq!(names(&found), git_reads(&wt));
    git(&dir, &["worktree", "remove", "--force", &fwd(&wt)]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn home_paths_missing_files_and_conditions_left_to_git() {
    let dir = folder("incl-home");
    let home = dir.join("home");
    let h = mark(&home, "h.cfg", "");
    let root = mark(
        &dir,
        "root.cfg",
        "[include]\n\tpath = ~/h.cfg\n\tpath = ~other/x.cfg\n\tpath = %(prefix)/etc/x.cfg\n\tpath = missing.cfg\n\
         [includeIf \"hasconfig:remote.*.url:https://example.com/**\"]\n\tpath = has.cfg\n\
         [includeIf \"gitdir:/\"]\n\tpath = no-repo.cfg\n[includeIf \"onbranch:*\"]\n\tpath = no-branch.cfg\n",
    );
    let cx = Context { git_dir: None, branch: None, home: Some(home.clone()) };
    let found = included(std::slice::from_ref(&root), &cx);
    let want = [crate::paths::resolve(&h), crate::paths::resolve(&dir).join("missing.cfg"), crate::paths::resolve(&dir).join("has.cfg")];
    assert_eq!(found.files, want, "a missing file is listed: its creation is a change");
    assert!(found.on_branch);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cycles_end_and_depth_and_count_are_bounded() {
    let dir = folder("incl-bounds");
    let a = mark(&dir, "a.cfg", "[include]\n\tpath = a.cfg\n\tpath = b.cfg\n");
    mark(&dir, "b.cfg", "[include]\n\tpath = a.cfg\n\tpath = ./b.cfg\n\tpath = c.cfg\n");
    mark(&dir, "c.cfg", "[include]\n\tpath = b.cfg\n");
    let found = included(std::slice::from_ref(&a), &Context::default());
    assert_eq!(names(&found), ["b.cfg", "c.cfg"], "each file once, the root never");

    for i in 0..20 {
        mark(&dir, &format!("chain{i:02}.cfg"), &format!("[include]\n\tpath = chain{:02}.cfg\n", i + 1));
    }
    let found = included(&[dir.join("chain00.cfg")], &Context::default());
    assert_eq!(found.files.len(), MAX_DEPTH, "{:?}", names(&found));
    assert_eq!(names(&found).last().map(String::as_str), Some("chain10.cfg"));

    let many: String = (0..100).map(|i| format!("\tpath = many{i}.cfg\n")).collect();
    mark(&dir, "wide.cfg", &format!("[include]\n{many}"));
    assert_eq!(included(&[dir.join("wide.cfg")], &Context::default()).files.len(), MAX_FILES);
    let _ = std::fs::remove_dir_all(&dir);
}
