//! A pull pinned to the commits that were checked. `git pull` fetches and then merges or
//! rebases onto whatever that fetch brought, so when hooks follow the working tree the commit
//! it merges can't be compared before git runs its hooks. Instead the pull is split: its fetch
//! runs first (`Pull::fetch`), the commits it marked for merging are read from FETCH_HEAD
//! (`fetched`) and compared, and then exactly those are merged or rebased onto (`Pull::pinned`)
//! with the pull's own choice of merge or rebase (`--rebase`, `branch.<name>.rebase`,
//! `pull.rebase`) and fast-forward (`--ff`, `--ff-only`, `pull.ff`), decided as git 2.34 and
//! later decide it (`Pull::plan`), with no second fetch: a merge is `git merge FETCH_HEAD`
//! (git's own message), a rebase starts from the fork point git would use, and both carry
//! `GIT_REFLOG_ACTION` as git pull sets it. Values git refuses are refused the same way.

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
    rebase: Vec<String>,
    /// `--rebase[=<how>]` or `--no-rebase` as given.
    how: Option<String>,
    /// `--ff`, `--no-ff` or `--ff-only` as given.
    ff: Option<String>,
    /// `--dry-run`: only the fetch runs.
    dry_run: bool,
    /// The remote and the first refspec named, for the fork point.
    remote: Option<String>,
    refspec: Option<String>,
    /// Every refspec named, in order.
    refspecs: Vec<String>,
    /// `--recurse-submodules` or `--no-recurse-submodules` as given.
    recurse: Option<bool>,
    /// `GIT_REFLOG_ACTION` as git pull sets it: `pull` and its arguments.
    pub reflog: String,
    /// The command as given.
    cmd: Vec<String>,
}

/// One commit FETCH_HEAD marks for merging (for a tag fetched, the tag), with its line.
#[derive(Clone, Debug, PartialEq)]
pub struct Fetched {
    pub oid: Oid,
    pub line: String,
}

impl Pull {
    /// The pull as it was given (run as it is when the person chose to run what can't be
    /// pinned).
    pub fn original(&self) -> Vec<String> {
        self.cmd.clone()
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
        let mut ff = match &self.ff {
            Some(f) => Some(f.clone()),
            None => configured_ff(&config)?,
        };
        if self.ff.is_none() && self.how.is_some() && ff.as_deref() == Some("--ff-only") {
            ff = Some("--ff".to_owned());
        }
        let (mut rebase, unspecified) = self.rebase_choice(repo, &config)?;
        let Ok(head) = repo.head().and_then(|h| h.peel_to_commit()).map(|c| c.id()) else {
            // An unborn branch takes the fetched commit as it is.
            return Ok(Plan::Merge(ff));
        };
        let descends =
            |a: Oid, b: Oid| -> Result<bool, String> { Ok(a == b || repo.graph_descendant_of(a, b).map_err(|e| e.message().to_owned())?) };
        let can_ff = match heads {
            [h] => descends(h.oid, head)?,
            _ => false,
        };
        let mut up_to_date = true;
        for h in heads {
            up_to_date &= descends(head, h.oid)?;
        }
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
    fn rebase_choice(&self, repo: &Repository, config: &git2::Config) -> Result<(Option<&'static str>, bool), String> {
        if let Some(how) = &self.how {
            return Ok((rebase_kind(how, "--rebase")?, false));
        }
        let branch = repo.head().ok().filter(|h| h.is_branch()).and_then(|h| h.shorthand().map(str::to_owned));
        let keys = branch.map(|b| format!("branch.{b}.rebase")).into_iter().chain(["pull.rebase".to_owned()]);
        for key in keys {
            if let Ok(v) = config.get_string(&key) {
                return Ok((rebase_kind(&v, &key)?, false));
            }
        }
        Ok((None, true))
    }

    /// Whether git would require a clean working tree before this pull fetches: it rebases
    /// (`--ff-only` aside, which git checks only later) without `--autostash` or
    /// `rebase.autoStash`.
    /// `Err` when the config can't be read (or holds a value git refuses).
    pub(super) fn needs_clean_tree(&self, repo: &Repository) -> Result<bool, String> {
        let config = repo.config().and_then(|mut c| c.snapshot()).map_err(|e| e.message().to_owned())?;
        Ok(self.rebase_choice(repo, &config)?.0.is_some() && !self.autostash(&config, "rebase.autoStash") && repo.head().is_ok())
    }

    /// Whether the merge or rebase stashes local changes first: the option, else `key`.
    pub(super) fn autostash(&self, config: &git2::Config, key: &str) -> bool {
        match self.merge.iter().rev().find(|a| a.ends_with("autostash")) {
            Some(a) => a == "--autostash",
            None => config.get_bool(key).unwrap_or(false),
        }
    }

    /// Whether the pull updates submodules after it merges (`--recurse-submodules`, else
    /// `submodule.recurse`).
    pub(super) fn recurses(&self, config: &git2::Config) -> bool {
        self.recurse.unwrap_or_else(|| config.get_bool("submodule.recurse").unwrap_or(false))
    }

    /// The `-X` options and strategy the merge or rebase gets.
    pub(super) fn how(&self) -> crate::trust::incoming::How {
        let mut how = crate::trust::incoming::How::default();
        let mut it = self.rebase.iter();
        while let Some(a) = it.next() {
            let value = |p: &str, it: &mut std::slice::Iter<'_, String>| {
                if a == p { it.next().cloned() } else { a.strip_prefix(p).map(|v| v.trim_start_matches('=').to_owned()) }
            };
            if a.starts_with("--strategy-option") || a.starts_with("-X") {
                how.xopts.extend(value(if a.starts_with("-X") { "-X" } else { "--strategy-option" }, &mut it));
            } else if a.starts_with("--strategy") || a.starts_with("-s") {
                how.strategy = value(if a.starts_with("-s") { "-s" } else { "--strategy" }, &mut it);
            }
        }
        how.unrelated = self.merge.iter().any(|a| a == "--allow-unrelated-histories");
        how
    }

