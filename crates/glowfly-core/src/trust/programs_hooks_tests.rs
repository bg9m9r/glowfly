//! Scripts a hook runs from the working tree: followed with the version HEAD tracks, so another
//! version asks again and a pull or checkout that brings one in is held; the words of ordinary
//! hooks (husky, Git LFS, `npx`, `git`, refs and messages) follow nothing new, a place outside
//! the working tree or at a network place is never read, and past the limits the whole working
//! tree is followed.

use super::super::super::compare::{Refused, check};
use super::super::super::tests::hook;
use super::super::super::{TrustScan, scan_now};
use super::*;
use crate::settings::SavedTrust;
use crate::testutil::*;
use git2::Repository;

fn scan_of(dir: &Path) -> TrustScan {
    scan_now(&Repository::open(dir).unwrap())
}

fn paths(s: &TrustScan) -> Vec<&str> {
    let mut out: Vec<&str> = s.follows.iter().map(|f| f.path.as_str()).collect();
    out.sort();
    out
}

/// Writes the hook `name` in `dir` with `text`, runnable.
fn hook_with(dir: &Path, name: &str, text: &str) {
    hook(dir, name);
    std::fs::write(dir.join(name), text).unwrap();
}

/// A repository whose tracked `scripts/check.sh` and `tools/lint.js` its pre-commit hook runs.
fn hook_running_scripts(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    std::fs::create_dir_all(dir.join("scripts")).unwrap();
    std::fs::create_dir_all(dir.join("tools")).unwrap();
    write(&dir, "scripts/check.sh", "#!/bin/sh\nexit 0\n");
    write(&dir, "tools/lint.js", "process.exit(0)\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "scripts"]);
    hook_with(&dir.join(".git/hooks"), "pre-commit", "#!/bin/sh\nset -e\nnode tools/lint.js \"$@\"\nexec ./scripts/check.sh\n");
    dir
}

#[test]
fn the_scripts_a_hook_runs_are_followed_and_in_the_print() {
    let dir = hook_running_scripts("hook-calls-print");
    let s = scan_of(&dir);
    assert_eq!(paths(&s), ["scripts/check.sh", "tools/lint.js"]);
    let repo = Repository::open(&dir).unwrap();
    let id = repo.head().unwrap().peel_to_tree().unwrap().get_path(Path::new("scripts/check.sh")).unwrap().id();
    assert!(s.follows.iter().any(|f| f.tracked == Some(("scripts/check.sh".into(), id))), "{:?}", s.follows);
    // Another committed version of the script is another print: it asks again.
    let saved = SavedTrust::trusted(Some(s.print()));
    write(&dir, "scripts/check.sh", "#!/bin/sh\necho two\n");
    git(&dir, &["-c", "core.hooksPath=", "commit", "-q", "-am", "edit"]);
    let after = scan_of(&dir);
    assert!(!super::super::super::allows(Some(&saved), &after));
    assert_eq!(after.gained_since(saved.print.as_ref().unwrap()).follows.len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_commit_that_changes_a_script_a_hook_runs_is_held() {
    let dir = hook_running_scripts("hook-calls-held");
    let s = scan_of(&dir);
    let saved = SavedTrust::trusted(Some(s.print()));
    let repo = Repository::open(&dir).unwrap();
    let head = repo.head().unwrap().peel_to_commit().unwrap();
    let blob = repo.blob(b"#!/bin/sh\necho changed\n").unwrap();
    let mut scripts = repo
        .treebuilder(Some(&head.tree().unwrap().get_name("scripts").unwrap().to_object(&repo).unwrap().peel_to_tree().unwrap()))
        .unwrap();
    scripts.insert("check.sh", blob, 0o100755).unwrap();
    let mut top = repo.treebuilder(Some(&head.tree().unwrap())).unwrap();
    top.insert("scripts", scripts.write().unwrap(), 0o040000).unwrap();
    let tree = repo.find_tree(top.write().unwrap()).unwrap();
    let sig = git2::Signature::now("t", "t@example.com").unwrap();
    let side = repo.commit(None, &sig, &sig, "side", &tree, &[&head]).unwrap();
    assert_eq!(check(&repo, &s.follows, Some(&saved), &[head.id()]), Ok(()), "the trusted version");
    let Err(Refused::Held(inc)) = check(&repo, &s.follows, Some(&saved), &[side]) else { panic!("held") };
    assert!(paths(&inc).contains(&"scripts/check.sh"), "{:?}", inc.follows);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_words_of_ordinary_hooks_follow_nothing_new() {
    let dir = scratch_repo("hook-calls-ordinary");
    // Things a bare word or a ref could be taken for.
    for d in ["test", "node_modules"] {
        std::fs::create_dir_all(dir.join(d)).unwrap();
    }
    write(&dir, "package.json", "{}\n");
    write(&dir, ".pre-commit-config.yaml", "repos: []\n");
    let hooks = dir.join(".git/hooks");
    hook_with(&hooks, "pre-commit", "#!/usr/bin/env sh\n# see docs/hooks.md\nnpx lint-staged\nnpm test\nprettier --check package.json\n");
    hook_with(&hooks, "pre-push", "#!/bin/sh\ngit diff --quiet origin/main refs/heads/main || echo \"Run npm/yarn first, and/or push\"\n");
    hook_with(
        &hooks,
        "post-checkout",
        "#!/bin/sh\ncommand -v git-lfs >/dev/null 2>&1 || { echo >&2 \"no lfs\"; exit 2; }\ngit lfs post-checkout \"$@\"\n",
    );
    hook_with(
        &hooks,
        "commit-msg",
        "#!/usr/bin/env bash\nARGS=(hook-impl --config=.pre-commit-config.yaml --hook-type=commit-msg)\nHERE=\"$(cd \"$(dirname \"$0\")\" && pwd)\"\nexec pre-commit \"${ARGS[@]}\"\n",
    );
    assert!(scan_of(&dir).follows.is_empty(), "{:?}", scan_of(&dir).follows);
    // husky: its hooks run `.husky/_/h` from their own folder, which the hooks folder covers.
    std::fs::create_dir_all(dir.join(".husky")).unwrap();
    hook_with(&dir.join(".husky/_"), "pre-commit", "#!/usr/bin/env sh\n. \"$(dirname \"$0\")/h\"\n");
    write(&dir, ".husky/_/h", "#!/usr/bin/env sh\n");
    hook_with(
        &dir.join(".husky"),
        "pre-push",
        "#!/usr/bin/env sh\n. \"$(dirname -- \"$0\")/_/husky.sh\"\nnpx --no -- commitlint --edit \"$1\"\n",
    );
    git(&dir, &["config", "core.hooksPath", ".husky/_"]);
    assert_eq!(paths(&scan_of(&dir)), [".husky/_"]);
    git(&dir, &["config", "core.hooksPath", ".husky"]);
    assert_eq!(paths(&scan_of(&dir)), [".husky"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_hook_naming_a_place_outside_the_working_tree_follows_nothing_there() {
    let dir = scratch_repo("hook-calls-outside");
    let far = git_spelling(&net_place().join("tools/x.sh"));
    let elsewhere = git_spelling(&dir.parent().unwrap().join("elsewhere.sh"));
    let text = format!("#!/bin/sh\nsh {far}\nsh {elsewhere}\nsh ../up.sh\nsh .git/hooks/other.sh\n");
    hook_with(&dir.join(".git/hooks"), "pre-commit", &text);
    let s = scan_of(&dir);
    assert!(s.follows.is_empty(), "{:?}", s.follows);
    assert_eq!(s.offsite, [crate::native_path::shown(Path::new(&far))], "noted, never looked at");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn past_the_limits_the_whole_working_tree_follows() {
    let dir = scratch_repo("hook-calls-limits");
    let many: String = (0..=MAX_FILES).map(|i| format!("sh ./s{i}.sh\n")).collect();
    hook_with(&dir.join(".git/hooks"), "pre-commit", &format!("#!/bin/sh\n{many}"));
    assert!(scan_of(&dir).follows.iter().any(|f| f.path.is_empty()), "too many files");
    let words = "x ".repeat(MAX_WORDS + 1);
    hook_with(&dir.join(".git/hooks"), "pre-commit", &format!("#!/bin/sh\necho {words}\nsh ./late.sh\n"));
    assert!(scan_of(&dir).follows.iter().any(|f| f.path.is_empty()), "too many words");
    let big = format!("#!/bin/sh\n{}\nsh ./late.sh\n", "#".repeat(MAX_TEXT_BYTES));
    hook_with(&dir.join(".git/hooks"), "pre-commit", &big);
    assert!(scan_of(&dir).follows.iter().any(|f| f.path.is_empty()), "too large to read");
    // A compiled hook's words can't be read: a NUL byte follows the whole working tree.
    hook_with(&dir.join(".git/hooks"), "pre-commit", "\x7fELF\0\0./bin/x");
    assert_eq!(paths(&scan_of(&dir)), [""], "a NUL byte");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_hook_names_its_own_folder_whatever_the_folder_is_called() {
    let dir = scratch_repo("hook-calls-own-folder");
    std::fs::create_dir_all(dir.join("scripts")).unwrap();
    write(&dir, "scripts/check.sh", "#!/bin/sh\nexit 0\n");
    for folder in ["my hooks", "hooks [v2]; x"] {
        let text = "#!/bin/sh\n. \"$(dirname -- \"$0\")/../scripts/check.sh\"\n";
        hook_with(&dir.join(folder), "pre-commit", text);
        git(&dir, &["config", "core.hooksPath", folder]);
        assert_eq!(paths(&scan_of(&dir)), [folder, "scripts/check.sh"], "{folder}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
