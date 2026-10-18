//! Trellis as git's sequence editor (`trellis --write-todo`): an interactive rebase run with the
//! real executable installs the prepared todo, and a todo that doesn't match git's is refused
//! without changing the repository; the same for `git rebase --edit-todo` on a running rebase.

use git2::{Oid, Repository, RepositoryState};
use std::path::{Path, PathBuf};
use trellis_core::gitcli::{args, run};
use trellis_core::rebase::{self, EditTodoRequest, Outcome, RebaseAction, RebaseBase, Resume, SequenceEditor, StartRequest, TodoStep};

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
    let dir = trellis_core::paths::temp_dir().join(format!("trellis-seq-editor-{name}-{}", std::process::id()));
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
    let mut items: Vec<TodoStep> = plan.items.into_iter().map(TodoStep::Commit).collect();
    items.reverse();
    let req = StartRequest { head: plan.head, upstream: plan.upstream, items, autostash: false };
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
    let items = tip.items.into_iter().map(TodoStep::Commit).collect();
    let req = StartRequest { head: tip.head, upstream: tip.upstream, items, autostash: false };
    let e = rebase::start(&dir, &req, &SequenceEditor::Shell(script)).unwrap_err();
    assert_eq!(e.to_string(), rebase::CHANGED);
    assert!(prepared_dir.join("todo.error").exists(), "the helper wrote why it refused");
    let repo = Repository::open(&dir).unwrap();
    assert_eq!(repo.state(), RepositoryState::Clean);
    assert_eq!(head(&dir), before);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A rebase of all three commits, stopped to edit the first.
fn stopped_at_first(dir: &Path) -> rebase::Remaining {
    let first = git(dir, &["rev-list", "--max-parents=0", "HEAD"]);
    let plan = rebase::plan(dir, &RebaseBase::FromCommit(Oid::from_str(first.trim()).unwrap())).unwrap();
    let mut items: Vec<TodoStep> = plan.items.into_iter().map(TodoStep::Commit).collect();
    items[0].commit_mut().unwrap().action = RebaseAction::Edit;
    let req = StartRequest { head: plan.head, upstream: plan.upstream, items, autostash: false };
    let outcome = rebase::start(dir, &req, &helper()).unwrap();
    assert!(matches!(outcome, Outcome::Stopped { .. }), "{outcome:?}");
    rebase::remaining(dir).unwrap()
}

fn request(r: &rebase::Remaining, steps: Vec<TodoStep>) -> EditTodoRequest {
    EditTodoRequest { steps, locked_text: r.locked_text.clone(), todo: r.todo.clone(), first_may_combine: r.first_may_combine }
}

fn todo_text(dir: &Path) -> String {
    let git_dir = Repository::open(dir).unwrap().path().to_path_buf();
    std::fs::read_to_string(git_dir.join("rebase-merge").join("git-rebase-todo")).unwrap()
}

#[test]
fn the_helper_edits_the_remaining_steps() {
    let dir = repo("edit-todo");
    let r = stopped_at_first(&dir);
    let mut steps = r.steps.clone();
    steps.reverse();
    rebase::edit_todo(&dir, &request(&r, steps), &helper()).unwrap();
    let after = rebase::remaining(&dir).unwrap();
    let order: Vec<String> = after.steps.iter().filter_map(TodoStep::commit).map(|i| i.summary.clone()).collect();
    assert_eq!(order, ["three", "two"]);
    let outcome = rebase::resume(&dir, Resume::Continue).unwrap();
    assert!(matches!(outcome, Outcome::Finished { .. }), "{outcome:?}");
    assert_eq!(git(&dir, &["log", "--format=%s"]).lines().collect::<Vec<_>>(), ["two", "three", "one"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_helper_refuses_a_foreign_remaining_step() {
    let dir = repo("edit-todo-foreign");
    let r = stopped_at_first(&dir);
    let before = todo_text(&dir);
    let prepared = Repository::open(&dir).unwrap().path().join("trellis-rebase").join("todo-edit");
    // Rewrites the prepared list to name the commit already done, then hands over to the helper.
    let done = git(&dir, &["rev-list", "--max-parents=0", "HEAD"]);
    let foreign = format!(
        "pick {} one
pick {} x
",
        done.trim(),
        done.trim()
    );
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trellis"));
    let script = format!(
        "printf '%s' {} > {} && {} --write-todo {}",
        rebase::shell_quote(&foreign),
        rebase::shell_quote(&prepared.to_string_lossy()),
        rebase::shell_quote(&exe.to_string_lossy()),
        rebase::shell_quote(&prepared.to_string_lossy()),
    );
    let e = rebase::edit_todo(&dir, &request(&r, r.steps.clone()), &SequenceEditor::Shell(script)).unwrap_err();
    assert_eq!(e.to_string(), rebase::CHANGED);
    assert!(prepared.with_file_name("todo-edit.error").exists(), "the helper wrote why it refused");
    assert_eq!(todo_text(&dir), before, "git's todo is unchanged");
    assert_eq!(rebase::remaining(&dir).unwrap().steps, r.steps);
    assert_eq!(Repository::open(&dir).unwrap().state(), RepositoryState::RebaseInteractive);
    git(&dir, &["rebase", "--abort"]);
    let _ = std::fs::remove_dir_all(&dir);
}