    /// The command that takes exactly `heads` (peeled to commits) as `plan` says: a merge of
    /// FETCH_HEAD (with git's own message), a fast-forward to it (with the merge options, and
    /// `--autostash` when `autostash`), or a rebase onto it from `fork_point` when there is one.
    pub fn command(&self, plan: &Plan, heads: &[Fetched], fork_point: Option<Oid>, autostash: bool) -> Vec<String> {
        let merge = |ff: Option<String>| {
            let mut cmd = [self.globals.clone(), vec!["merge".to_owned()], self.merge.clone()].concat();
            if autostash && !cmd.iter().any(|a| a.ends_with("autostash")) {
                cmd.push("--autostash".to_owned());
            }
            cmd.extend(ff);
            cmd.push("FETCH_HEAD".to_owned());
            cmd
        };
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
            Plan::FastForward => merge(Some("--ff-only".to_owned())),
            Plan::Merge(ff) => merge(ff.clone()),
        }
    }

    /// `plan` and `command` together.
    pub fn pinned(&self, repo: &Repository, heads: &[Fetched], fork_point: Option<Oid>) -> Result<Vec<String>, String> {
        Ok(self.command(&self.plan(repo, heads)?, heads, fork_point, false))
    }

    /// The remote-tracking branch whose reflog gives the fork point a rebasing pull starts
    /// from, as git pull finds it: with a remote and a refspec named, where that remote's
    /// fetch refspecs store the refspec's source; else the current branch's upstream (with a
    /// remote named, only when the branch's remote is that one).
    pub fn fork_ref(&self, repo: &Repository) -> Option<String> {
        if let (Some(remote), Some(spec)) = (&self.remote, &self.refspec) {
            let src = spec.trim_start_matches('+').split(':').next().unwrap_or_default();
            let src = if src.starts_with("refs/") { src.to_owned() } else { format!("refs/heads/{src}") };
            let r = repo.find_remote(remote).ok()?;
            let mut specs = r.refspecs().filter(|s| s.direction() == git2::Direction::Fetch && s.src_matches(&src));
            return specs.find_map(|s| s.transform(&src).ok().and_then(|d| d.as_str().map(str::to_owned)));
        }
        let head = repo.head().ok()?;
        let name = head.name()?;
        if let Some(remote) = &self.remote
            && repo.branch_upstream_remote(name).ok()?.as_str() != Some(remote.as_str())
        {
            return None;
        }
        repo.branch_upstream_name(name).ok()?.as_str().map(str::to_owned)
    }

    /// Whether the fetch may store into the branch checked out, born or not (judged before it
    /// runs, erring towards yes): a refspec named, or a configured fetch refspec of the remote
    /// it fetches from (git applies those to what it fetches even when refspecs are named),
    /// whose destination is that branch, a pattern that could match it, or one it can't read;
    /// or config it can't read. Only an early check (it saves a fetch): the split's fetch runs
    /// without `--update-head-ok`, so git refuses whatever this misses.
    pub(super) fn stores_into_head(&self, repo: &Repository) -> bool {
        // The branch HEAD names, born or not: a fetch into an unborn one makes it.
        let Some(head) = repo.find_reference("HEAD").ok().and_then(|h| h.symbolic_target().map(str::to_owned)) else { return false };
        let remote = self.remote.clone().or_else(|| repo.branch_upstream_remote(&head).ok()?.as_str().map(str::to_owned));
        let key = format!("remote.{}.fetch", remote.as_deref().unwrap_or("origin"));
        let mut configured = Vec::new();
        // Config that can't be read may say anything.
        let Ok(config) = repo.config().and_then(|mut c| c.snapshot()) else { return true };
        match config.multivar(&key, None) {
            Ok(mut values) => {
                while let Some(v) = values.next() {
                    let Ok(v) = v else { return true };
                    match v.value() {
                        Some(v) => configured.push(v.to_owned()),
                        None => return true,
                    }
                }
            }
            Err(e) if e.code() == git2::ErrorCode::NotFound => {}
            Err(_) => return true,
        }
        self.refspecs.iter().chain(&configured).any(|s| may_store(s, &head))
    }
}

