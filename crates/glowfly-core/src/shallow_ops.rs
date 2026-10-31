//! Fetching more of a shallow clone's history: the `git fetch` command lines for completing it
//! (--unshallow) or deepening it by a commit count (--deepen). The app runs them through gitcli.

/// How much more history to download.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Deepen {
    /// Everything the remote has: the clone becomes a full one.
    Full,
    /// This many older commits below where each fetched branch's history now stops.
    By(u32),
}

impl Deepen {
    /// The job's label in the status bar, toast and log.
    pub fn job_label(self) -> String {
        match self {
            Deepen::Full => "Fetch full history".to_owned(),
            Deepen::By(1) => "Fetch 1 more commit".to_owned(),
            Deepen::By(n) => format!("Fetch {n} more commits"),
        }
    }
}

/// `git fetch` arguments (without the leading `git`) that download `how` much more history from
/// `remote`. `--deepen` counts from the current shallow boundary, not from each branch tip.
pub fn deepen_cmd(remote: &str, how: Deepen) -> Vec<String> {
    let flag = match how {
        Deepen::Full => "--unshallow".to_owned(),
        Deepen::By(n) => format!("--deepen={n}"),
    };
    vec!["fetch".to_owned(), flag, "--".to_owned(), remote.to_owned()]
}

#[cfg(test)]
mod tests;
