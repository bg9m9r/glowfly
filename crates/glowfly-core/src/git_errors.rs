//! Reading why a git command failed, from its untranslated output, for the failure toast; and
//! keeping git's internal patch warnings out of job output. Pure: no I/O.

use crate::auth_failure::{self, AuthNeed};
use std::borrow::Cow;

/// Why a git command failed, as far as its output tells.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cause {
    /// Uncommitted changes would be overwritten; `files` is empty when git lists none.
    LocalChanges {
        files: Vec<String>,
    },
    /// Untracked files would be overwritten or removed.
    UntrackedInTheWay {
        files: Vec<String>,
    },
    /// The operation stopped on conflicts in `files` (empty when git names none), and in a
    /// path for each of `unsure`, whose readings (`conflict_lines`) leave open which it is.
    Conflicts {
        files: Vec<String>,
        unsure: Vec<Vec<String>>,
    },
    /// Conflicts from earlier aren't resolved yet.
    Unresolved,
    /// A merge is still in progress (MERGE_HEAD exists).
    MergeInProgress,
    /// A push of `branch` was rejected because the remote has commits it lacks.
    PushBehind {
        branch: String,
    },
    /// A force push with lease of `branch` was refused: the remote branch moved.
    StaleLease {
        branch: String,
    },
    /// The remote (or git) refused the push for `reason`, git's text in brackets.
    PushRefused {
        reason: String,
    },
    /// Push's "The current branch X has no upstream branch" (`Some(X)`), or pull's "no tracking
    /// information" (None).
    NoUpstream {
        push_branch: Option<String>,
    },
    /// The upstream `branch` (short name) is no longer on the remote.
    UpstreamGone {
        branch: String,
    },
    /// A fast-forward isn't possible: a pull (None) or a fetch into the branch `Some(dst)`.
    Diverged {
        branch: Option<String>,
    },
    /// Pull or push in detached HEAD.
    DetachedHead,
    /// git can't find the ref, revision or path `name`.
    UnknownRef {
        name: String,
    },
    /// A lock file (its file name, e.g. "index.lock") exists.
    Locked {
        file: String,
    },
    SignIn(AuthNeed),
    /// git couldn't reach or read the remote.
    Remote(crate::remote_failure::RemoteFailure),
    /// ssh reports that the server's host key changed.
    HostKeyChanged,
    /// Only "Merge with strategy <s> failed." was recognised.
    MergeRefused,
}

/// Why `output` (a failed job's combined output, untranslated) says git failed, when it is a
/// cause Glowfly can explain. `$ git …` echo lines and `hint:` lines are never matched.
pub fn classify(output: &str) -> Option<Cause> {
    let lines: Vec<&str> = meaningful(output).collect();
    let text = lines.join("\n");
    let lower: Vec<String> = lines.iter().map(|l| l.to_lowercase()).collect();
    if let Some(file) = lines.iter().find_map(|l| locked(l)) {
        return Some(Cause::Locked { file });
    }
    if auth_failure::host_key_changed(&text) {
        return Some(Cause::HostKeyChanged);
    }
    if let Some(need) = auth_failure::need(&text) {
        return Some(Cause::SignIn(need));
    }
    if let Some(f) = crate::remote_failure::read(&text) {
        return Some(Cause::Remote(f));
    }
    let untracked = |l: &str| {
        l.contains("untracked working tree files would be overwritten by") || l.contains("untracked working tree files would be removed by")
    };
    if lower.iter().any(|l| untracked(l)) {
        return Some(Cause::UntrackedInTheWay { files: listed(&lines, untracked) });
    }
    if let Some(cause) = local_changes(&lines, &lower) {
        return Some(cause);
    }
    let unresolved = [
        "is not possible because you have unmerged files",
        "you need to resolve your current index first",
        "because of an unresolved conflict",
    ];
    if lower.iter().any(|l| unresolved.iter().any(|m| l.contains(m))) {
        return Some(Cause::Unresolved);
    }
    if lines.iter().any(|l| l.contains("You have not concluded your merge")) {
        return Some(Cause::MergeInProgress);
    }
    if lines.iter().any(|l| conflict_line(l)) {
        let (files, unsure) = crate::conflict_lines::conflict_paths(&lines);
        return Some(Cause::Conflicts { files, unsure });
    }
    if let Some(cause) = rejected(&lines) {
        return Some(cause);
    }
    if let Some(cause) = upstream(&lines) {
        return Some(cause);
    }
    if lower.iter().any(|l| diverged_pull(l)) {
        return Some(Cause::Diverged { branch: None });
    }
    if lines.iter().any(|l| l.contains("You are not currently on a branch.")) {
        return Some(Cause::DetachedHead);
    }
    if let Some(name) = lines.iter().find_map(|l| unknown_ref(l)) {
        return Some(Cause::UnknownRef { name });
    }
    lines.iter().any(|l| l.starts_with("Merge with strategy ") && l.trim_end().ends_with(" failed.")).then_some(Cause::MergeRefused)
}

