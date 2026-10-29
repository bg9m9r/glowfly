//! A pull pinned to the commits that were checked. `git pull` fetches and then merges or
//! rebases onto whatever that fetch brought, so when hooks follow the working tree the commit
//! it merges can't be compared before git runs its hooks. Instead the pull is split: its fetch
//! runs first (`Pull::fetch`), the commits it marked for merging are read from FETCH_HEAD
//! (`fetched`) and compared, and then exactly those are merged or rebased onto (`Pull::pinned`)
//! with the pull's own choice of merge or rebase (`--rebase`, `branch.<name>.rebase`,
//! `pull.rebase`) and fast-forward (`--ff`, `--ff-only`, `pull.ff`), decided as git 2.34 and
//! later decide it (`Pull::plan`), with no second fetch.

use git2::{Oid, Repository};

/// A `git pull` command split into its fetch and what follows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Pull {
    /// git's own options before `pull` (`-c key=value`), for both commands.
    globals: Vec<String>,
    /// `fetch` with the pull's remote, refspecs and fetch options.
    pub fetch: Vec<String>,
    /// The options that go to the merge or rebase.
    merge: Vec<String>,
    /// `--log[=<n>]` or `--no-log`: they go to `git fmt-merge-msg`, which makes the message.
    log: Vec<String>,
    rebase: Vec<String>,
    /// `--rebase[=<how>]` or `--no-rebase` as given.
    how: Option<String>,
    /// `--ff`, `--no-ff` or `--ff-only` as given.
    ff: Option<String>,
    /// `--dry-run`: only the fetch runs.
    dry_run: bool,
    /// A remote or refspec was named: the fork point isn't the branch's upstream.
    named: bool,
}

/// One commit FETCH_HEAD marks for merging, with its line as git wrote it.
#[derive(Clone, Debug, PartialEq)]
pub struct Fetched {
    pub oid: Oid,
    pub line: String,
}

/// Options that go to both commands.
const BOTH: &[&str] = &["-q", "--quiet", "-v", "--verbose", "--progress", "--no-progress"];
/// Options that go to the merge only.
const MERGE: &[&str] = &[
    "--commit",
    "--no-commit",
    "-e",
    "--edit",
    "--no-edit",
    "--stat",
    "-n",
    "--no-stat",
    "--summary",
    "--no-summary",
    "--compact-summary",
    "--squash",
    "--no-squash",
    "--verify-signatures",
    "--no-verify-signatures",
    "--allow-unrelated-histories",
];
/// Options that go to the merge and to the rebase.
const SHARED: &[&str] = &["--signoff", "--no-signoff", "--verify", "--no-verify", "--autostash", "--no-autostash", "--no-gpg-sign"];
/// Options of the merge and the rebase that take the next argument as their value.
const SHARED_VALUED: &[&str] = &["-s", "--strategy", "-X", "--strategy-option"];
/// Fetch options that take the next argument as their value.
const FETCH_VALUED: &[&str] = &[
    "--depth",
    "--deepen",
    "--shallow-since",
    "--shallow-exclude",
    "-j",
    "--jobs",
    "--upload-pack",
    "-o",
    "--server-option",
    "--refmap",
    "--negotiation-tip",
    "--filter",
];

impl Pull {
    /// The pull `cmd` (its arguments, without `git`), or None when it isn't one.
    pub fn of(cmd: &[String]) -> Option<Pull> {
        let at = super::incoming::sub_at(cmd)?;
        if cmd[at] != "pull" {
            return None;
        }
        let globals = cmd[..at].to_vec();
        let mut p = Pull {
            globals: globals.clone(),
            fetch: [globals, vec!["fetch".to_owned(), "--update-head-ok".to_owned()]].concat(),
            merge: Vec::new(),
            log: Vec::new(),
            rebase: Vec::new(),
            how: None,
            ff: None,
            dry_run: false,
            named: false,
        };
        let mut it = cmd[at + 1..].iter();
        while let Some(a) = it.next() {
            let s = a.as_str();
            match s {
                "-r" | "--rebase" => p.how = Some("true".to_owned()),
                "--no-rebase" => p.how = Some("false".to_owned()),
                _ if s.starts_with("--rebase=") => p.how = Some(s["--rebase=".len()..].to_owned()),
                "--ff" | "--no-ff" | "--ff-only" => p.ff = Some(s.to_owned()),
                "--dry-run" => {
                    p.dry_run = true;
                    p.fetch.push(a.clone());
                }
                _ if BOTH.contains(&s) => {
                    p.fetch.push(a.clone());
                    p.merge.push(a.clone());
                    p.rebase.push(a.clone());
                }
                "--log" | "--no-log" => p.log.push(a.clone()),
                _ if s.starts_with("--log=") => p.log.push(a.clone()),
                _ if MERGE.contains(&s) || s.starts_with("--cleanup=") => p.merge.push(a.clone()),
                _ if SHARED.contains(&s) || s.starts_with("-S") || s.starts_with("--gpg-sign") || shared_joined(s) => {
                    p.merge.push(a.clone());
                    p.rebase.push(a.clone());
                }
                _ if SHARED_VALUED.contains(&s) => {
                    let v = it.next().cloned().unwrap_or_default();
                    p.merge.extend([a.clone(), v.clone()]);
                    p.rebase.extend([a.clone(), v]);
                }
                _ if FETCH_VALUED.contains(&s) => {
                    p.fetch.push(a.clone());
                    p.fetch.extend(it.next().cloned());
                }
                _ if s.starts_with('-') => p.fetch.push(a.clone()),
                _ => {
                    p.named = true;
                    p.fetch.push(a.clone());
                }
            }
        }
        Some(p)
    }

