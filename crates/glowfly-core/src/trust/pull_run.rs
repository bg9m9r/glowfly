//! Running a pull pinned to its one fetch (pull.rs): the fetch, the comparison of what it
//! brought (compare.rs, as the merge or rebase that follows would take it), then that merge or
//! rebase of exactly those commits, with `GIT_REFLOG_ACTION` as git pull sets it. A pull that
//! updates submodules, may store into the branch checked out (judged before the fetch, erring
//! towards yes, and refused by the fetch itself, which runs without `--update-head-ok`) or
//! pulls into an unborn branch is held. The split never updates the working tree itself: a
//! branch that moved anyway ends the pull there, with git's own words for a working tree that
//! couldn't follow it.
//!
//! `run_split` is the same split for a pull with nothing to compare (a job that can be stopped
//! in its fetch): whatever the split doesn't model, or any way it would fail other than git's
//! own commands failing (divergent branches, no upstream, an upstream that is gone), runs as
//! `git pull` itself, so the outcome and git's words are always `git pull`'s.

use super::{Plan, Pull, fetched};
use crate::settings::SavedTrust;
use crate::trust::Follow;
use crate::trust::compare::{Refused, check_targets, skip_accepted};
use crate::trust::incoming::{Target, Unchecked};
use git2::Repository;
use std::path::Path;

/// What a pinned pull did.
#[derive(Debug, PartialEq)]
pub enum PullRun {
    /// It ran to the end or to git's failure: whether it succeeded, and what git printed.
    Ran { ok: bool, output: String },
    /// Only the fetch ran, if that: what it brought isn't covered, or couldn't be compared.
    Refused { refused: Refused, output: String },
}

/// Runs one git command (its arguments, standard input and extra environment): whether it
/// succeeded, and what it printed.
pub type Git<'a> = dyn FnMut(&[String], &str, &[(&str, &str)]) -> (bool, String) + 'a;

/// Runs `pull` in the repository at `path`, pinned: its fetch, then a comparison with `saved`,
/// where `follows` points, of the commits FETCH_HEAD marks for merging as the merge or rebase
/// would take them (a rebase's local commits, what each merge leaves), then that merge or
/// rebase of exactly those commits. `accept`: the one reason an item couldn't be compared
/// that the person chose to run anyway (the others are still compared; one found before the
/// fetch runs the pull as it is). `git` runs one git command. The output lists the fetch and
/// the merge or rebase as `$ git ...` lines.
pub fn run_pinned(
    path: &Path,
    pull: &Pull,
    follows: &[Follow],
    saved: Option<&SavedTrust>,
    accept: Option<&Unchecked>,
    git: &mut Git<'_>,
) -> PullRun {
    run(path, pull, follows, saved, accept, git, false)
}

/// Runs `pull` as its fetch and then its merge or rebase, with nothing to compare, ending as
/// `git pull` would: a pull that updates submodules, may store into the branch checked out or
/// pulls into an unborn branch runs whole, and so does one the split would refuse after its fetch (git pull's second fetch
/// then finds nothing new). Never `PullRun::Refused` but for a repository that can't be opened.
pub fn run_split(path: &Path, pull: &Pull, git: &mut Git<'_>) -> PullRun {
    run(path, pull, &[], None, None, git, true)
}