/// Whether the fetch refspec `spec` may store into the ref `head` (`refs/heads/…`): its
/// destination, spelled out as git spells out a short one, is `head`, a pattern that could match
/// it, or something this can't read (more than one `*`). A negative refspec or one with no
/// destination stores nothing.
fn may_store(spec: &str, head: &str) -> bool {
    let spec = spec.trim().trim_start_matches('+');
    let Some((_, dst)) = spec.split_once(':').filter(|_| !spec.starts_with('^')) else { return false };
    if dst.is_empty() {
        return false;
    }
    let full = if dst.starts_with("refs/") {
        dst.to_owned()
    } else if ["heads/", "tags/", "remotes/"].iter().any(|p| dst.starts_with(p)) {
        format!("refs/{dst}")
    } else {
        format!("refs/heads/{dst}")
    };
    let (full, head) = (full.to_ascii_lowercase(), head.to_ascii_lowercase());
    match full.split('*').collect::<Vec<_>>().as_slice() {
        [exact] => *exact == head,
        [pre, post] => head.len() >= pre.len() + post.len() && head.starts_with(pre) && head.ends_with(post),
        _ => true,
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

/// A value as git reads a boolean: Some(true), Some(false), or None when it isn't one.
fn boolean(v: &str) -> Option<bool> {
    match v.to_ascii_lowercase().as_str() {
        "true" | "yes" | "on" => Some(true),
        "false" | "no" | "off" | "" => Some(false),
        n => n.parse::<i64>().ok().map(|n| n != 0),
    }
}

/// git's refusal of `value` for `key`.
fn invalid(key: &str, value: &str) -> String {
    format!("fatal: invalid value for '{key}': '{value}'")
}

/// `pull.ff` as the merge option it means; `Err` for a value git refuses.
fn configured_ff(config: &git2::Config) -> Result<Option<String>, String> {
    let Ok(v) = config.get_string("pull.ff") else { return Ok(None) };
    let ff = match (v.eq_ignore_ascii_case("only"), boolean(&v)) {
        (true, _) => "--ff-only",
        (_, Some(false)) => "--no-ff",
        (_, Some(true)) => "--ff",
        (_, None) => return Err(invalid("pull.ff", &v)),
    };
    Ok(Some(ff.to_owned()))
}

/// A rebase setting (`true`, `merges`, `interactive`, `false`) as how the rebase runs; None
/// for a merge. `Err` for a value git refuses (`preserve` is gone since git 2.34).
fn rebase_kind(how: &str, key: &str) -> Result<Option<&'static str>, String> {
    match (how, boolean(how)) {
        (_, Some(false)) => Ok(None),
        (_, Some(true)) => Ok(Some("")),
        ("merges" | "m", _) => Ok(Some("--rebase-merges")),
        ("interactive" | "i", _) => Ok(Some("-i")),
        _ => Err(invalid(key, how)),
    }
}

/// The commits the last fetch in `repo` marked for merging, from FETCH_HEAD, with their lines.
/// `Err` when it can't be read or a line doesn't parse.
pub fn fetched(repo: &Repository) -> Result<Vec<Fetched>, String> {
    let path = repo.path().join("FETCH_HEAD");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("Couldn't read {}: {e}", path.display()))?;
    fetched_in(&text).map_err(|line| format!("Couldn't read {}: {line}", path.display()))
}

/// The commits FETCH_HEAD's `text` marks for merging; `Err` with the first line that doesn't
/// parse.
pub(crate) fn fetched_in(text: &str) -> Result<Vec<Fetched>, &str> {
    let mut out = Vec::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let mut parts = line.splitn(3, '\t');
        let (oid, mark) = (parts.next().unwrap_or_default(), parts.next().unwrap_or_default());
        let oid = Oid::from_str(oid).map_err(|_| line)?;
        if mark.is_empty() {
            out.push(Fetched { oid, line: line.to_owned() });
        }
    }
    Ok(out)
}

#[path = "pull_args.rs"]
mod args;
#[path = "pull_run.rs"]
mod run;
pub use run::{Git, PullRun, run_pinned, run_split};

#[cfg(test)]
#[path = "pull_tests.rs"]
mod tests;
