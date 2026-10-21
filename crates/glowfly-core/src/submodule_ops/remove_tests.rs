//! Adding (through `add_cmd`) and removing submodules against real repositories: a library
//! repository with a bare clone of it as the submodule's URL, so the file-protocol flag is
//! exercised too.

use super::*;
use crate::gitcli::{args, run};
use crate::submodule_ops::add_cmd;
use crate::testutil::*;
use std::path::{Path, PathBuf};

const PATH: &str = "libs/lib";

struct Fixture {
    parent: PathBuf,
    lib: PathBuf,
    bare: PathBuf,
}

impl Fixture {
    /// A library with one commit on main and one more on dev, its bare clone, and a parent
    /// repository with one commit.
    fn new(name: &str) -> Self {
        let lib = scratch_repo(&format!("{name}-lib"));
        write(&lib, "lib.txt", "lib\n");
        git(&lib, &["add", "."]);
        git(&lib, &["commit", "-q", "-m", "lib"]);
        git(&lib, &["checkout", "-q", "-b", "dev"]);
        write(&lib, "dev.txt", "dev\n");
        git(&lib, &["add", "."]);
        git(&lib, &["commit", "-q", "-m", "dev work"]);
        git(&lib, &["checkout", "-q", "main"]);
        let bare = lib.with_file_name(format!("{}-bare.git", lib.file_name().unwrap().to_string_lossy()));
        let _ = std::fs::remove_dir_all(&bare);
        let (ok, out) =
            run(lib.parent().unwrap(), &args(&["clone", "-q", "--bare", &lib.display().to_string(), &bare.display().to_string()]));
        assert!(ok, "{out}");
        let parent = scratch_repo(name);
        write(&parent, "a.txt", "one\n");
        git(&parent, &["add", "."]);
        git(&parent, &["commit", "-q", "-m", "first"]);
        Fixture { parent, lib, bare }
    }

    fn url(&self) -> String {
        self.bare.display().to_string()
    }

    /// Adds the submodule at `PATH` in `dir` the way the Add dialog does, then commits it.
    fn add_in(&self, dir: &Path, branch: Option<&str>) {
        let (ok, out) = run(dir, &add_cmd(&self.url(), PATH, branch));
        assert!(ok, "{out}");
        git(dir, &["commit", "-q", "-m", "add lib"]);
        let sub = dir.join("libs").join("lib");
        set_config(&sub, &[("user.name", "Test"), ("user.email", "test@example.com"), ("commit.gpgsign", "false")]);
    }

    fn add(&self) {
        self.add_in(&self.parent, None);
    }

    fn modules(&self) -> PathBuf {
        self.parent.join(".git").join("modules").join("libs").join("lib")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        for d in [&self.parent, &self.lib, &self.bare] {
            let _ = std::fs::remove_dir_all(d);
        }
    }
}

fn gitlink_in_index(dir: &Path) -> bool {
    let repo = git2::Repository::open(dir).unwrap();
    repo.index().unwrap().get_path(Path::new(PATH), 0).is_some()
}

fn status(dir: &Path) -> String {
    run(dir, &args(&["status", "--porcelain"])).1
}

#[test]
fn remove_deletes_everything_and_the_path_can_be_added_again() {
    let f = Fixture::new("sm-remove");
    f.add();
    assert!(f.modules().join("HEAD").is_file());
    let rep = remove(&f.parent, PATH, PATH);
    assert_eq!(rep.outcome, RemoveOutcome::Removed, "{}", rep.output);
    assert!(rep.output.contains("$ git submodule deinit -f -- :(literal)libs/lib"), "{}", rep.output);
    assert!(rep.output.contains("Deleted the stored module"), "{}", rep.output);
    assert!(!gitlink_in_index(&f.parent));
    let gitmodules = std::fs::read_to_string(f.parent.join(".gitmodules")).unwrap_or_default();
    assert!(!gitmodules.contains("libs/lib"), "{gitmodules}");
    let st = status(&f.parent);
    assert!(st.contains("M  .gitmodules") || st.contains("D  .gitmodules"), "{st}");
    assert!(st.contains("D  libs/lib"), "{st}");
    assert!(!f.parent.join("libs").join("lib").exists());
    assert!(!f.modules().exists());
    assert!(!f.parent.join(".git").join("modules").join("libs").exists(), "emptied parents go too");
    let config = git2::Config::open(&f.parent.join(".git").join("config")).unwrap();
    assert!(config.get_string("submodule.libs/lib.url").is_err());

    let (ok, out) = run(&f.parent, &add_cmd(&f.url(), PATH, None));
    assert!(ok, "adding again works: {out}");
}

#[test]
fn add_with_a_branch_records_it() {
    let f = Fixture::new("sm-add-branch");
    f.add_in(&f.parent, Some("dev"));
    let gitmodules = std::fs::read_to_string(f.parent.join(".gitmodules")).unwrap();
    assert!(gitmodules.contains("branch = dev"), "{gitmodules}");
    let lib = git2::Repository::open(&f.lib).unwrap();
    let dev = lib.find_branch("dev", git2::BranchType::Local).unwrap().get().target().unwrap();
    let sub = git2::Repository::open(f.parent.join("libs").join("lib")).unwrap();
    assert_eq!(sub.head().unwrap().target(), Some(dev));
}

