//! Recognising the command lines Glowfly builds for undoable operations.

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

/// An operation that moves HEAD (and the checked-out branch) to new commits; undone with
/// `reset --keep`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveOp {
    Merge,
    Pull,
    CherryPick,
    Revert,
    Rebase,
    /// An interactive rebase or a history edit (reword, squash, drop); recorded around the
    /// rebase run (`rebased.rs`), never from a command line.
    Rewrite,
}

impl MoveOp {
    pub fn noun(self) -> &'static str {
        match self {
            MoveOp::Merge => "merge",
            MoveOp::Pull => "pull",
            MoveOp::CherryPick => "cherry-pick",
            MoveOp::Revert => "revert",
            MoveOp::Rebase => "rebase",
            MoveOp::Rewrite => "history edit",
        }
    }
}

/// Which kind of undoable operation a git command line is.
#[derive(Clone, Debug, PartialEq)]
pub enum UndoKind {
    Checkout,
    Commit {
        amend: bool,
    },
    Reset(ResetMode),
    DeleteBranch(String),
    DiscardAll,
    CreateBranch(String),
    CreateTag(String),
    /// `detail` names what was merged, picked or rebased onto, for the label.
    Move {
        op: MoveOp,
        detail: Option<String>,
    },
    /// `replay` is the command lines that stash again on redo (the push, and for chosen files
    /// the commands that follow it: `stash_ops::restore_commands`); `None` when it read its
    /// paths from a file that may be gone by then.
    StashPush {
        replay: Option<Vec<Vec<String>>>,
    },
    /// A pop (or apply) of `rev` (`None`: the newest stash).
    StashApply {
        pop: bool,
        rev: Option<String>,
    },
    /// `merge --continue`, `cherry-pick`/`revert` `--continue` or `--skip`: carries on an
    /// operation that stopped for conflicts; recorded as the whole operation once nothing is
    /// left in progress.
    Resume(MoveOp),
    /// A drop of `rev` (`None`: the newest stash).
    StashDrop {
        rev: Option<String>,
    },
    /// `update-ref -m <reason> refs/heads/<name> <new> <old>`: the journal's Restore of a branch
    /// that isn't checked out.
    SetBranch(String),
}

