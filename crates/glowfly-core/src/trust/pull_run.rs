//! Running a pull pinned to its one fetch (pull.rs): the fetch, the comparison of what it
//! brought (compare.rs, as the merge or rebase that follows would take it), then that merge or
//! rebase of exactly those commits, with `GIT_REFLOG_ACTION` as git pull sets it. A pull that
//! updates submodules, stores into the branch checked out, or whose fetch moved HEAD is held.

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
    let before = if follows.is_empty() {
        None
    } else if pull.recurses(&config) && !pull.fetch_only() {
        Some("git pull --recurse-submodules")
    } else if pull.stores_into_head(&repo) {
        Some("git pull into the branch checked out")
    } else {
        None
    };
    if let Some(what) = before {
        let u = Unchecked::Unmodeled(what.to_owned());
        if accept != Some(&u) {
            return refused(Refused::Unchecked(u), output);
        }
        let ok = shown(&pull.original(), &mut output, git);
        return done(ok, output);
    }
    // As git does before it fetches: a rebasing pull refuses local changes.
    match pull.needs_clean_tree(&repo) {
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
                    return failed(&format!("error: cannot pull with rebase: {why}\nerror: Please commit or stash them."), output);
                }
            }
        }
        Ok(false) => {}
    }
    let head_before = repo.head().ok().and_then(|h| h.target());
    if !shown(&pull.fetch, &mut output, git) {
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
    if !follows.is_empty()
        && repo.head().ok().and_then(|h| h.target()) != head_before
        && let Err(r) = skip_accepted::<()>(Err(Unchecked::Unmodeled("a fetch that moved the branch checked out".to_owned())), accept)
    {
        return refused(r, output);
    }
    let mut heads = match fetched(&repo) {
        Ok(h) => h,
        Err(e) => return refused(unreadable(&e), output),
    };
    // A tag fetched is merged as its commit.
    for h in &mut heads {
        match crate::trust::plumbing::resolve(&repo, &h.oid.to_string(), "commit") {
            Ok(c) => h.oid = c,
            Err(u) => return refused(Refused::Unchecked(u), output),
        }
    }
    let plan = match pull.plan(&repo, &heads) {
        Ok(p) => p,
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
