//! The kit's in-process `checkout`, `branch`, `tag`, `update-ref` and `remote`, in their plain forms, with the reflog
//! messages the git CLI writes. Each returns false, having changed nothing, when the command
//! is in another form or git would do more than this does (set an upstream, sign a tag).

use super::guard;
use git2::build::CheckoutBuilder;
use git2::{Commit, Oid, Repository};

/// `git checkout [-q] -b <new> [<start>]`, `git checkout [-q] <branch>` for a local branch
/// other than the current one, and `git checkout [-q] <full commit id>` (detaching HEAD).
pub(super) fn checkout(repo: &Repository, a: &[&str]) -> bool {
    let a: Vec<&str> = a.iter().copied().filter(|&x| x != "-q" && x != "--quiet").collect();
    let Some(from) = head_name(repo) else { return false };
    match a.as_slice() {
        ["-b", name] | ["-b", name, "HEAD"] => {
            let Some(head) = head_commit(repo) else { return false };
            if !plain_name(name) {
                return false;
            }
            let made = repo.reference(&format!("refs/heads/{name}"), head.id(), false, "branch: Created from HEAD");
            made.is_ok() && attach(repo, name, &from)
        }
        ["-b", name, start] if !start.starts_with('-') => {
            let new = format!("refs/heads/{name}");
            if !plain_name(name) || repo.find_reference(&new).is_ok() || from_remote(repo, start) {
                return false;
            }
            let Ok(commit) = repo.revparse_single(start).and_then(|o| o.peel_to_commit()) else { return false };
            switch_tree(repo, &commit)
                && repo.reference(&new, commit.id(), false, &format!("branch: Created from {start}")).is_ok()
                && attach(repo, name, &from)
        }
        [target] if !target.starts_with('-') && !exists_in_workdir(repo, target) => {
            // Git refuses a branch another worktree has checked out; leave that to it.
            let worktrees = repo.commondir().join("worktrees").exists();
            if worktrees {
                false
            } else if repo.find_reference(&format!("refs/heads/{target}")).is_ok() {
                let Ok(commit) = repo.revparse_single(&format!("refs/heads/{target}")).and_then(|o| o.peel_to_commit()) else {
                    return false;
                };
                *target != from && switch_tree(repo, &commit) && attach(repo, target, &from)
            } else if target.len() == 40 && target.bytes().all(|b| b.is_ascii_hexdigit()) {
                let Some(commit) = Oid::from_str(target).ok().and_then(|id| repo.find_commit(id).ok()) else { return false };
                let message = format!("checkout: moving from {from} to {target}");
                switch_tree(repo, &commit) && repo.reference("HEAD", commit.id(), true, &message).is_ok()
            } else {
                false
            }
        }
        _ => false,
    }
}

/// `git branch <new> [<start>]` (start defaults to HEAD); not from a remote-tracking branch,
/// which would also set an upstream.
pub(super) fn branch(repo: &Repository, a: &[&str]) -> bool {
    let (name, start) = match a {
        [name] => (*name, None),
        [name, start] if !start.starts_with('-') => (*name, Some(*start)),
        _ => return false,
    };
    if !plain_name(name) {
        return false;
    }
    let commit = match start {
        None => head_commit(repo),
        Some(s) => {
            if from_remote(repo, s) {
                return false;
            }
            repo.revparse_single(s).and_then(|o| o.peel_to_commit()).ok()
        }
    };
    let Some(commit) = commit else { return false };
    let message = format!("branch: Created from {}", start.unwrap_or("HEAD"));
    repo.reference(&format!("refs/heads/{name}"), commit.id(), false, &message).is_ok()
}

/// `git tag <name> [<target>]`: a lightweight tag (no message, so no tag object).
pub(super) fn tag(repo: &Repository, a: &[&str]) -> bool {
    let (name, target) = match a {
        [name] => (*name, "HEAD"),
        [name, target] if !target.starts_with('-') => (*name, *target),
        _ => return false,
    };
    if !plain_name(name) || repo.config().and_then(|c| c.get_bool("tag.gpgsign")).unwrap_or(false) {
        return false;
    }
    let Ok(object) = repo.revparse_single(target) else { return false };
    repo.reference(&format!("refs/tags/{name}"), object.id(), false, "").is_ok()
}

