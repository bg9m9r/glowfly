//! The todo list: checking a list before it runs, writing it in git's format (with the reword
//! and combined message files), recognising the lines Trellis adds, and installing it over the
//! list git generated.

use super::{
    CHANGED, COMBINE_AFTER_MERGE, COMBINE_AFTER_STEP, EMPTY_COMBINED, EMPTY_EXEC, EMPTY_REWORD, EXEC_ONE_LINE, FIRST_COMBINES,
    LOST_TO_RESET, RebaseAction, SequenceEditor, TodoItem, TodoStep, merges,
};
use crate::error::{Error, Result};
use git2::Oid;
use std::path::{Path, PathBuf};

/// The folder in the git directory that holds the message files and the prepared todo.
pub(crate) const MSG_DIR: &str = "trellis-rebase";

/// A kept commit row that can head a group: neither dropped nor melded into the one before.
fn heads(step: &TodoStep) -> Option<&TodoItem> {
    step.commit().filter(|i| i.action != RebaseAction::Drop && !i.action.combines())
}

/// For the kept commit at `head`, the index of the last squash or fixup of its group: the
/// commit rows right after it that are dropped or meld into it, up to any other step (exec,
/// break, label, reset or merge). `None` when nothing melds into it.
pub fn group_end(steps: &[TodoStep], head: usize) -> Option<usize> {
    heads(steps.get(head)?)?;
    let mut end = None;
    for (i, step) in steps.iter().enumerate().skip(head + 1) {
        match step.commit() {
            Some(c) if c.action.combines() => end = Some(i),
            Some(c) if c.action == RebaseAction::Drop => {}
            _ => break,
        }
    }
    end
}

/// The message git gives the commit the group at `head` combines into: the head's (its typed
/// one when it is reworded), then each squash's, joined with blank lines. Fixups and dropped
/// rows add nothing.
pub fn default_combined(steps: &[TodoStep], head: usize) -> String {
    let Some(item) = steps.get(head).and_then(TodoStep::commit) else { return String::new() };
    let first = if item.rewords() { item.new_message.as_deref().unwrap_or_default() } else { item.message.as_str() };
    let mut parts = vec![first.trim().to_owned()];
    if let Some(end) = group_end(steps, head) {
        let squashes = steps[head + 1..=end].iter().filter_map(TodoStep::commit).filter(|c| c.action == RebaseAction::Squash);
        parts.extend(squashes.map(|c| c.message.trim().to_owned()));
    }
    parts.join("\n\n")
}

/// Why the list can't run as it is; see [`problem_with`].
pub fn problem(steps: &[TodoStep]) -> Option<String> {
    problem_with(steps, false)
}

/// Why the list can't run as it is: a squash or fixup with no kept commit before it (with
/// `first_may_combine`, the first may meld into a commit already rebased) or right after an
/// exec or break step (or a label, reset or merge), a commit a later reset would throw away, an
/// empty typed reword or combined message, or an exec command that is empty or longer than one
/// line.
pub fn problem_with(steps: &[TodoStep], first_may_combine: bool) -> Option<String> {
    let dropped = |s: &TodoStep| s.item().is_some_and(|i| i.action == RebaseAction::Drop);
    let mut kept = first_may_combine;
    for step in steps {
        match step {
            TodoStep::Commit(i) if i.action.combines() && !kept => return Some(FIRST_COMBINES.into()),
            TodoStep::Commit(_) | TodoStep::Merge(_) if !dropped(step) => kept = true,
            _ => {}
        }
    }
    let mut before: Option<&TodoStep> = None;
    for step in steps {
        let combines = step.commit().is_some_and(|i| i.action.combines());
        match before {
            Some(TodoStep::Exec(_) | TodoStep::Break) if combines => return Some(COMBINE_AFTER_STEP.into()),
            Some(b) if combines && b.is_structure() => return Some(COMBINE_AFTER_MERGE.into()),
            _ => {}
        }
        if !dropped(step) {
            before = Some(step);
        }
    }
    if merges::loses_commit(steps) {
        return Some(LOST_TO_RESET.into());
    }
    let commits = || steps.iter().filter_map(TodoStep::item);
    if commits().any(|i| i.action == RebaseAction::Reword && i.new_message.as_ref().is_some_and(|m| m.trim().is_empty())) {
        return Some(EMPTY_REWORD.into());
    }
    let empty_combined = |(n, s): (usize, &TodoStep)| {
        s.commit().and_then(|i| i.combined_message.as_ref()).is_some_and(|m| m.trim().is_empty()) && group_end(steps, n).is_some()
    };
    if steps.iter().enumerate().any(empty_combined) {
        return Some(EMPTY_COMBINED.into());
    }
    let execs = || steps.iter().filter_map(|s| if let TodoStep::Exec(c) = s { Some(c) } else { None });
    if execs().any(|c| c.trim().is_empty()) {
        return Some(EMPTY_EXEC.into());
    }
    execs().any(|c| c.contains(['\n', '\r'])).then(|| EXEC_ONE_LINE.into())
}

