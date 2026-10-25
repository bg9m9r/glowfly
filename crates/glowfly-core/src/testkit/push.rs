//! The kit's in-process `git push [-q] [-u] <remote> <branch>` to a bare repository in a local
//! folder (the `origin` most fixtures make next to the working repository): a new branch or a
//! fast-forward, the remote-tracking ref and, with `-u`, the upstream.

use super::{guard, refs};
use git2::{PushOptions, RemoteCallbacks, Repository};
use std::cell::Cell;

pub(super) fn push(repo: &Repository, a: &[&str]) -> bool {
    let (mut upstream, mut words) = (false, Vec::new());
    for &arg in a {
        match arg {
            "-q" | "--quiet" => {}
            "-u" | "--set-upstream" => upstream = true,
            _ if arg.starts_with('-') => return false,
            _ => words.push(arg),
        }
    }
    let [remote_name, branch] = words[..] else { return false };
    if !refs::plain_name(branch) {
        return false;
    }
    let local_ref = format!("refs/heads/{branch}");
    let Some(new) = repo.find_reference(&local_ref).ok().and_then(|r| r.target()) else { return false };
    let Ok(mut remote) = repo.find_remote(remote_name) else { return false };
    let Some(url) = remote.url().map(str::to_owned) else { return false };
    // Only a folder on this machine: not a URL, and not text read against the current folder.
    if remote.pushurl().is_some() || url.contains("://") || url.starts_with('~') || !crate::paths::typed_is_absolute(&url) {
        return false;
    }
    let Ok(target) = Repository::open_bare(&url) else { return false };
    if guard::hooks(&target) {
        return false;
    }
    // Git refuses a push that isn't a fast-forward, and says "Everything up-to-date" for one
    // that changes nothing; both are left to it.
    if let Some(old) = target.find_reference(&local_ref).ok().and_then(|r| r.target())
        && (old == new || !repo.graph_descendant_of(new, old).unwrap_or(false))
    {
        return false;
    }
    let refused = Cell::new(false);
    let mut callbacks = RemoteCallbacks::new();
    callbacks.push_update_reference(|_, status| {
        refused.set(refused.get() || status.is_some());
        Ok(())
    });
    let mut opts = PushOptions::new();
    opts.remote_callbacks(callbacks);
    let pushed = remote.push(&[format!("{local_ref}:{local_ref}")], Some(&mut opts)).is_ok();
    if !pushed || refused.get() {
        panic!("in-process push of {branch} to {url} failed");
    }
    let tracking = format!("refs/remotes/{remote_name}/{branch}");
    if repo.find_reference(&tracking).ok().and_then(|r| r.target()) != Some(new) {
        repo.reference(&tracking, new, true, "update by push").expect("the remote-tracking ref");
    }
    if upstream {
        let mut config = repo.config().and_then(|c| c.open_level(git2::ConfigLevel::Local)).expect("the repository's config");
        config.set_str(&format!("branch.{branch}.remote"), remote_name).expect("branch.<name>.remote");
        config.set_str(&format!("branch.{branch}.merge"), &local_ref).expect("branch.<name>.merge");
    }
    true
}
