//! What a reflog entry's message says happened (pure). Git writes `<action>: <details>`, where
//! the action is a command name with an optional phase in parentheses ("commit (amend)",
//! "rebase (finish)"); `GIT_REFLOG_ACTION` can put a longer command in its place ("rebase -i",
//! "pull --rebase", "pull origin main"), so only the action's first word and its phase decide.

/// One reflog entry's meaning. `Other` is anything not recognised: its raw text is shown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Commit {
        subject: String,
    },
    Initial {
        subject: String,
    },
    Amend {
        subject: String,
    },
    MergeCommit {
        subject: String,
    },
    Checkout {
        from: String,
        to: String,
    },
    Reset {
        to: String,
    },
    Merge {
        from: String,
        fast_forward: bool,
    },
    Pull {
        rebase: bool,
    },
    /// A rebase's start or one of its steps (pick, reword, edit, squash, fixup, continue, skip).
    RebaseStep,
    /// The rebase finished; `onto` is the commit it went onto when the message names one.
    RebaseFinish {
        onto: Option<String>,
    },
    RebaseAbort,
    CherryPick {
        subject: String,
    },
    Revert {
        subject: String,
    },
    BranchCreated {
        from: String,
    },
    BranchReset {
        to: String,
    },
    BranchRenamed {
        from: String,
    },
    Clone,
    Fetch,
    Am {
        subject: String,
    },
    Other,
}

impl Event {
    /// The entries the timeline leaves out: a rebase's start and its steps (its finish and
    /// abort show).
    pub fn hidden(&self) -> bool {
        matches!(self, Event::RebaseStep)
    }
}

/// A full commit id cut to its short form; anything else as it is.
pub(crate) fn short_rev(rev: &str) -> String {
    if rev.len() == 40 && rev.bytes().all(|b| b.is_ascii_hexdigit()) { rev[..7].to_owned() } else { rev.to_owned() }
}

/// A ref name as people say it: without refs/heads/ or refs/remotes/.
fn ref_label(name: &str) -> &str {
    name.strip_prefix("refs/heads/").or_else(|| name.strip_prefix("refs/remotes/")).unwrap_or(name)
}

/// The meaning of reflog message `message`.
pub fn parse(message: &str) -> Event {
    let message = message.trim_end_matches(['\n', '\r']);
    // "Branch: renamed refs/heads/a to refs/heads/b" (git writes it capitalised).
    if let Some(rest) = message.strip_prefix("Branch: renamed ") {
        return match rest.split_once(" to ") {
            Some((from, _)) => Event::BranchRenamed { from: ref_label(from).to_owned() },
            None => Event::Other,
        };
    }
    let Some((action, rest)) = message.split_once(": ") else { return Event::Other };
    let (base, phase) = match action.strip_suffix(')').and_then(|a| a.rsplit_once(" (")) {
        Some((base, phase)) => (base, Some(phase)),
        None => (action, None),
    };
    let mut words = base.split_whitespace();
    let verb = words.next().unwrap_or_default();
    let flags: Vec<&str> = words.collect();
    let subject = rest.to_owned();
    match (verb, phase) {
        ("commit", None) => Event::Commit { subject },
        ("commit", Some("initial")) => Event::Initial { subject },
        ("commit", Some("amend")) => Event::Amend { subject },
        ("commit", Some("merge")) => Event::MergeCommit { subject },
        ("commit", Some(_)) => Event::Other,
        ("checkout", None) => match rest.strip_prefix("moving from ").and_then(|r| r.split_once(" to ")) {
            Some((from, to)) => Event::Checkout { from: from.to_owned(), to: to.to_owned() },
            None => Event::Other,
        },
        ("reset", None) => match rest.strip_prefix("moving to ") {
            Some(to) => Event::Reset { to: short_rev(to) },
            None => Event::Other,
        },
        ("merge", None) => match flags.last() {
            Some(from) => Event::Merge { from: ref_label(from).to_owned(), fast_forward: rest.starts_with("Fast-forward") },
            None => Event::Other,
        },
        ("pull", None) => Event::Pull { rebase: flags.iter().any(|f| matches!(*f, "--rebase" | "-r")) },
        // A pull that rebased: its finish is the pull's own state.
        ("pull", Some("finish")) => Event::Pull { rebase: true },
        (_, Some("abort")) if matches!(verb, "rebase" | "pull") => Event::RebaseAbort,
        ("rebase", Some("finish")) => Event::RebaseFinish { onto: rest.rsplit_once(" onto ").map(|(_, o)| short_rev(o.trim())) },
        ("rebase" | "pull", Some(_)) => Event::RebaseStep,
        ("cherry-pick", None) => Event::CherryPick { subject },
        ("revert", None) => Event::Revert { subject },
        ("branch", None) => {
            if let Some(from) = rest.strip_prefix("Created from ") {
                Event::BranchCreated { from: ref_label(from).to_owned() }
            } else if let Some(to) = rest.strip_prefix("Reset to ") {
                Event::BranchReset { to: short_rev(ref_label(to)) }
            } else {
                Event::Other
            }
        }
        ("clone", None) => Event::Clone,
        ("fetch", None) => Event::Fetch,
        ("am", None) => Event::Am { subject },
        _ => Event::Other,
    }
}
