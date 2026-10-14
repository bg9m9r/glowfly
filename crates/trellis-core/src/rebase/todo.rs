//! The todo list: checking a list before it runs, writing it in git's format (with the reword
//! message files), and installing it over the list git generated.

use super::{CHANGED, EMPTY_REWORD, FIRST_COMBINES, RebaseAction, SequenceEditor, TodoItem};
use crate::error::{Error, Result};
use std::path::{Path, PathBuf};

/// Why the list can't run as it is: a squash or fixup with no kept commit before it, or a
/// reword whose typed message is empty.
pub fn problem(items: &[TodoItem]) -> Option<String> {
    let mut kept = false;
    for item in items {
        match item.action {
            RebaseAction::Drop => {}
            a if a.combines() && !kept => return Some(FIRST_COMBINES.into()),
            _ => kept = true,
        }
    }
    let empty = |i: &TodoItem| i.action == RebaseAction::Reword && i.new_message.as_ref().is_some_and(|m| m.trim().is_empty());
    items.iter().any(empty).then(|| EMPTY_REWORD.into())
}

/// Whether running `items` would replay `original` as it is: the same order, nothing but
/// picks (or rewords that keep their message).
pub fn is_unchanged(items: &[TodoItem], original: &[TodoItem]) -> bool {
    items.len() == original.len()
        && items.iter().zip(original).all(|(a, b)| a.oid == b.oid)
        && items.iter().all(|i| i.action == RebaseAction::Pick || (i.action == RebaseAction::Reword && !i.rewords()))
}

/// The todo text (LF line endings) and the message files to write first. A reword becomes a
/// pick plus an `exec` that amends the message from a file in `msg_dir`; the exec follows the
/// commit's whole group (the squash and fixup rows right after it), so the typed message
/// becomes the combined commit's message. Another `exec` before the pick records HEAD, and the
/// amend runs only when HEAD is then a new commit on top of it: a pick that was skipped (after
/// a conflict, or because it became empty) leaves nothing to reword, and the amend must not
/// rewrite the commit before it.
pub fn build(items: &[TodoItem], msg_dir: &Path) -> (String, Vec<(PathBuf, String)>) {
    let mut text = String::new();
    let mut files = Vec::new();
    let mut pending: Option<String> = None;
    for (i, item) in items.iter().enumerate() {
        let keyword = if item.action == RebaseAction::Reword { "pick" } else { item.action.keyword() };
        let summary = item.summary.replace(['\r', '\n'], " ");
        if item.rewords() {
            let file = msg_dir.join(format!("reword-{}.txt", item.oid));
            let base = shell_quote(&msg_dir.join(format!("reword-{}.base", item.oid)).to_string_lossy());
            let message = item.new_message.as_deref().unwrap_or_default().trim_end().to_owned() + "\n";
            text.push_str(&format!("exec git rev-parse HEAD >{base}\n"));
            pending = Some(guarded_amend(&base, &shell_quote(&file.to_string_lossy())));
            files.push((file, message));
        }
        text.push_str(&format!("{keyword} {} {}\n", item.oid, summary.trim_end()));
        // The group ends where the next row neither combines nor is dropped.
        let continues =
            items[i + 1..].iter().take_while(|n| n.action == RebaseAction::Drop || n.action.combines()).any(|n| n.action.combines());
        if !continues && let Some(exec) = pending.take() {
            text.push_str(&exec);
        }
    }
    (text, files)
}

/// The `exec` line that amends HEAD's message from `file`, but only when HEAD is a new commit
/// whose parent is the commit recorded in `base`, or a new root commit (rewording the first
/// commit of a `--root` rebase, which starts from a parentless placeholder). Both arguments
/// are already shell-quoted.
fn guarded_amend(base: &str, file: &str) -> String {
    format!(
        r#"exec h=$(git rev-parse -q --verify HEAD); p=$(git rev-parse -q --verify HEAD^); b=$(cat {base}); if [ "$h" != "$b" ] && {{ [ "$p" = "$b" ] || [ -z "$p" ]; }}; then git commit --amend --allow-empty --no-verify --cleanup=whitespace -F {file}; fi"#
    ) + "\n"
}

/// `s` quoted for `sh` (git runs editors and exec lines through it, also on Windows), with
/// backslashes turned into forward slashes.
pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\\', "/").replace('\'', "'\\''"))
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
    std::fs::write(git_todo, text)?;
    Ok(())
}