fn run(
    path: &Path,
    pull: &Pull,
    follows: &[Follow],
    saved: Option<&SavedTrust>,
    accept: Option<&Unchecked>,
    git: &mut Git<'_>,
    whole: bool,
) -> PullRun {
    let mut output = String::new();
    let reflog = [("GIT_REFLOG_ACTION", pull.reflog.as_str())];
    let shown = |cmd: &[String], output: &mut String, git: &mut Git<'_>| {
        output.push_str(&format!("$ git {}\n", crate::text::redact_credentials(&cmd.join(" "))));
        let (ok, text) = git(cmd, "", &reflog);
        if !text.is_empty() {
            output.push_str(text.trim_end());
            output.push('\n');
        }
        ok
    };
    let done = |ok, output: String| PullRun::Ran { ok, output: output.trim_end().to_owned() };
    let failed = |why: &str, mut output: String| {
        output.push_str(why);
        PullRun::Ran { ok: false, output: output.trim_end().to_owned() }
    };
    let refused = |r, output: String| PullRun::Refused { refused: r, output: output.trim_end().to_owned() };
    // The pull as git runs it, after what ran so far.
    let pull_whole = |mut output: String, git: &mut Git<'_>| {
        let ok = shown(&pull.original(), &mut output, git);
        done(ok, output)
    };
    // The split can't go on as git pull would: `run_split` has git pull do it.
    let as_git = |r: Refused, output: String, git: &mut Git<'_>| if whole { pull_whole(output, git) } else { refused(r, output) };
    let unreadable = |e: &str| Refused::Unchecked(Unchecked::Unreadable(e.to_owned()));
    let repo = match Repository::open(path) {
        Ok(r) => r,
        Err(e) => return refused(unreadable(e.message()), output),
    };
    let config = match repo.config().and_then(|mut c| c.snapshot()) {
        Ok(c) => c,
        Err(e) => return refused(unreadable(e.message()), output),
    };
    // What the pinned pull doesn't do as git pull does: held, or with the person's choice run
    // as git pull itself.
    let before = if follows.is_empty() && !whole {
        None
    } else if pull.recurses(&config) && !pull.fetch_only() {
        Some("git pull --recurse-submodules")
    } else if repo.head().is_err() {
        // git pull checks out into an unborn branch its own way (whatever --ff or --squash say).
        Some("git pull into a branch with no commits yet")
    } else if pull.stores_into_head(&repo) {
        Some("git pull into the branch checked out")
    } else {
        None
    };
    if let Some(what) = before {
        let u = Unchecked::Unmodeled(what.to_owned());
        if accept != Some(&u) && !whole {
            return refused(Refused::Unchecked(u), output);
        }
        let ok = shown(&pull.original(), &mut output, git);
        return done(ok, output);
    }
    // As git does before it fetches: a rebasing pull refuses local changes.
    match pull.needs_clean_tree(&repo) {
        Err(_) if whole => return pull_whole(output, git),
        Err(why) => return failed(&why, output),
        Ok(true) => {
            let _ = git(&crate::gitcli::args(&["update-index", "-q", "--ignore-submodules", "--refresh"]), "", &[]);
            let checks = [
                (crate::gitcli::args(&["diff-files", "--quiet", "--ignore-submodules"]), "You have unstaged changes."),
                (
                    crate::gitcli::args(&["diff-index", "--cached", "--quiet", "--ignore-submodules", "HEAD", "--"]),
                    "Your index contains uncommitted changes.",
                ),
            ];
            for (cmd, why) in checks {
                if !git(&cmd, "", &[]).0 {
                    // git pull refuses the same way, before it fetches.
                    if whole {
                        return pull_whole(output, git);
                    }
                    return failed(&format!("error: cannot pull with rebase: {why}\nerror: Please commit or stash them."), output);
                }
            }
        }
        Ok(false) => {}
    }
    let head_before = repo.head().ok().and_then(|h| h.target());
    let fetch_from = output.len();
    if !shown(&pull.fetch, &mut output, git) {
        // Without --update-head-ok git refuses a fetch that would store into a branch checked
        // out (here or in another worktree) before it writes any ref: what the split doesn't
        // model, whatever refspec, --refmap, --all or -c made it so.
        // git's own line only: a remote's `remote:` lines can say anything.
        if output[fetch_from..].lines().any(|l| l.to_ascii_lowercase().starts_with("fatal: refusing to fetch into")) {
            let u = Unchecked::Unmodeled("git pull into the branch checked out".to_owned());
            if whole || accept == Some(&u) {
                return pull_whole(output, git);
            }
            return refused(Refused::Unchecked(u), output);
        }
        return done(false, output);
    }
    if pull.fetch_only() {
        return done(true, output);
    }
    // Reopened: the fetch added objects and references this process hasn't seen.
    let repo = match Repository::open(path) {
        Ok(r) => r,
        Err(e) => return refused(unreadable(e.message()), output),
    };
    let head_after = repo.head().ok().and_then(|h| h.target());
    // The branch checked out moved all the same (something else moved it while the pull ran):
    // the branch has moved and the working tree hasn't. That is left as it is, with git's words
    // for it (git_errors::Cause::FetchMovedHead says what to do), rather than brought along.
    if let Some(from) = head_before
        && head_after != head_before
    {
        let why = format!(
            "warning: fetch updated the current branch head.\nfatal: Cannot fast-forward your working tree.\nAfter making sure that you saved anything precious from\n$ git diff {from}\noutput, run\n$ git reset --hard\nto recover."
        );
        return failed(&why, output);
    }
    let mut heads = match fetched(&repo) {
        Ok(h) => h,
        Err(e) => return as_git(unreadable(&e), output, git),
    };
    // A tag fetched is merged as its commit.
    for h in &mut heads {
        match crate::trust::plumbing::resolve(&repo, &h.oid.to_string(), "commit") {
            Ok(c) => h.oid = c,
            Err(u) => return as_git(Refused::Unchecked(u), output, git),
        }
    }
    let plan = match pull.plan(&repo, &heads) {
        Ok(p) => p,
        // git pull says it its own way (divergent branches, no upstream, an upstream gone).
        Err(_) if whole => return pull_whole(output, git),
        Err(why) => return failed(&why, output),
    };
    let how = pull.how();
    let fork_point = pull.fork_ref(&repo).filter(|_| matches!(plan, Plan::Rebase(_))).and_then(|r| {
        let (ok, out) = git(&crate::gitcli::args(&["merge-base", "--fork-point", &r, "HEAD"]), "", &[]);
        ok.then(|| git2::Oid::from_str(out.trim()).ok()).flatten()
    });
    let (targets, kind, stash_key) = match &plan {
        Plan::Merge(_) => (vec![Target::Merged { revs: vec!["FETCH_HEAD".to_owned()], how: how.clone() }], "", "merge.autoStash"),
        Plan::FastForward => (vec![Target::Commit(heads[0].oid)], "", "rebase.autoStash"),
        Plan::Rebase(kind) => {
            let onto = heads[0].oid.to_string();
            let upstream = fork_point.map_or(onto.clone(), |f| f.to_string());
            let replay = Target::Replayed {
                onto: Some(onto),
                upstream: Some(upstream),
                branch: None,
                root: false,
                keep_base: false,
                how: how.clone(),
            };
            (vec![replay], *kind, "rebase.autoStash")
        }
    };
    let autostash = pull.autostash(&config, stash_key);
    let mut found = check_targets(&repo, follows, saved, &targets, accept);
    if found.is_ok() && !kind.is_empty() && !follows.is_empty() {
        found = skip_accepted(Err(Unchecked::Unmodeled(format!("git pull --rebase={}", kind.trim_start_matches('-')))), accept);
    }
    if found.is_ok() && autostash {
        found = skip_accepted(crate::trust::compare::autostash(&repo, follows), accept);
    }
    if let Err(r) = found {
        return refused(r, output);
    }
    let cmd = pull.command(&plan, &heads, fork_point, autostash && matches!(plan, Plan::FastForward));
    let ok = shown(&cmd, &mut output, git);
    done(ok, output)
}