#[test]
fn a_linked_worktree_cleans_its_own_store() {
    let f = Fixture::new("sm-remove-wt");
    let wt = f.parent.with_file_name(format!("{}-wt", f.parent.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&wt);
    git(&f.parent, &["worktree", "add", "-q", "-b", "wt", &wt.display().to_string()]);
    f.add_in(&wt, None);
    let git_dir = crate::native_path::native(git2::Repository::open(&wt).unwrap().path());
    let store = git_dir.join("modules").join("libs").join("lib");
    assert!(store.join("HEAD").is_file(), "git keeps the module in the worktree's own git dir");
    assert!(!f.modules().exists(), "not in the common dir");
    let rep = remove(&wt, PATH, PATH);
    assert_eq!(rep.outcome, RemoveOutcome::Removed, "{}", rep.output);
    assert!(!store.exists());
    git(&f.parent, &["worktree", "remove", "--force", &wt.display().to_string()]);
    let _ = std::fs::remove_dir_all(&wt);
}

#[test]
fn an_uninitialized_submodule_loses_its_store_too() {
    let f = Fixture::new("sm-remove-uninit");
    f.add();
    git(&f.parent, &["submodule", "deinit", "-q", "-f", PATH]);
    assert!(f.modules().exists());
    let facts = removal_facts(&f.parent, PATH).unwrap().unwrap();
    assert!(!facts.info.is_initialized());
    assert!(facts.module_dir.is_some());
    let rep = remove(&f.parent, PATH, PATH);
    assert_eq!(rep.outcome, RemoveOutcome::Removed, "{}", rep.output);
    assert!(!f.modules().exists() && !gitlink_in_index(&f.parent));
}

#[test]
fn unstaged_gitmodules_fails_at_git_rm() {
    let f = Fixture::new("sm-remove-rm-fails");
    f.add();
    let gm = f.parent.join(".gitmodules");
    let text = std::fs::read_to_string(&gm).unwrap();
    std::fs::write(&gm, format!("{text}# edited\n")).unwrap();
    let rep = remove(&f.parent, PATH, PATH);
    assert_eq!(rep.outcome, RemoveOutcome::RmFailed, "{}", rep.output);
    assert!(rep.output.contains("$ git --literal-pathspecs rm -f -- libs/lib"), "{}", rep.output);
    assert!(gitlink_in_index(&f.parent));
    assert!(f.modules().exists(), "nothing past the failure runs");
}

#[test]
fn an_unknown_path_fails_at_deinit_and_changes_nothing() {
    let f = Fixture::new("sm-remove-unknown");
    f.add();
    let before = status(&f.parent);
    let rep = remove(&f.parent, "not/a/submodule", "not/a/submodule");
    assert_eq!(rep.outcome, RemoveOutcome::DeinitFailed, "{}", rep.output);
    assert!(!rep.output.trim().is_empty());
    assert_eq!(status(&f.parent), before);
    assert!(gitlink_in_index(&f.parent));
}

/// Makes `link` a link to the folder `target`; false where neither a symbolic link nor (on
/// Windows) a junction can be made.
fn dir_link(target: &Path, link: &Path) -> bool {
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink(target, link).is_ok();
    #[cfg(windows)]
    let made = std::os::windows::fs::symlink_dir(target, link).is_ok()
        || std::process::Command::new("cmd").args(["/c", "mklink", "/J"]).arg(link).arg(target).output().is_ok_and(|o| o.status.success());
    made
}

#[test]
fn a_linked_store_is_left_alone() {
    let f = Fixture::new("sm-remove-linked-store");
    f.add();
    let outside = f.parent.with_file_name(format!("{}-store", f.parent.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&outside);
    std::fs::rename(f.modules(), &outside).unwrap();
    if !dir_link(&outside, &f.modules()) {
        eprintln!("skipped: this account can't make folder links");
        let _ = std::fs::remove_dir_all(&outside);
        return;
    }
    let rep = remove(&f.parent, PATH, PATH);
    match &rep.outcome {
        RemoveOutcome::ModuleLeft { dir, error } => {
            assert!(dir.ends_with(Path::new("modules").join("libs").join("lib")), "{}", dir.display());
            assert!(error.contains("link"), "{error}");
        }
        other => panic!("{other:?}: {}", rep.output),
    }
    assert!(!gitlink_in_index(&f.parent), "the git part ran");
    assert!(outside.join("HEAD").is_file(), "the folder the link leads to survives");
    let _ = std::fs::remove_dir(f.modules()).or_else(|_| std::fs::remove_file(f.modules()));
    let _ = std::fs::remove_dir_all(&outside);
}

#[test]
fn removal_facts_name_changes_commits_and_stashes() {
    let f = Fixture::new("sm-facts");
    f.add();
    assert_eq!(removal_facts(&f.parent, "not/here").unwrap(), None);
    let sub = f.parent.join("libs").join("lib");
    let facts = removal_facts(&f.parent, PATH).unwrap().unwrap();
    assert!(!facts.info.is_dirty() && facts.unpushed.is_empty() && facts.stashes == 0, "{facts:?}");
    assert!(facts.module_dir.as_ref().is_some_and(|d| d.ends_with(Path::new("modules").join("libs").join("lib"))));

    write(&sub, "new.txt", "untracked\n");
    assert!(removal_facts(&f.parent, PATH).unwrap().unwrap().info.is_dirty());
    std::fs::remove_file(sub.join("new.txt")).unwrap();

    write(&sub, "lib.txt", "changed\n");
    git(&sub, &["commit", "-q", "-am", "inside"]);
    let id = git2::Repository::open(&sub).unwrap().head().unwrap().target().unwrap();
    let facts = removal_facts(&f.parent, PATH).unwrap().unwrap();
    assert_eq!(facts.unpushed, vec![(id, "inside".to_owned())]);
    git(&sub, &["push", "-q", "origin", "HEAD:main"]);
    assert!(removal_facts(&f.parent, PATH).unwrap().unwrap().unpushed.is_empty());

    write(&sub, "lib.txt", "stash me\n");
    git(&sub, &["stash", "-q"]);
    assert_eq!(removal_facts(&f.parent, PATH).unwrap().unwrap().stashes, 1);
}