    /// Whether only the fetch runs (`--dry-run`).
    pub fn fetch_only(&self) -> bool {
        self.dry_run
    }

    /// How this pull takes `heads` (FETCH_HEAD's commits for merging), decided as `git pull`
    /// (2.34 and later) decides: a rebase chosen on the command line overrides `pull.ff=only`,
    /// `--ff-only` (given or set) overrides a rebase, a rebase that can fast-forward does, and
    /// divergent branches with neither a rebase nor a fast-forward choice anywhere are refused.
    /// `Err` with git's reason when the pull can't go on.
    pub fn plan(&self, repo: &Repository, heads: &[Fetched]) -> Result<Plan, String> {
        if heads.is_empty() {
            return Err("There is no candidate for merging among the refs that you just fetched.".to_owned());
        }
        let config = repo.config().and_then(|mut c| c.snapshot()).map_err(|e| e.message().to_owned())?;
        let mut ff = self.ff.clone().or_else(|| configured_ff(&config));
        if self.ff.is_none() && self.how.is_some() && ff.as_deref() == Some("--ff-only") {
            ff = Some("--ff".to_owned());
        }
        let (mut rebase, unspecified) = self.rebase_choice(repo, &config);
        let Ok(head) = repo.head().and_then(|h| h.peel_to_commit()).map(|c| c.id()) else {
            // An unborn branch takes the fetched commit as it is.
            return Ok(Plan::Merge(ff));
        };
        let descends = |a: Oid, b: Oid| a == b || repo.graph_descendant_of(a, b).unwrap_or(false);
        let can_ff = matches!(heads, [h] if descends(h.oid, head));
        let up_to_date = heads.iter().all(|h| descends(head, h.oid));
        let divergent = !can_ff && !up_to_date;
        if ff.as_deref() == Some("--ff-only") {
            if divergent {
                return Err("Not possible to fast-forward, aborting.".to_owned());
            }
            rebase = None;
        }
        if divergent && unspecified && ff.is_none() {
            return Err("You have divergent branches and need to specify how to reconcile them.".to_owned());
        }
        match rebase {
            Some(_) if can_ff => Ok(Plan::FastForward),
            Some(_) if heads.len() > 1 => Err("Cannot rebase onto multiple branches.".to_owned()),
            Some(how) => Ok(Plan::Rebase(how)),
            None => Ok(Plan::Merge(ff)),
        }
    }

    /// Whether the pull rebases, and how (the option, else `branch.<current>.rebase`, else
    /// `pull.rebase`); and whether nothing chose either way.
    fn rebase_choice(&self, repo: &Repository, config: &git2::Config) -> (Option<&'static str>, bool) {
        let branch = repo.head().ok().filter(|h| h.is_branch()).and_then(|h| h.shorthand().map(str::to_owned));
        let configured =
            branch.and_then(|b| config.get_string(&format!("branch.{b}.rebase")).ok()).or_else(|| config.get_string("pull.rebase").ok());
        let unspecified = self.how.is_none() && configured.is_none();
        (self.how.clone().or(configured).and_then(|how| rebase_kind(&how)), unspecified)
    }

    /// Whether git would require a clean working tree before this pull fetches: it rebases
    /// (`--ff-only` aside, which git checks only later) without `--autostash` or
    /// `rebase.autoStash`.
    pub(super) fn needs_clean_tree(&self, repo: &Repository) -> bool {
        let Ok(config) = repo.config().and_then(|mut c| c.snapshot()) else { return false };
        let stash = match (self.rebase.iter().rev().find(|a| a.ends_with("autostash")), config.get_bool("rebase.autoStash")) {
            (Some(a), _) => a == "--autostash",
            (None, v) => v.unwrap_or(false),
        };
        self.rebase_choice(repo, &config).0.is_some() && !stash && repo.head().is_ok()
    }

