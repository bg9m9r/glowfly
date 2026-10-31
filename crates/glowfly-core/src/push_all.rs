//! Push to all remotes: one local branch pushed to every remote in turn (`plan`, `run`), each
//! push run even when one before it failed, never forced and never setting an upstream. The
//! upstream's remote gets the upstream's name; every other remote the branch's own name.

use crate::branch_ops::push_cmd;
use crate::gitcli;
use crate::repo::BranchInfo;
use std::path::Path;

/// One remote's push: the remote, the branch name there and the command.
#[derive(Clone, Debug, PartialEq)]
pub struct RemotePush {
    pub remote: String,
    pub dest: String,
    pub cmd: Vec<String>,
}

/// `b` pushed to each of `remotes`, in their order: to its upstream's name on the upstream's
/// remote (so the current upstream stays the one it tracks), under its own name elsewhere.
pub fn plan<'a>(b: &BranchInfo, remotes: impl IntoIterator<Item = &'a str>) -> Vec<RemotePush> {
    let tracked = b.tracking.as_ref().filter(|t| t.is_remote());
    remotes
        .into_iter()
        .map(|remote| {
            let dest = tracked.filter(|t| t.remote == remote).map_or_else(|| b.name.clone(), |t| t.merge.clone());
            let cmd = push_cmd(&b.name, remote, &dest, false);
            RemotePush { remote: remote.to_owned(), dest, cmd }
        })
        .collect()
}

/// What became of a push to all remotes: the remotes whose push failed (in push order) and
/// every command's echo and output.
#[derive(Clone, Debug, PartialEq)]
pub struct PushedAll {
    pub failed: Vec<String>,
    pub output: String,
}

/// Runs each of `pushes` in turn with the extra environment `env`, going on past a failure.
pub fn run(workdir: &Path, pushes: &[RemotePush], env: &[(&str, &str)]) -> PushedAll {
    let mut output = String::new();
    let mut failed = Vec::new();
    for p in pushes {
        output.push_str(&format!("$ git {}\n", crate::text::redact_credentials(&p.cmd.join(" "))));
        let (ok, text) = gitcli::run_with_input(workdir, &p.cmd, "", env);
        if !text.trim().is_empty() {
            output.push_str(text.trim_end());
            output.push('\n');
        }
        if !ok {
            failed.push(p.remote.clone());
        }
    }
    PushedAll { failed, output: output.trim_end().to_owned() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gitcli::args;
    use crate::repo::Tracking;
    use crate::testutil::{git, scratch_repo};

    fn branch(name: &str, upstream: Option<(&str, &str)>) -> BranchInfo {
        let tracking = upstream.map(|(remote, merge)| Tracking { remote: remote.into(), merge: merge.into() });
        let (ahead, behind) = (0, 0);
        BranchInfo {
            name: name.into(),
            oid: None,
            is_head: false,
            upstream: None,
            ahead,
            behind,
            tracking,
            upstream_ref: None,
            merged: false,
        }
    }

    #[test]
    fn the_upstream_remote_gets_the_upstream_name_and_no_push_sets_an_upstream() {
        let p = plan(&branch("topic", Some(("origin", "feature/topic"))), ["origin", "fork"]);
        assert_eq!(p[0].cmd, args(&["push", "--", "origin", "refs/heads/topic:refs/heads/feature/topic"]));
        assert_eq!(p[1].cmd, args(&["push", "--", "fork", "refs/heads/topic:refs/heads/topic"]));
        let none = plan(&branch("topic", None), ["origin"]);
        assert_eq!(none[0].cmd, args(&["push", "--", "origin", "refs/heads/topic:refs/heads/topic"]));
    }

    #[test]
    fn a_failing_remote_does_not_stop_the_others() {
        let work = scratch_repo("push-all-run");
        let bare = work.with_file_name(format!("{}-a.git", work.file_name().unwrap().to_string_lossy()));
        let _ = std::fs::remove_dir_all(&bare);
        std::fs::create_dir_all(&bare).unwrap();
        git(&bare, &["init", "-q", "--bare"]);
        std::fs::write(work.join("a.txt"), "a\n").unwrap();
        git(&work, &["add", "."]);
        git(&work, &["commit", "-q", "-m", "a"]);
        let gone = work.with_file_name("glowfly-push-all-no-such-remote");
        git(&work, &["remote", "add", "broken", &gone.display().to_string()]);
        git(&work, &["remote", "add", "good", &bare.display().to_string()]);
        let pushes = plan(&branch("main", None), ["broken", "good"]);
        let done = run(&work, &pushes, gitcli::BACKGROUND);
        assert_eq!(done.failed, ["broken"], "{}", done.output);
        assert!(done.output.contains("$ git push -- good refs/heads/main:refs/heads/main"), "{}", done.output);
        let (ok, _) = gitcli::run(&bare, &args(&["rev-parse", "--verify", "refs/heads/main"]));
        assert!(ok, "the second remote got the branch");
        let _ = std::fs::remove_dir_all(&work);
        let _ = std::fs::remove_dir_all(&bare);
    }
}