/// `args` (a git command line without "git") with `-c apply.whitespace=nowarn` in front when
/// its subcommand is merge, pull (not rebasing) or stash: merge's state restore and
/// `stash … --index` feed `git apply` a patch on stdin, whose whitespace warnings name
/// `<stdin>` and mean nothing in a job log. Left off rebasing pulls, as `apply.whitespace` also
/// governs `git am` (an apply-backend rebase); a plain pull that rebases by config still gets
/// it, so for that one pull an `apply.whitespace` of fix or error acts as nowarn. The
/// command-line value reaches git's child processes and beats the user's config.
pub fn with_quiet_apply(args: &[String]) -> Cow<'_, [String]> {
    let Some(i) = subcommand(args) else { return Cow::Borrowed(args) };
    let rest = &args[i + 1..];
    let rebasing = |a: &String| a == "-r" || a == "--rebase" || (a.starts_with("--rebase=") && a != "--rebase=false");
    let quiet = match args[i].as_str() {
        "merge" | "stash" => true,
        "pull" => !rest.iter().any(rebasing),
        _ => false,
    };
    if !quiet {
        return Cow::Borrowed(args);
    }
    let mut out = vec!["-c".to_owned(), "apply.whitespace=nowarn".to_owned()];
    out.extend_from_slice(args);
    Cow::Owned(out)
}

/// The index of the subcommand in `args`, past git's global options (`-c` and `-C` take a value).
fn subcommand(args: &[String]) -> Option<usize> {
    let mut i = 0;
    while let Some(a) = args.get(i) {
        match a.as_str() {
            "-c" | "-C" => i += 2,
            a if a.starts_with('-') => i += 1,
            _ => return Some(i),
        }
    }
    None
}

/// The lines worth matching: no `$ git …` echoes and no hints.
fn meaningful(output: &str) -> impl Iterator<Item = &str> {
    output.lines().filter(|l| !l.starts_with("$ ") && !l.starts_with("hint:"))
}

/// The files listed under the first line `header` accepts (given in lowercase) that ends in
/// ':': the indented lines after it, up to the first line that isn't indented.
fn listed(lines: &[&str], header: impl Fn(&str) -> bool) -> Vec<String> {
    let Some(at) = lines.iter().position(|l| l.trim_end().ends_with(':') && header(&l.to_lowercase())) else { return Vec::new() };
    lines[at + 1..]
        .iter()
        .take_while(|l| l.starts_with([' ', '\t']) && !l.trim().is_empty())
        .map(|l| {
            let p = l.trim();
            if p.starts_with('"') { crate::quoted::unquote_c(p) } else { p.to_owned() }
        })
        .collect()
}

/// Uncommitted changes in the way, listed or not.
fn local_changes(lines: &[&str], lower: &[String]) -> Option<Cause> {
    let header = |l: &str| l.contains("your local changes to the following files would be overwritten by");
    if lower.iter().any(|l| header(l)) {
        return Some(Cause::LocalChanges { files: listed(lines, header) });
    }
    let unlisted = |l: &String| {
        l.contains("your local changes would be overwritten by")
            || ((l.contains("cannot pull with rebase:") || l.contains("cannot rebase:"))
                && (l.contains("unstaged changes") || l.contains("uncommitted changes")))
    };
    lower.iter().any(unlisted).then(|| Cause::LocalChanges { files: Vec::new() })
}

/// A line that says the operation stopped on conflicts.
fn conflict_line(l: &str) -> bool {
    l.starts_with("CONFLICT (")
        || l.contains("Automatic merge failed; fix conflicts")
        // A rebase prints its progress ("Rebasing (1/1)") in front, on the same line.
        || l.contains("error: could not apply ")
        || l.contains("error: could not revert ")
}

/// The text between `prefix` and the next `'` in `line`.
fn quoted_after<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
    let rest = &line[line.find(prefix)? + prefix.len()..];
    rest.split_once('\'').map(|(q, _)| q)
}

/// The lock file's name in "Unable to create '<path>.lock': File exists." A server's own
/// `remote:` line names a lock on the server, never one here.
fn locked(line: &str) -> Option<String> {
    if from_remote(line) || !line.contains(".lock': File exists") {
        return None;
    }
    let path = quoted_after(line, "Unable to create '")?;
    path.rsplit(['/', '\\']).next().filter(|f| !f.is_empty()).map(str::to_owned)
}

/// The lock file's path exactly as git printed it in "Unable to create '<path>': File exists."
/// (the first such line of `output`, `remote:` lines left out), for Delete lock file. Read up to
/// the last `': File exists`, so a path holding a `'` stays whole.
pub fn lock_path(output: &str) -> Option<String> {
    meaningful(output).filter(|l| !from_remote(l)).find_map(|l| {
        let (_, rest) = l.split_once("Unable to create '")?;
        let (path, _) = rest.rsplit_once("': File exists")?;
        path.ends_with(".lock").then(|| path.to_owned())
    })
}

