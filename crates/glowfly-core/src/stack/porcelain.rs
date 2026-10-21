//! The command lines of a stack push and the reading of `git push --porcelain`'s answer. Pure.

use super::push::{PushPlan, PushTarget};
use crate::gitcli::args;

/// The pushes for `layers` (index, target, plan), one per (remote, set_upstream) in first-seen
/// order: `push --porcelain [-u] [--force-with-lease=refs/heads/<dest>:<seen>]... <remote>
/// refs/heads/<branch>:refs/heads/<dest>...`, with the indices each carries. Only plans that
/// push are included, and only Rewritten layers get a lease, so the others keep git's
/// fast-forward check. `-u` goes only on a group whose layers all track nothing.
pub fn commands(layers: &[(usize, &PushTarget, PushPlan)]) -> Vec<(Vec<String>, Vec<usize>)> {
    type Group<'a> = (&'a str, bool, Vec<(usize, &'a PushTarget, PushPlan)>);
    let mut groups: Vec<Group> = Vec::new();
    for &(i, t, plan) in layers.iter().filter(|(_, _, p)| p.pushes()) {
        match groups.iter_mut().find(|(r, u, _)| *r == t.remote && *u == t.set_upstream) {
            Some((_, _, members)) => members.push((i, t, plan)),
            None => groups.push((&t.remote, t.set_upstream, vec![(i, t, plan)])),
        }
    }
    groups
        .into_iter()
        .map(|(remote, upstream, members)| {
            let mut cmd = args(&["push", "--porcelain"]);
            if upstream {
                cmd.push("-u".into());
            }
            for (_, t, plan) in &members {
                if let (PushPlan::Rewritten, Some(seen)) = (plan, t.seen) {
                    cmd.push(format!("--force-with-lease=refs/heads/{}:{seen}", t.dest));
                }
            }
            cmd.push(remote.to_owned());
            cmd.extend(members.iter().map(|(_, t, _)| format!("refs/heads/{}:refs/heads/{}", t.branch, t.dest)));
            (cmd, members.iter().map(|(i, _, _)| *i).collect())
        })
        .collect()
}

/// (flag, destination ref, summary) of each ref line of `git push --porcelain`'s stdout:
/// `<flag>\t<from>:<to>\t<summary>`. Every other line ("To <url>", "Done", upstream notes) is
/// skipped.
pub fn parse(stdout: &str) -> Vec<(char, String, String)> {
    stdout
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\t');
            let (flag, refs, summary) = (parts.next()?, parts.next()?, parts.next().unwrap_or(""));
            let mut chars = flag.chars();
            let f = chars.next().filter(|c| " +-*!=".contains(*c) && chars.next().is_none())?;
            let to = refs.rsplit(':').next().unwrap_or(refs);
            Some((f, to.to_owned(), summary.trim().to_owned()))
        })
        .collect()
}
