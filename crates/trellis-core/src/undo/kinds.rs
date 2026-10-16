//! Recognising the command lines Trellis builds for undoable operations.

use super::DISCARD_STASH_MESSAGE;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResetMode {
    Soft,
    Mixed,
    Hard,
}

impl ResetMode {
    pub fn flag(self) -> &'static str {
        match self {
            ResetMode::Soft => "--soft",
            ResetMode::Mixed => "--mixed",
            ResetMode::Hard => "--hard",
        }
    }

    pub(super) fn name(self) -> &'static str {
        &self.flag()[2..]
    }
}

/// Which kind of undoable operation a git command line is.
#[derive(Clone, Debug, PartialEq)]
pub enum UndoKind {
    Checkout,
    Commit { amend: bool },
    Reset(ResetMode),
    DeleteBranch(String),
    DiscardAll,
}

impl UndoKind {
    /// Recognises the command lines Trellis builds for undoable operations.
    pub fn classify(argv: &[String]) -> Option<UndoKind> {
        let a: Vec<&str> = argv.iter().map(String::as_str).collect();
        match a.as_slice() {
            ["checkout", rest @ ..] => checkout_kind(rest),
            ["commit", ..] => Some(UndoKind::Commit { amend: a.contains(&"--amend") }),
            ["reset", mode, rev] if !rev.starts_with('-') => match *mode {
                "--soft" => Some(UndoKind::Reset(ResetMode::Soft)),
                "--mixed" => Some(UndoKind::Reset(ResetMode::Mixed)),
                "--hard" => Some(UndoKind::Reset(ResetMode::Hard)),
                _ => None,
            },
            ["branch", "-d" | "-D", name] if !name.starts_with('-') => Some(UndoKind::DeleteBranch((*name).to_owned())),
            ["stash", "push", "-u", "-m", m] if *m == DISCARD_STASH_MESSAGE => Some(UndoKind::DiscardAll),
            _ => None,
        }
    }
}

/// `checkout <target>`, with `--track`, `--detach` or a trailing `--`.
fn checkout_kind(rest: &[&str]) -> Option<UndoKind> {
    let mut targets = 0;
    for (i, arg) in rest.iter().enumerate() {
        match *arg {
            "--track" | "--detach" => {}
            // A trailing `--` only stops the target being read as a path.
            "--" if i + 1 == rest.len() => {}
            s if s.starts_with('-') => return None,
            _ => targets += 1,
        }
    }
    (targets == 1).then_some(UndoKind::Checkout)
}