/// Whether `line` is the server's, relayed by git as `remote: …`.
fn from_remote(line: &str) -> bool {
    line.trim_start().starts_with("remote:")
}

/// The `(dst, reason, remote)` of each "! [rejected]" or "! [remote rejected]" line (`remote`:
/// the remote's own refusal).
fn rejections<'a>(lines: &[&'a str]) -> Vec<(&'a str, &'a str, bool)> {
    let mut out = Vec::new();
    for l in lines {
        let t = l.trim();
        let (rest, remote) = if let Some(r) = t.strip_prefix("! [rejected]") {
            (r, false)
        } else if let Some(r) = t.strip_prefix("! [remote rejected]") {
            (r, true)
        } else {
            continue;
        };
        let Some((_, to)) = rest.split_once(" -> ") else { continue };
        let (dst, reason) = match to.split_once('(') {
            Some((d, r)) => (d.trim(), r.rsplit_once(')').map_or(r, |(r, _)| r).trim()),
            None => (to.trim(), ""),
        };
        out.push((dst, reason, remote));
    }
    out
}

/// A push or fetch rejection: pushes say "failed to push some refs".
fn rejected(lines: &[&str]) -> Option<Cause> {
    let (dst, reason, remote) = *rejections(lines).first()?;
    let pushing = lines.iter().any(|l| l.contains("failed to push some refs"));
    let branch = dst.to_owned();
    match (remote, reason) {
        (false, "stale info") => Some(Cause::StaleLease { branch }),
        (false, "fetch first" | "non-fast-forward") if pushing => Some(Cause::PushBehind { branch }),
        (false, "non-fast-forward") => Some(Cause::Diverged { branch: Some(branch) }),
        _ if pushing || remote => Some(Cause::PushRefused { reason: reason.to_owned() }),
        _ => None,
    }
}

/// No upstream to push to or pull from, or one deleted on the remote.
fn upstream(lines: &[&str]) -> Option<Cause> {
    for l in lines {
        if let Some((_, rest)) = l.split_once("The current branch ")
            && let Some(name) = rest.trim_end().strip_suffix(" has no upstream branch.")
        {
            return Some(Cause::NoUpstream { push_branch: Some(name.to_owned()) });
        }
    }
    if lines.iter().any(|l| l.contains("There is no tracking information for the current branch")) {
        return Some(Cause::NoUpstream { push_branch: None });
    }
    let gone = lines.iter().find_map(|l| quoted_after(l, "Your configuration specifies to merge with the ref '"));
    match gone {
        Some(r) if lines.iter().any(|l| l.contains("no such ref was fetched")) => {
            Some(Cause::UpstreamGone { branch: r.strip_prefix("refs/heads/").unwrap_or(r).to_owned() })
        }
        _ => None,
    }
}

/// A pull (`line` in lowercase) that stopped because the branch and its upstream both have
/// commits: "not possible to fast-forward" (`--ff-only` or `pull.ff=only`, with or without
/// ", aborting"), or "need to specify how to reconcile divergent branches" (git 2.33 and later,
/// when no pull mode is configured).
fn diverged_pull(line: &str) -> bool {
    line.contains("not possible to fast-forward") || line.contains("how to reconcile divergent branches")
}

/// The name in git's ways of saying it can't find a ref, revision or path.
fn unknown_ref(l: &str) -> Option<String> {
    let name = if l.contains("did not match any file(s) known to git") {
        quoted_after(l, "pathspec '")
    } else if let Some((before, _)) = l.split_once(" - not something we can merge") {
        Some(before.rsplit_once(": ").map_or(before, |(_, n)| n))
    } else if let Some((_, rest)) = l.split_once("couldn't find remote ref ") {
        Some(rest)
    } else if let Some((_, rest)) = l.split_once("src refspec ") {
        rest.split_once(" does not match any").map(|(n, _)| n)
    } else if let Some((before, _)) = l.split_once("' is not a commit and a branch") {
        before.split_once('\'').map(|(_, n)| n)
    } else if l.contains("unknown revision") {
        quoted_after(l, "ambiguous argument '")
    } else if l.contains("bad revision '") {
        quoted_after(l, "bad revision '")
    } else if l.contains("invalid upstream '") {
        quoted_after(l, "invalid upstream '")
    } else {
        l.split_once("invalid reference: ").map(|(_, n)| n)
    };
    name.map(str::trim).filter(|n| !n.is_empty()).map(str::to_owned)
}

#[cfg(test)]
#[path = "git_errors_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "git_errors_fixture_tests.rs"]
mod fixture_tests;
