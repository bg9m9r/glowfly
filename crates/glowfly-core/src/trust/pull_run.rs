//! Running a pull pinned to its one fetch (pull.rs): the fetch, the comparison of what it
//! brought, then the merge or rebase of exactly those commits.

use super::{Plan, Pull, fetched};
use crate::settings::SavedTrust;
use crate::trust::Follow;
use crate::trust::compare::Refused;
use crate::trust::incoming::Unchecked;
use git2::{Oid, Repository};
use std::path::Path;

/// What a pinned pull did.
#[derive(Debug, PartialEq)]
pub enum PullRun {
    /// It ran to the end or to git's failure: whether it succeeded, and what git printed.
    Ran { ok: bool, output: String },
    /// Only the fetch ran: what it brought isn't covered, or couldn't be compared.
    Refused { refused: Refused, output: String },
}

/// Runs one git command (its arguments and standard input): whether it succeeded, and what it
/// printed.
pub type Git<'a> = dyn FnMut(&[String], &str) -> (bool, String) + 'a;

/// Runs `pull` in the repository at `path`, pinned: its fetch, then a comparison of the commits
/// FETCH_HEAD marks for merging (for a rebase, the local commits it replays too; for a merge,
/// what merging them leaves) with `saved` where `follows` points, then the merge or rebase of
/// exactly those commits. `accept`: what couldn't be compared that the user chose to run
/// anyway (this reason only). `git` runs one git command (its arguments and standard input)
/// and says whether it succeeded and what it printed. The output lists the fetch and the merge
/// or rebase as `$ git ...` lines.
pub fn run_pinned(
    path: &Path,
    pull: &Pull,
    follows: &[Follow],
    saved: Option<&SavedTrust>,
    accept: Option<&Unchecked>,
    git: &mut Git<'_>,
) -> PullRun {
    let mut output = String::new();
    let shown = |cmd: &[String], output: &mut String, git: &mut Git<'_>| {
        output.push_str(&format!("$ git {}\n", crate::text::redact_credentials(&cmd.join(" "))));
        let (ok, text) = git(cmd, "");
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
    // As git does before it fetches: a rebasing pull refuses local changes.
    if Repository::open(path).is_ok_and(|r| pull.needs_clean_tree(&r)) {
        let _ = git(&crate::gitcli::args(&["update-index", "-q", "--ignore-submodules", "--refresh"]), "");
        let checks = [
            (crate::gitcli::args(&["diff-files", "--quiet", "--ignore-submodules"]), "You have unstaged changes."),
            (
                crate::gitcli::args(&["diff-index", "--cached", "--quiet", "--ignore-submodules", "HEAD", "--"]),
                "Your index contains uncommitted changes.",
            ),
        ];
        for (cmd, why) in checks {
            if !git(&cmd, "").0 {
                return failed(&format!("error: cannot pull with rebase: {why}\nerror: Please commit or stash them."), output);
            }
        }
    }
    if !shown(&pull.fetch, &mut output, git) {
        return done(false, output);
    }
    if pull.fetch_only() {
        return done(true, output);
    }
    let refused = |r, output: String| PullRun::Refused { refused: r, output: output.trim_end().to_owned() };
    let unreadable = |e: String| Refused::Unchecked(Unchecked::Unreadable(e));
    // Reopened: the fetch added objects and references this process hasn't seen.
    let repo = match Repository::open(path) {
        Ok(r) => r,
        Err(e) => return refused(unreadable(e.message().to_owned()), output),
    };
    let heads = match fetched(&repo) {
        Ok(h) => h,
        Err(e) => return refused(unreadable(e), output),
    };
    let plan = match pull.plan(&repo, &heads) {
        Ok(p) => p,
        Err(why) => return failed(&why, output),
    };
    let mut oids: Vec<Oid> = heads.iter().map(|h| h.oid).collect();
    let mut found = Ok(());
    if let (Plan::Rebase(_), Ok(head)) = (&plan, repo.head().and_then(|h| h.peel_to_commit())) {
        match local_commits(&repo, head.id(), &oids) {
            Ok(local) => oids.extend(local),
            Err(u) => found = Err(Refused::Unchecked(u)),
        }
    }
    let found = found.and_then(|()| crate::trust::compare::check(&repo, follows, saved, &oids)).and_then(|()| match plan {
        Plan::Merge(_) => {
            let head = repo.head().and_then(|h| h.peel_to_commit()).map(|c| c.id());
            let steps: Vec<_> = heads
                .iter()
                .map(|h| crate::trust::merged::Step {
                    base: head.as_ref().ok().and_then(|o| repo.merge_base(*o, h.oid).ok()),
                    theirs: h.oid,
                })
                .collect();
            crate::trust::merged::check(&repo, follows, saved, &steps)
        }
        _ => Ok(()),
    });
    match found {
        Err(Refused::Unchecked(u)) if accept == Some(&u) => {}
        Err(r) => return refused(r, output),
        Ok(()) => {}
    }
    let message = match plan {
        Plan::Merge(_) => {
            let lines: String = heads.iter().map(|h| format!("{}\n", h.line)).collect();
            let cmd = [pull.globals.clone(), crate::gitcli::args(&["fmt-merge-msg"]), pull.log.clone()].concat();
            match git(&cmd, &lines) {
                (true, m) => m,
                (false, why) => return failed(&format!("git fmt-merge-msg failed: {}", why.trim()), output),
            }
        }
        _ => String::new(),
    };
    let fork_point = pull.fork_ref(&repo).filter(|_| matches!(plan, Plan::Rebase(_))).and_then(|r| {
        let (ok, out) = git(&crate::gitcli::args(&["merge-base", "--fork-point", &r, "HEAD"]), "");
        ok.then(|| Oid::from_str(out.trim()).ok()).flatten()
    });
    let cmd = pull.command(&plan, &heads, &message, fork_point);
    let ok = shown(&cmd, &mut output, git);
    done(ok, output)
}

/// The local commits a rebase onto `onto` replays: HEAD's, not in any of them.
fn local_commits(repo: &Repository, head: Oid, onto: &[Oid]) -> Result<Vec<Oid>, Unchecked> {
    let err = |e: git2::Error| Unchecked::Unreadable(e.message().to_owned());
    let mut w = repo.revwalk().map_err(err)?;
    w.push(head).map_err(err)?;
    for o in onto {
        w.hide(*o).map_err(err)?;
    }
    crate::trust::incoming::listed(w)
}