/// Whether running `steps` would replay `original` as it is: the same steps in the same order,
/// nothing but picks (or rewords that keep their message), no exec or break step and no typed
/// combined message.
pub fn is_unchanged(steps: &[TodoStep], original: &[TodoStep]) -> bool {
    let picked = |i: &TodoItem| i.action == RebaseAction::Pick || (i.action == RebaseAction::Reword && !i.rewords());
    steps.len() == original.len()
        && steps.iter().zip(original).all(|pair| match pair {
            (TodoStep::Commit(a), TodoStep::Commit(b)) => a.oid == b.oid && picked(a) && a.combined_message.is_none(),
            (TodoStep::Merge(a), TodoStep::Merge(b)) => a.item.oid == b.item.oid && picked(&a.item),
            (a @ (TodoStep::Label { .. } | TodoStep::Reset(_)), b) => a == b,
            _ => false,
        })
}

/// The todo text (LF line endings) and the message files to write first. A reword becomes a
/// pick plus an `exec` that amends the message from a file in `msg_dir`; the exec follows the
/// commit's whole group (the squash and fixup rows right after it). The combined commit gets
/// [`default_combined`]: the typed message, then each squash's (a file of its own, with the
/// typed message alone kept in the reword file for reading the list back). A group head's
/// typed combined message is amended the same way from its own file, and wins over a reword. Another `exec` before the pick records
/// HEAD, and the amend runs only when HEAD is then a new commit on top of it: a pick that was
/// skipped (after a conflict, or because it became empty) leaves nothing to reword, and the
/// amend must not rewrite the commit before it. Exec and break steps are written as they are.
/// With `keep_rewords`, a reword without a typed message stays a `reword` (as in a running
/// rebase's own list); otherwise it is a plain pick. A list with label, reset or merge steps
/// starts with `label onto`, which names the new base (see `merges`).
pub fn build(steps: &[TodoStep], msg_dir: &Path, keep_rewords: bool) -> (String, Vec<(PathBuf, String)>) {
    let mut text = String::new();
    let mut files = Vec::new();
    let mut pending: Option<String> = None;
    if merges::has_structure(steps) {
        text.push_str("label onto\n");
    }
    for (i, step) in steps.iter().enumerate() {
        if merges::write(step, msg_dir, &mut text, &mut files) {
            continue;
        }
        let item = match step {
            TodoStep::Commit(item) => item,
            TodoStep::Exec(cmd) => {
                text.push_str(&format!("exec {cmd}\n"));
                continue;
            }
            TodoStep::Break => {
                text.push_str("break\n");
                continue;
            }
            _ => continue,
        };
        let keyword = match item.action {
            RebaseAction::Reword if keep_rewords && item.new_message.is_none() => "reword",
            RebaseAction::Reword => "pick",
            a => a.keyword(),
        };
        let summary = item.summary.replace(['\r', '\n'], " ");
        let end = group_end(steps, i);
        let squashes = end.is_some_and(|e| steps[i + 1..=e].iter().filter_map(TodoStep::commit).any(|c| c.action == RebaseAction::Squash));
        let typed = item.new_message.as_deref().unwrap_or_default();
        let file = |kind: MessageFile, message: &str| (msg_dir.join(kind.name(item.oid)), message.trim_end().to_owned() + "\n");
        let amend = match item.combined_message.as_deref().filter(|_| end.is_some()) {
            Some(m) => Some(file(MessageFile::Combined, m)),
            None if item.rewords() && squashes => {
                files.push(file(MessageFile::Reword, typed));
                Some(file(MessageFile::Squashed, &default_combined(steps, i)))
            }
            None if item.rewords() => Some(file(MessageFile::Reword, typed)),
            None => None,
        };
        if let Some((path, message)) = amend {
            let base = record_base(msg_dir, item.oid, &mut text);
            pending = Some(guarded_amend(&base, &shell_quote(&path.to_string_lossy())));
            files.push((path, message));
        }
        text.push_str(&format!("{keyword} {} {}\n", item.oid, summary.trim_end()));
        // The group ends where the next row neither combines nor is dropped, or at an exec or
        // break step.
        let continues = steps[i + 1..]
            .iter()
            .map_while(TodoStep::commit)
            .take_while(|n| n.action == RebaseAction::Drop || n.action.combines())
            .any(|n| n.action.combines());
        if !continues && let Some(exec) = pending.take() {
            text.push_str(&exec);
        }
    }
    (text, files)
}