/// `git remote add <name> <url>` (the URL and git's default fetch refspec) and `git remote
/// set-url <name> <url>`.
pub(super) fn remote(repo: &Repository, a: &[&str]) -> bool {
    match a {
        ["add", name, url] if !name.starts_with('-') && !url.starts_with('-') => repo.remote(name, url).is_ok(),
        ["set-url", name, url] if !name.starts_with('-') && !url.starts_with('-') => {
            repo.find_remote(name).is_ok() && repo.remote_set_url(name, url).is_ok()
        }
        _ => false,
    }
}

/// `git update-ref refs/<name> <rev>`: points the ref at the object, logged without a message
/// as git does (HEAD, which git would follow to its branch, is left to git).
pub(super) fn update_ref(repo: &Repository, a: &[&str]) -> bool {
    let [name, rev] = a else { return false };
    if !name.starts_with("refs/") || rev.starts_with('-') {
        return false;
    }
    let Ok(object) = repo.revparse_single(rev) else { return false };
    repo.reference(name, object.id(), true, "").is_ok()
}

/// Whether a branch started from `start` would get an upstream: `start` is a remote-tracking
/// branch, or the config sets one up for every branch.
fn from_remote(repo: &Repository, start: &str) -> bool {
    let setup = repo.config().and_then(|c| c.get_string("branch.autosetupmerge")).unwrap_or_default();
    start.starts_with("refs/remotes/")
        || repo.find_reference(&format!("refs/remotes/{start}")).is_ok()
        || setup.eq_ignore_ascii_case("always")
}

/// A name git would take as given: not an option, not `HEAD`, and a valid ref name.
pub(super) fn plain_name(name: &str) -> bool {
    !name.starts_with('-') && name != "HEAD" && git2::Reference::is_valid_name(&format!("refs/heads/{name}"))
}

/// The commit HEAD points at; None on an unborn branch.
fn head_commit(repo: &Repository) -> Option<Commit<'_>> {
    repo.head().ok()?.peel_to_commit().ok()
}

/// How git names HEAD in a checkout's reflog: the branch, or the full id when detached. None
/// on an unborn branch or when HEAD points outside `refs/heads/`.
fn head_name(repo: &Repository) -> Option<String> {
    let head = repo.find_reference("HEAD").ok()?;
    match head.symbolic_target() {
        Some(target) => {
            let name = target.strip_prefix("refs/heads/")?.to_owned();
            head_commit(repo).map(|_| name)
        }
        None => head.target().map(|id| id.to_string()),
    }
}

/// Whether `name` is also a file or folder in the working tree, where git would have to choose.
fn exists_in_workdir(repo: &Repository, name: &str) -> bool {
    repo.workdir().is_none_or(|w| w.join(name).exists())
}

/// Updates the working tree and index from HEAD's tree to `commit`'s, keeping local changes
/// git would keep; false (with nothing written) on a conflict git would also refuse.
fn switch_tree(repo: &Repository, commit: &Commit<'_>) -> bool {
    let Ok(tree) = commit.tree() else { return false };
    let Ok(index) = repo.index() else { return false };
    if guard::index_needs_cli(repo, &index) || tree_needs_cli(repo, &tree) {
        return false;
    }
    repo.checkout_tree(tree.as_object(), Some(CheckoutBuilder::new().safe())).is_ok()
}

/// Whether `tree` holds a submodule or an attributes file asking for git's filters.
fn tree_needs_cli(repo: &Repository, tree: &git2::Tree<'_>) -> bool {
    let mut odd = false;
    let _ = tree.walk(git2::TreeWalkMode::PreOrder, |_, e| {
        if e.kind() == Some(git2::ObjectType::Commit) {
            odd = true;
        } else if e.name() == Some(".gitattributes") {
            odd |= repo.find_blob(e.id()).map_or(true, |b| guard::attrs_need_cli(&String::from_utf8_lossy(b.content())));
        }
        if odd { git2::TreeWalkResult::Abort } else { git2::TreeWalkResult::Ok }
    });
    odd
}

/// Points HEAD at `refs/heads/<name>` with git's checkout reflog message.
fn attach(repo: &Repository, name: &str, from: &str) -> bool {
    let message = format!("checkout: moving from {from} to {name}");
    repo.reference_symbolic("HEAD", &format!("refs/heads/{name}"), true, &message).is_ok()
}
