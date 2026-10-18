//! The steps an interactive rebase in progress has left, read from git's own todo file (so also
//! for a rebase started outside Trellis), and replacing them with `git rebase --edit-todo`. The
//! exec lines Trellis adds for a reword or a combined message are folded back into the row they
//! belong to; the lines finishing a reword whose pick is already done are kept as they are.

use super::todo::{MSG_DIR, MessageFile, OwnLine, build, editor_command, listed_oids, own_line, problem_with};
use super::{CHANGED, NOT_INTERACTIVE, RebaseAction, SequenceEditor, TODO_MOVED, TodoItem, TodoStep, UNSUPPORTED_STEPS, short};
use crate::error::{Error, Result};
use crate::gitcli;
use git2::{Oid, Repository, RepositoryState};
use std::collections::HashSet;
use std::path::Path;

/// What is left of an interactive rebase in progress.
#[derive(Clone, Debug)]
pub struct Remaining {
    /// The branch being rebased, without refs/heads/; `None` for a detached HEAD.
    pub branch: Option<String>,
    /// "1a2b3c4 Summary" of the commit it rebases onto.
    pub onto_label: String,
    /// Commits already done.
    pub done: usize,
    /// The steps finishing a reword whose pick is done: shown, never changed.
    pub locked: Vec<TodoStep>,
    /// The todo lines of `locked`, written back first as they are.
    pub locked_text: String,
    /// The steps that can be edited, in order.
    pub steps: Vec<TodoStep>,
    /// The done list has a commit, so the first step may squash or fix up into it.
    pub first_may_combine: bool,
    /// git's todo file as it was read, to tell whether the rebase moved on since.
    pub todo: String,
}

/// The edited steps to write over a running rebase's todo.
#[derive(Clone, Debug)]
pub struct EditTodoRequest {
    pub steps: Vec<TodoStep>,
    pub locked_text: String,
    /// git's todo as it was read (`Remaining::todo`).
    pub todo: String,
    pub first_may_combine: bool,
}

/// One line of git's todo.
enum Parsed {
    Step(TodoStep),
    Own(OwnLine),
}

fn interactive(workdir: &Path) -> Result<Repository> {
    let repo = Repository::open(workdir)?;
    if repo.state() != RepositoryState::RebaseInteractive {
        return Err(Error::Unsupported(NOT_INTERACTIVE.into()));
    }
    Ok(repo)
}

/// The remaining steps of the interactive rebase in progress. Refused (`Unsupported`) when none
/// is in progress, or when the todo holds commands the list can't show (label, reset, merge,
/// update-ref, `fixup -C`); a commit that can't be found is `NotFound`.
pub fn remaining(workdir: &Path) -> Result<Remaining> {
    let repo = interactive(workdir)?;
    let dir = repo.path().join("rebase-merge");
    let read = |name: &str| std::fs::read_to_string(dir.join(name)).unwrap_or_default();
    let todo = std::fs::read_to_string(dir.join("git-rebase-todo"))?;
    let done = read("done");
    let branch = read("head-name").trim().strip_prefix("refs/heads/").map(str::to_owned);
    let onto = read("onto");
    let onto_label = match repo.revparse_single(onto.trim()).and_then(|o| o.peel_to_commit()) {
        Ok(c) => format!("{} {}", short(c.id()), c.summary().unwrap_or_default()).trim_end().to_owned(),
        Err(_) => onto.trim().chars().take(7).collect(),
    };
    let done_commits = listed_oids(&done).len();

    let mut lines: Vec<(&str, Parsed)> = Vec::new();
    for raw in todo.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(p) = parse(&repo, line)? {
            lines.push((raw.trim_end(), p));
        }
    }

    // An amend whose base isn't listed: its pick is done, so the lines up to it finish that
    // commit and stay as they are.
    let mut bases = HashSet::new();
    let orphan = lines.iter().position(|(_, p)| match p {
        Parsed::Own(OwnLine::Base(o)) => {
            bases.insert(*o);
            false
        }
        Parsed::Own(OwnLine::Amend { oid, .. }) => !bases.contains(oid),
        Parsed::Step(_) => false,
    });
    let split = orphan.map_or(0, |k| k + 1);
    let rest = lines.split_off(split);
    let locked_text: String = lines.iter().map(|(raw, _)| format!("{raw}\n")).collect();
    let locked = lines.into_iter().filter_map(|(_, p)| if let Parsed::Step(s) = p { Some(s) } else { None }).collect();

    let msg_dir = repo.path().join(MSG_DIR);
    let steps = fold(rest, &msg_dir);
    Ok(Remaining { branch, onto_label, done: done_commits, locked, locked_text, steps, first_may_combine: done_commits > 0, todo })
}