/// The message file an amend line reads, in the message folder.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum MessageFile {
    /// `reword-<oid>.txt`: the commit's typed message.
    Reword,
    /// `squashed-<oid>.txt`: a reworded group head's typed message followed by its squashes'.
    Squashed,
    /// `combined-<oid>.txt`: a group's typed combined message.
    Combined,
}

impl MessageFile {
    const ALL: [MessageFile; 3] = [MessageFile::Reword, MessageFile::Squashed, MessageFile::Combined];

    fn prefix(self) -> &'static str {
        match self {
            MessageFile::Reword => "reword-",
            MessageFile::Squashed => "squashed-",
            MessageFile::Combined => "combined-",
        }
    }

    /// The file's name for the commit `oid`.
    pub(crate) fn name(self, oid: Oid) -> String {
        format!("{}{oid}.txt", self.prefix())
    }
}

/// One of the `exec` lines [`build`] adds around a reworded commit or group.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum OwnLine {
    /// Records HEAD before the commit's pick.
    Base(Oid),
    /// Amends the message after its group from `file`.
    Amend { oid: Oid, file: MessageFile },
}

/// Recognises Trellis's own exec lines (with or without the `exec`/`x` keyword) by the message
/// folder's file names in them; `shell_quote` writes their paths with forward slashes on every
/// OS.
pub(crate) fn own_line(line: &str) -> Option<OwnLine> {
    let line = line.trim();
    let cmd = line.strip_prefix("exec ").or_else(|| line.strip_prefix("x ")).unwrap_or(line).trim_start();
    let named = |prefix: &str, suffix: &str| -> Option<Oid> {
        let marker = format!("{MSG_DIR}/{prefix}");
        cmd.match_indices(&marker).find_map(|(at, m)| {
            let rest = &cmd[at + m.len()..];
            let hex = rest.get(..40).filter(|h| h.bytes().all(|b| b.is_ascii_hexdigit()))?;
            if rest[40..].starts_with(suffix) { Oid::from_str(hex).ok() } else { None }
        })
    };
    if cmd.starts_with("h=$(git rev-parse") {
        return MessageFile::ALL.into_iter().find_map(|file| named(file.prefix(), ".txt").map(|oid| OwnLine::Amend { oid, file }));
    }
    named("reword-", ".base").filter(|_| cmd.starts_with("git rev-parse HEAD >")).map(OwnLine::Base)
}

/// Writes the `exec` line that records HEAD before the commit `oid` is replayed; returns the
/// record's path, shell-quoted.
pub(super) fn record_base(msg_dir: &Path, oid: Oid, text: &mut String) -> String {
    let base = shell_quote(&msg_dir.join(format!("reword-{oid}.base")).to_string_lossy());
    text.push_str(&format!("exec git rev-parse HEAD >{base}\n"));
    base
}