fn full_id(s: &str) -> bool {
    s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn plain(s: &str) -> bool {
    !s.starts_with('-')
}

impl UndoKind {
    /// Recognises the command lines Glowfly builds for undoable operations. A leading
    /// `--literal-pathspecs` (a custom command with `{files}`) changes only how paths match.
    pub fn classify(argv: &[String]) -> Option<UndoKind> {
        let a: Vec<&str> = argv.iter().map(String::as_str).collect();
        match a.as_slice() {
            ["--literal-pathspecs", "stash", ..] => stash_kind(&a, argv),
            ["--literal-pathspecs", ..] => Self::classify(&argv[1..]),
            ["checkout", rest @ ..] => checkout_kind(rest),
            ["commit", ..] => Some(UndoKind::Commit { amend: a.contains(&"--amend") }),
            ["reset", mode, rev] if plain(rev) => match *mode {
                "--soft" => Some(UndoKind::Reset(ResetMode::Soft)),
                "--mixed" => Some(UndoKind::Reset(ResetMode::Mixed)),
                "--hard" => Some(UndoKind::Reset(ResetMode::Hard)),
                _ => None,
            },
            ["branch", "-d" | "-D", name] if plain(name) => Some(UndoKind::DeleteBranch((*name).to_owned())),
            ["branch", name] if plain(name) => Some(UndoKind::CreateBranch((*name).to_owned())),
            ["branch", name, start] | ["branch", "--track", name, start] if plain(name) && plain(start) => {
                Some(UndoKind::CreateBranch((*name).to_owned()))
            }
            ["tag", name, at] if plain(name) && plain(at) => Some(UndoKind::CreateTag((*name).to_owned())),
            ["tag", "-a", name, at, "-m", _] if plain(name) && plain(at) => Some(UndoKind::CreateTag((*name).to_owned())),
            [op @ ("merge" | "cherry-pick" | "revert"), "--continue"] | [op @ ("cherry-pick" | "revert"), "--skip"] => {
                Some(UndoKind::Resume(match *op {
                    "merge" => MoveOp::Merge,
                    "cherry-pick" => MoveOp::CherryPick,
                    _ => MoveOp::Revert,
                }))
            }
            [op @ ("merge" | "pull" | "cherry-pick" | "revert" | "rebase"), rest @ ..] => move_kind(op, rest),
            ["update-ref", "-m", _, r, new, old] if full_id(new) && full_id(old) => {
                r.strip_prefix("refs/heads/").filter(|n| !n.is_empty()).map(|n| UndoKind::SetBranch(n.to_owned()))
            }
            _ => stash_kind(&a, argv),
        }
    }
}

/// `checkout <target>`, with `--track`, `--detach` or a trailing `--`; or `checkout -b <name>`
/// with an optional start point.
fn checkout_kind(rest: &[&str]) -> Option<UndoKind> {
    let mut targets = 0;
    let mut new_branch = false;
    let mut args = rest.iter().enumerate();
    while let Some((i, arg)) = args.next() {
        match *arg {
            "--track" | "--detach" => {}
            "-b" => match args.next() {
                Some((_, name)) if plain(name) && !new_branch => new_branch = true,
                _ => return None,
            },
            // A trailing `--` only stops the target being read as a path.
            "--" if i + 1 == rest.len() => {}
            s if s.starts_with('-') => return None,
            _ => targets += 1,
        }
    }
    let fits = if new_branch { targets <= 1 } else { targets == 1 };
    fits.then_some(UndoKind::Checkout)
}

/// What a merge, pull, cherry-pick, revert or rebase did, unless it squashes, leaves the
/// result uncommitted, edits interactively or carries on an operation already started (a
/// plain Continue or Skip is a `Resume`, matched before this).
fn move_kind(op: &str, rest: &[&str]) -> Option<UndoKind> {
    const NOT_RECORDED: &[&str] =
        &["--abort", "--continue", "--skip", "--quit", "--squash", "--no-commit", "-n", "-i", "--interactive", "--edit-todo"];
    if rest.iter().any(|a| NOT_RECORDED.contains(a)) {
        return None;
    }
    let picks = matches!(op, "cherry-pick" | "revert");
    let targets = move_targets(rest, picks);
    let (op, detail) = match op {
        "merge" => (MoveOp::Merge, targets.first().map(|t| (*t).to_owned())),
        "rebase" => (MoveOp::Rebase, targets.first().map(|t| (*t).to_owned())),
        "cherry-pick" => (MoveOp::CherryPick, commits_label(&targets)),
        "revert" => (MoveOp::Revert, commits_label(&targets)),
        _ => (MoveOp::Pull, None),
    };
    Some(UndoKind::Move { op, detail })
}

/// The plain arguments of a move command. For a cherry-pick or revert (`picks`), a mainline
/// parent (`-m N`, `--mainline N`, `-mN`, `--mainline=N`) is left out: it isn't a commit.
pub(crate) fn move_targets<'a>(rest: &[&'a str], picks: bool) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut args = rest.iter();
    while let Some(a) = args.next() {
        match *a {
            "-m" | "--mainline" if picks => {
                args.next();
            }
            s if plain(s) => out.push(s),
            _ => {}
        }
    }
    out
}

/// "abc1234" for one commit, "3 commits" for several.
pub(crate) fn commits_label(targets: &[&str]) -> Option<String> {
    match targets {
        [] => None,
        [one] => Some(commit_label(one)),
        many => Some(format!("{} commits", many.len())),
    }
}

/// A full commit id cut to its short form; anything else as it is.
fn commit_label(rev: &str) -> String {
    if rev.len() == 40 && rev.bytes().all(|b| b.is_ascii_hexdigit()) { rev[..7].to_owned() } else { rev.to_owned() }
}

/// `stash push` (Discard all's own stash is `DiscardAll`), and `stash pop` / `stash apply` /
/// `stash drop` of one entry.
fn stash_kind(a: &[&str], argv: &[String]) -> Option<UndoKind> {
    match a {
        ["stash", "push", "-u", "-m", m] if *m == DISCARD_STASH_MESSAGE => Some(UndoKind::DiscardAll),
        ["stash", "push", ..] | ["--literal-pathspecs", "stash", "push", ..] => {
            let from_file = a.iter().any(|s| s.starts_with("--pathspec-from-file"));
            Some(UndoKind::StashPush { replay: (!from_file).then(|| vec![argv.to_vec()]) })
        }
        ["stash", verb @ ("pop" | "apply")] => Some(UndoKind::StashApply { pop: *verb == "pop", rev: None }),
        ["stash", verb @ ("pop" | "apply"), rev] if plain(rev) => {
            Some(UndoKind::StashApply { pop: *verb == "pop", rev: Some((*rev).to_owned()) })
        }
        ["stash", "drop"] => Some(UndoKind::StashDrop { rev: None }),
        ["stash", "drop", rev] if plain(rev) => Some(UndoKind::StashDrop { rev: Some((*rev).to_owned()) }),
        _ => None,
    }
}