    /// The command that takes exactly `heads` as `plan` says: a merge of them with `message`
    /// (git's own, from `git fmt-merge-msg`), a fast-forward to the one of them, or a rebase
    /// onto it from `fork_point` when there is one.
    pub fn command(&self, plan: &Plan, heads: &[Fetched], message: &str, fork_point: Option<Oid>) -> Vec<String> {
        let oids = heads.iter().map(|h| h.oid.to_string());
        match plan {
            Plan::Rebase(how) => {
                let mut cmd = [self.globals.clone(), vec!["rebase".to_owned()], self.rebase.clone()].concat();
                if !how.is_empty() {
                    cmd.push((*how).to_owned());
                }
                let onto = heads[0].oid.to_string();
                cmd.extend(["--onto".to_owned(), onto.clone(), fork_point.map_or(onto, |f| f.to_string())]);
                cmd
            }
            Plan::FastForward => [self.globals.clone(), crate::gitcli::args(&["merge", "--ff-only"]), oids.collect()].concat(),
            Plan::Merge(ff) => {
                let mut cmd = [self.globals.clone(), vec!["merge".to_owned()], self.merge.clone()].concat();
                cmd.extend(ff.clone());
                // The message has the log already, as merge.log or the pull's --log made it.
                cmd.extend(["--no-log".to_owned(), "-m".to_owned(), message.trim_end().to_owned()]);
                cmd.extend(oids);
                cmd
            }
        }
    }

    /// `plan` and `command` together (`message` is used only by a merge).
    pub fn pinned(&self, repo: &Repository, heads: &[Fetched], message: &str, fork_point: Option<Oid>) -> Result<Vec<String>, String> {
        Ok(self.command(&self.plan(repo, heads)?, heads, message, fork_point))
    }

    /// The remote-tracking branch whose reflog gives the fork point a rebasing pull starts
    /// from: the current branch's upstream, when the pull named no remote.
    pub fn fork_ref(&self, repo: &Repository) -> Option<String> {
        if self.named {
            return None;
        }
        let head = repo.head().ok()?;
        repo.branch_upstream_name(head.name()?).ok()?.as_str().map(str::to_owned)
    }
}

/// How a pull takes the commits it fetched (`Pull::plan`).
#[derive(Clone, Debug, PartialEq)]
pub enum Plan {
    /// A merge, with the fast-forward option it gets (None: git's own default).
    Merge(Option<String>),
    /// A rebase, with how: "", "-i" or "--rebase-merges".
    Rebase(&'static str),
    /// A rebasing pull with nothing of its own to replay: git fast-forwards instead.
    FastForward,
}

/// `pull.ff` as the merge option it means.
fn configured_ff(config: &git2::Config) -> Option<String> {
    let v = config.get_string("pull.ff").ok()?.to_ascii_lowercase();
    Some(
        match v.as_str() {
            "only" => "--ff-only",
            "false" | "no" | "off" | "0" => "--no-ff",
            _ => "--ff",
        }
        .to_owned(),
    )
}

/// A rebase setting (`true`, `merges`, `interactive`, `false`) as how the rebase runs; None
/// for a merge.
fn rebase_kind(how: &str) -> Option<&'static str> {
    match how.to_ascii_lowercase().as_str() {
        "false" | "no" | "off" | "0" => None,
        "merges" | "m" | "preserve" | "p" => Some("--rebase-merges"),
        "interactive" | "i" => Some("-i"),
        _ => Some(""),
    }
}

/// `-s<x>`, `-X<x>` or `--strategy=<x>` written as one argument.
fn shared_joined(s: &str) -> bool {
    (s.len() > 2 && (s.starts_with("-s") || s.starts_with("-X"))) || s.starts_with("--strategy=") || s.starts_with("--strategy-option=")
}

/// The commits the last fetch in `repo` marked for merging, from FETCH_HEAD, with their lines
/// (for `git fmt-merge-msg`). `Err` when it can't be read or a line doesn't parse.
pub fn fetched(repo: &Repository) -> Result<Vec<Fetched>, String> {
    let path = repo.path().join("FETCH_HEAD");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("Couldn't read {}: {e}", path.display()))?;
    let mut out = Vec::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let mut parts = line.splitn(3, '\t');
        let (oid, mark) = (parts.next().unwrap_or_default(), parts.next().unwrap_or_default());
        let oid = Oid::from_str(oid).map_err(|_| format!("Couldn't read {}: {line}", path.display()))?;
        if mark.is_empty() {
            out.push(Fetched { oid, line: line.to_owned() });
        }
    }
    Ok(out)
}

#[path = "pull_run.rs"]
mod run;
pub use run::{Git, PullRun, run_pinned};

#[cfg(test)]
#[path = "pull_tests.rs"]
mod tests;