/// One todo line; `None` for a `noop`.
fn parse(repo: &Repository, line: &str) -> Result<Option<Parsed>> {
    let (cmd, rest) = line.split_once(char::is_whitespace).map_or((line, ""), |(c, r)| (c, r.trim()));
    let action = match cmd {
        "pick" | "p" => RebaseAction::Pick,
        "reword" | "r" => RebaseAction::Reword,
        "edit" | "e" => RebaseAction::Edit,
        "squash" | "s" => RebaseAction::Squash,
        "fixup" | "f" => RebaseAction::Fixup,
        "drop" | "d" => RebaseAction::Drop,
        "exec" | "x" => return Ok(Some(own_line(line).map_or_else(|| Parsed::Step(TodoStep::Exec(rest.to_owned())), Parsed::Own))),
        "break" | "b" => return Ok(Some(Parsed::Step(TodoStep::Break))),
        "noop" => return Ok(None),
        _ => return Err(Error::Unsupported(UNSUPPORTED_STEPS.into())),
    };
    let sha = rest.split_whitespace().next().unwrap_or_default();
    if sha.is_empty() || sha.starts_with('-') {
        return Err(Error::Unsupported(UNSUPPORTED_STEPS.into()));
    }
    let c = repo.revparse_single(sha).and_then(|o| o.peel_to_commit()).map_err(|_| Error::NotFound(format!("commit {sha}")))?;
    Ok(Some(Parsed::Step(TodoStep::Commit(TodoItem {
        oid: c.id(),
        action,
        summary: c.summary().unwrap_or_default().to_owned(),
        message: c.message().unwrap_or_default().to_owned(),
        new_message: None,
        combined_message: None,
        author: c.author().name().unwrap_or_default().to_owned(),
        time: c.time().seconds(),
    }))))
}

/// The editable steps: each base line with its later amend (and a readable message file) is
/// folded into the commit's reword or combined message; other own lines stay exec steps.
fn fold(lines: Vec<(&str, Parsed)>, msg_dir: &Path) -> Vec<TodoStep> {
    // (base index, amend index, message, combined)
    let mut pairs: Vec<(usize, usize, String, bool)> = Vec::new();
    for (b, (_, p)) in lines.iter().enumerate() {
        let Parsed::Own(OwnLine::Base(oid)) = p else { continue };
        let amend = lines.iter().enumerate().skip(b + 1).find_map(|(a, (_, p))| match p {
            Parsed::Own(OwnLine::Amend { oid: o, file }) if o == oid => Some((a, *file)),
            _ => None,
        });
        let Some((a, file)) = amend else { continue };
        // A reworded group's squashed message is rebuilt from its typed one, in the reword file.
        let file = if file == MessageFile::Squashed { MessageFile::Reword } else { file };
        if let Ok(text) = std::fs::read_to_string(msg_dir.join(file.name(*oid))) {
            pairs.push((b, a, text.trim_end().to_owned(), file == MessageFile::Combined));
        }
    }
    let mut steps: Vec<TodoStep> = Vec::new();
    for (n, (raw, p)) in lines.into_iter().enumerate() {
        if pairs.iter().any(|(b, ..)| *b == n) {
            continue;
        }
        if let Some((_, _, text, combined)) = pairs.iter().find(|(_, a, ..)| *a == n) {
            let oid = match p {
                Parsed::Own(OwnLine::Amend { oid, .. }) => oid,
                _ => continue,
            };
            if let Some(item) = steps.iter_mut().rev().filter_map(TodoStep::commit_mut).find(|i| i.oid == oid) {
                if *combined {
                    item.combined_message = Some(text.clone());
                } else {
                    item.action = RebaseAction::Reword;
                    item.new_message = Some(text.clone());
                }
            }
            continue;
        }
        steps.push(match p {
            Parsed::Step(s) => s,
            Parsed::Own(_) => {
                let line = raw.trim();
                TodoStep::Exec(line.split_once(char::is_whitespace).map_or("", |(_, r)| r.trim()).to_owned())
            }
        });
    }
    steps
}