/// The `exec` line that amends HEAD's message from `file`, but only when HEAD is a new commit
/// whose parent is the commit recorded in `base`, or a new root commit (rewording the first
/// commit of a `--root` rebase, which starts from a parentless placeholder). Both arguments
/// are already shell-quoted.
pub(super) fn guarded_amend(base: &str, file: &str) -> String {
    format!(
        r#"exec h=$(git rev-parse -q --verify HEAD); p=$(git rev-parse -q --verify HEAD^); b=$(cat {base}); if [ "$h" != "$b" ] && {{ [ "$p" = "$b" ] || [ -z "$p" ]; }}; then git commit --amend --allow-empty --no-verify --cleanup=whitespace -F {file}; fi"#
    ) + "\n"
}

/// `s` quoted for `sh` (git runs editors and exec lines through it, also on Windows); see
/// [`shell_quote_for`].
pub fn shell_quote(s: &str) -> String {
    shell_quote_for(s, cfg!(windows))
}

/// `s` quoted for `sh`. With `windows`, backslashes (Windows' separators) become forward
/// slashes, which Git for Windows' sh reads as the same path; on Unix a backslash is part of a
/// file name and is kept.
pub fn shell_quote_for(s: &str, windows: bool) -> String {
    let s = if windows { s.replace('\\', "/") } else { s.to_owned() };
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// The `GIT_SEQUENCE_EDITOR` command that installs `prepared`; git appends the todo's path.
pub fn editor_command(editor: &SequenceEditor, prepared: &Path) -> String {
    let prepared = shell_quote(&prepared.to_string_lossy());
    match editor {
        SequenceEditor::Helper(exe) => format!("{} --write-todo {prepared}", shell_quote(&exe.to_string_lossy())),
        SequenceEditor::Copy => format!("cp {prepared}"),
        SequenceEditor::Shell(s) => s.clone(),
    }
}

/// The commit ids a todo text lists, in order: the second word of every pick, reword, edit,
/// squash, fixup or drop line (or their one-letter forms), skipping options such as
/// `fixup -C`. Comments and other commands (exec, label, reset, merge, update-ref, break) are
/// ignored.
pub fn listed_oids(text: &str) -> Vec<String> {
    const COMMANDS: [&str; 12] = ["pick", "p", "reword", "r", "edit", "e", "squash", "s", "fixup", "f", "drop", "d"];
    let mut out = Vec::new();
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut words = line.split_whitespace();
        if !words.next().is_some_and(|c| COMMANDS.contains(&c)) {
            continue;
        }
        if let Some(sha) = words.find(|w| !w.starts_with('-')) {
            out.push(sha.to_owned());
        }
    }
    out
}

/// Replaces git's todo (`git_todo`) with `prepared`, after checking that both list the same
/// commits: each id git lists (abbreviated) must be the prefix of exactly one id in `prepared`,
/// and the counts must match. On a mismatch git's todo is left as it is.
pub fn install_todo(prepared: &Path, git_todo: &Path) -> Result<()> {
    let theirs = listed_oids(&std::fs::read_to_string(git_todo)?);
    let text = std::fs::read_to_string(prepared)?;
    let ours = listed_oids(&text);
    if theirs.len() != ours.len() {
        return Err(Error::Unsupported(CHANGED.into()));
    }
    let mut used = vec![false; ours.len()];
    for t in &theirs {
        let t = t.to_ascii_lowercase();
        let mut matches = ours.iter().enumerate().filter(|(_, o)| o.to_ascii_lowercase().starts_with(&t)).map(|(i, _)| i);
        match (matches.next(), matches.next()) {
            (Some(i), None) if !used[i] => used[i] = true,
            _ => return Err(Error::Unsupported(CHANGED.into())),
        }
    }
    crate::persist::atomic_write(git_todo, text.as_bytes())?;
    Ok(())
}
