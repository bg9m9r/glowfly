//! Trellis as git's sequence editor (`trellis --write-todo`): an interactive rebase run with the
//! real executable installs the prepared todo, and a todo that doesn't match git's is refused
//! without changing the repository.

use git2::{Oid, Repository, RepositoryState};
use std::path::{Path, PathBuf};
use trellis_core::gitcli::{args, run};
use trellis_core::rebase::{self, Outcome, RebaseBase, SequenceEditor, StartRequest};

fn git(dir: &Path, a: &[&str]) -> String {
    let (ok, out) = run(dir, &args(a));
    assert!(ok, "git {a:?} failed: {out}");
    out
}

fn head(dir: &Path) -> Oid {
    Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

/// main with three commits, each adding a file.
fn repo(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("trellis-seq-editor-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q", "-b", "main"]);
    for (k, v) in [("user.name", "Test"), ("user.email", "test@example.com"), ("commit.gpgsign", "false"), ("core.autocrlf", "false")] {
        git(&dir, &["config", k, v]);
    }
    for n in ["one", "two", "three"] {
        std::fs::write(dir.join(format!("{n}.txt")), n).unwrap();
        git(&dir, &["add", "-A"]);
        git(&dir, &["commit", "-q", "-m", n]);
    }
    dir
}

fn helper() -> SequenceEditor {
    SequenceEditor::Helper(PathBuf::from(env!("CARGO_BIN_EXE_trellis")))
}

#[test]
fn the_helper_installs_the_prepared_todo() {
    let dir = repo("reverse");
    let base = git(&dir, &["rev-parse", "HEAD~1"]);
    let plan = rebase::plan(&dir, &RebaseBase::FromCommit(Oid::from_str(base.trim()).unwrap())).unwrap();
    let mut items = plan.items;
    items.reverse();
    let req = StartRequest { head: plan.head, upstream: plan.upstream, items };
    let outcome = rebase::start(&dir, &req, &helper()).unwrap();
    assert!(matches!(outcome, Outcome::Finished { .. }), "{outcome:?}");
    assert_eq!(git(&dir, &["log", "--format=%s"]).lines().collect::<Vec<_>>(), ["two", "three", "one"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_helper_refuses_a_foreign_commit() {
    let dir = repo("foreign");
    let before = head(&dir);
    // The list holds only the tip; `start` checks it and writes the prepared todo.
    let tip = rebase::plan(&dir, &RebaseBase::FromCommit(before)).unwrap();
    let prepared_dir = Repository::open(&dir).unwrap().path().join("trellis-rebase");
    let parent = Repository::open(&dir).unwrap().find_commit(before).unwrap().parent_id(0).unwrap();
    // A sequence editor that rewrites the prepared todo to name `parent` (outside the range),
    // then hands over to the helper, as if the list had changed in between.
    let foreign = format!("pick {parent} two\n");
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trellis"));
    let prepared = prepared_dir.join("todo");
    let script = format!(
        "printf '%s' {} > {} && {} --write-todo {}",
        rebase::shell_quote(&foreign),
        rebase::shell_quote(&prepared.to_string_lossy()),
        rebase::shell_quote(&exe.to_string_lossy()),
        rebase::shell_quote(&prepared.to_string_lossy()),
    );
    let req = StartRequest { head: tip.head, upstream: tip.upstream, items: tip.items };
    let e = rebase::start(&dir, &req, &SequenceEditor::Shell(script)).unwrap_err();
    assert_eq!(e.to_string(), rebase::CHANGED);
    assert!(prepared_dir.join("todo.error").exists(), "the helper wrote why it refused");
    let repo = Repository::open(&dir).unwrap();
    assert_eq!(repo.state(), RepositoryState::Clean);
    assert_eq!(head(&dir), before);
    let _ = std::fs::remove_dir_all(&dir);
}