/// Replaces the remaining steps of the rebase in progress with `req` through
/// `git rebase --edit-todo`, with `editor` installing the prepared list. The rebase stays
/// stopped where it was. Refused when no interactive rebase is in progress, when its todo
/// changed since it was read (`TODO_MOVED`), when the commits differ from the ones git has
/// left (`CHANGED`), when the list has a problem, or when git or the editor refuses it.
pub fn edit_todo(workdir: &Path, req: &EditTodoRequest, editor: &SequenceEditor) -> Result<()> {
    let repo = interactive(workdir)?;
    let git_dir = repo.path().to_path_buf();
    let todo_path = git_dir.join("rebase-merge").join("git-rebase-todo");
    let current = std::fs::read_to_string(&todo_path)?;
    if current != req.todo {
        return Err(Error::Unsupported(TODO_MOVED.into()));
    }
    let resolve = |shas: Vec<String>| -> Option<Vec<Oid>> {
        let mut oids: Vec<Oid> = shas.iter().map(|s| repo.revparse_single(s).map(|o| o.id()).ok()).collect::<Option<_>>()?;
        oids.sort();
        Some(oids)
    };
    let mut ours: Vec<String> = req.steps.iter().filter_map(TodoStep::oid).map(|o| o.to_string()).collect();
    ours.extend(listed_oids(&req.locked_text));
    let (ours, theirs) = (resolve(ours), resolve(listed_oids(&current)));
    if ours.is_none() || ours != theirs {
        return Err(Error::Unsupported(CHANGED.into()));
    }
    if let Some(p) = problem_with(&req.steps, req.first_may_combine) {
        return Err(Error::Unsupported(p));
    }
    // Not wiped: a locked amend still reads its files from here.
    let msg_dir = git_dir.join(MSG_DIR);
    std::fs::create_dir_all(&msg_dir)?;
    let (text, files) = build(&req.steps, &msg_dir, true);
    for (path, body) in files {
        crate::persist::atomic_write(&path, body.as_bytes())?;
    }
    let prepared = msg_dir.join("todo-edit");
    crate::persist::atomic_write(&prepared, format!("{}{text}", req.locked_text).as_bytes())?;
    let refused = msg_dir.join("todo-edit.error");
    if refused.exists() {
        std::fs::remove_file(&refused)?;
    }
    let command = editor_command(editor, &prepared);
    let (ok, output) = gitcli::run_with_input(workdir, &gitcli::args(&["rebase", "--edit-todo"]), "", &[("GIT_SEQUENCE_EDITOR", &command)]);
    let why = if refused.exists() {
        std::fs::read_to_string(&refused).unwrap_or_default().trim().to_owned()
    } else if !ok {
        output.lines().map(str::trim).rfind(|l| !l.is_empty()).unwrap_or("git rebase --edit-todo failed").to_owned()
    } else {
        return Ok(());
    };
    // git leaves the list it handed the editor (short ids, help comments) behind: put back the
    // text that was read, so the panel's list still matches it.
    crate::persist::atomic_write(&todo_path, req.todo.as_bytes())?;
    Err(Error::Unsupported(why))
}
