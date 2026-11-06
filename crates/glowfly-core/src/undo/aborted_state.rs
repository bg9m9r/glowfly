//! Keeping and restoring what an abort of a run of steps (a rebase, a patch series, a
//! multi-commit cherry-pick or revert) removes besides files: its state folder in the git dir
//! (`rebase-merge`, `rebase-apply` or `sequencer`, kept byte for byte as the kept commit's
//! `gitdir` tree), where HEAD was, the branch the abort moves and the label refs it deletes (the
//! `heads` blob). Refused, with nothing written, when git's abort would do something else than
//! take the branch back: a rebase that stashed changes itself, or a patch series or run of picks
//! whose HEAD moved since it stopped. The folder comes back through a staging folder renamed into
//! place, so a crash never leaves half of it.

use super::aborted::not_utf8;
use super::record::branch_name;
use super::{HeadPos, MoveOp};
use crate::error::{Error, Result};
use crate::persist;
use git2::{ObjectType, Oid, Repository, Tree, TreeBuilder};
use std::path::Path;

/// The state folders an abort of a run of steps removes.
const DIRS: [&str; 3] = ["rebase-merge", "rebase-apply", "sequencer"];

/// The reflog message of the refs Undo puts back.
const REFLOG: &str = "glowfly: undo of an abort";

#[derive(Clone, Copy, Debug, PartialEq)]
enum RefKind {
    /// HEAD: on the branch `name`, or detached (`name` empty) at `oid`.
    Head,
    /// A branch the abort moves, back at `oid` (zero: it didn't exist).
    Branch,
    /// A label ref of `rebase -r` the abort deletes.
    Label,
}

impl RefKind {
    fn name(self) -> &'static str {
        match self {
            RefKind::Head => "head",
            RefKind::Branch => "branch",
            RefKind::Label => "label",
        }
    }

    fn parse(s: &str) -> Option<RefKind> {
        [RefKind::Head, RefKind::Branch, RefKind::Label].into_iter().find(|k| k.name() == s)
    }
}

/// A ref as it was while the operation was in progress.
#[derive(Clone, Debug)]
struct KeptRef {
    kind: RefKind,
    name: String,
    oid: Oid,
}

/// What `read` found about the run of steps in progress.
pub(super) struct Steps {
    /// The state folder, under the git dir.
    pub dir: &'static str,
    /// The command whose `--abort` ends it.
    pub verb: &'static str,
    /// The commit the abort takes HEAD back to.
    pub target: Oid,
    /// The branch HEAD is on after the abort (`None`: detached).
    pub after_branch: Option<String>,
    /// The abort resets every file (a rebase's hard reset), so unrelated edits are kept too.
    pub resets_all: bool,
    refs: Vec<KeptRef>,
    /// Commits the state folder names (its list of steps, its labels), kept reachable.
    pub commits: Vec<Oid>,
}

fn refused(why: String) -> Error {
    Error::Unsupported(why)
}

/// The commits the todo list `text` names: in each line that isn't blank or a comment, the
/// first three words that are 7 to 40 hex digits and name a commit.
fn todo_commits(repo: &Repository, text: &str) -> Vec<Oid> {
    let lines = text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#'));
    let hex = |w: &&str| (7..=40).contains(&w.len()) && w.bytes().all(|b| b.is_ascii_hexdigit());
    let words = lines.flat_map(|l| l.split_whitespace().take(3).filter(hex).collect::<Vec<_>>());
    words.filter_map(|w| repo.revparse_single(w).ok()?.peel_to_commit().ok().map(|c| c.id())).collect()
}

/// Reads the run of steps `op` in progress, its state in `dir`, ended by `<verb> --abort`, with
/// HEAD at `pos` and commit `s`. Refuses what can't be kept or wouldn't go back as Undo expects.
pub(super) fn read(repo: &Repository, op: MoveOp, dir: &'static str, verb: &'static str, pos: &Option<HeadPos>, s: Oid) -> Result<Steps> {
    let folder = repo.path().join(dir);
    let noun = op.noun();
    let file = |name: &str| -> Result<Option<String>> {
        match std::fs::read_to_string(folder.join(name)) {
            Ok(t) => Ok(Some(t.trim().to_owned())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    };
    let missing = |name: &str| refused(format!("{dir}/{name} is missing, so the {noun} can't be kept"));
    let unreadable = |name: &str| refused(format!("{dir}/{name} can't be read, so the {noun} can't be kept"));
    let oid_in = |name: &str| -> Result<Oid> { Oid::from_str(&file(name)?.ok_or_else(|| missing(name))?).map_err(|_| unreadable(name)) };
    // Git's abort of a patch series or a run of picks only rewinds while HEAD is where it stopped.
    let safe = || -> Result<()> {
        match file("abort-safety")?.and_then(|t| Oid::from_str(&t).ok()) {
            Some(o) if o == s => Ok(()),
            _ => Err(refused(format!("HEAD has moved since the {noun} stopped, so its abort wouldn't take the branch back"))),
        }
    };
    let branch = branch_name(pos);
    let mut refs = vec![KeptRef { kind: RefKind::Head, name: branch.clone().unwrap_or_default(), oid: s }];
    let on_branch =
        |refs: &mut Vec<KeptRef>, b: &str, oid: Oid| refs.push(KeptRef { kind: RefKind::Branch, name: format!("refs/heads/{b}"), oid });
    let mut commits = Vec::new();
    let (target, after_branch, resets_all) = match op {
        MoveOp::Rebase => {
            if folder.join("autostash").exists() {
                return Err(refused(
                    "The rebase stashed your changes when it started and its abort puts them back, so the work can't be kept".into(),
                ));
            }
            let head_name = file("head-name")?.ok_or_else(|| missing("head-name"))?;
            let after = match head_name.strip_prefix("refs/heads/") {
                Some(b) => Some(b.to_owned()),
                None if head_name == "detached HEAD" => None,
                None => return Err(unreadable("head-name")),
            };
            let target = oid_in("orig-head")?;
            if let Some(b) = &after {
                on_branch(&mut refs, b, repo.refname_to_id(&format!("refs/heads/{b}")).unwrap_or_else(|_| Oid::zero()));
            }
            for name in file("refs-to-delete")?.unwrap_or_default().lines().map(str::trim).filter(|l| !l.is_empty()) {
                if let Ok(oid) = repo.refname_to_id(name) {
                    refs.push(KeptRef { kind: RefKind::Label, name: name.to_owned(), oid });
                    commits.push(oid);
                }
            }
            for list in ["git-rebase-todo", "done"] {
                commits.extend(todo_commits(repo, &file(list)?.unwrap_or_default()));
            }
            (target, after, true)
        }
        MoveOp::Am => {
            if folder.join("dirtyindex").exists() {
                return Err(refused("The patch series started with staged changes, so its abort wouldn't take the branch back".into()));
            }
            safe()?;
            let target =
                repo.refname_to_id("ORIG_HEAD").map_err(|_| refused(format!("ORIG_HEAD is missing, so the {noun} can't be kept")))?;
            if let Some(b) = &branch {
                on_branch(&mut refs, b, s);
            }
            (target, branch.clone(), false)
        }
        _ => {
            safe()?;
            let target = oid_in("head")?;
            if let Some(b) = &branch {
                on_branch(&mut refs, b, s);
            }
            commits.extend(todo_commits(repo, &file("todo")?.unwrap_or_default()));
            (target, branch.clone(), false)
        }
    };
    Ok(Steps { dir, verb, target, after_branch, resets_all, refs, commits })
}

/// The tree of the folder `path` (named `rel` in messages): its files as blobs, its folders as
/// trees. Anything else, or a name that isn't UTF-8, is refused.
fn folder_tree(repo: &Repository, path: &Path, rel: &str) -> Result<Oid> {
    let mut b = repo.treebuilder(None)?;
    for e in std::fs::read_dir(path)? {
        let e = e?;
        let name = e.file_name().into_string().map_err(|_| not_utf8())?;
        let (kind, rel) = (e.file_type()?, format!("{rel}/{name}"));
        if kind.is_dir() {
            b.insert(&name, folder_tree(repo, &e.path(), &rel)?, 0o040000)?;
        } else if kind.is_file() {
            b.insert(&name, repo.blob(&std::fs::read(e.path())?)?, 0o100644)?;
        } else {
            return Err(refused(format!("{rel} isn't a regular file")));
        }
    }
    Ok(b.write()?)
}

/// Adds the state folder (`gitdir/<dir>/…`) and the refs (`heads`) of `steps` to the kept
/// commit's tree `top`.
pub(super) fn keep(repo: &Repository, steps: &Steps, top: &mut TreeBuilder) -> Result<()> {
    let mut gitdir = repo.treebuilder(None)?;
    gitdir.insert(steps.dir, folder_tree(repo, &repo.path().join(steps.dir), steps.dir)?, 0o040000)?;
    top.insert("gitdir", gitdir.write()?, 0o040000)?;
    let heads: String = steps.refs.iter().map(|r| format!("{}\0{}\0{}\0", r.kind.name(), r.name, r.oid)).collect();
    top.insert("heads", repo.blob(heads.as_bytes())?, 0o100644)?;
    Ok(())
}

/// The state folder and refs a kept abort of a run of steps holds.
pub(super) struct KeptSteps {
    dir: String,
    tree: Oid,
    refs: Vec<KeptRef>,
}

fn damaged() -> Error {
    Error::Other("The kept work is damaged".into())
}

/// The run of steps the kept commit's tree `top` holds; `None` for a merge, single pick or
/// revert, which has no state folder.
pub(super) fn read_kept(repo: &Repository, top: &Tree) -> Result<Option<KeptSteps>> {
    let Some(gitdir) = top.get_name("gitdir") else { return Ok(None) };
    let gitdir = repo.find_tree(gitdir.id())?;
    let folder = gitdir.iter().next().ok_or_else(damaged)?;
    let dir = folder.name().filter(|n| DIRS.contains(n)).ok_or_else(damaged)?.to_owned();
    let heads = repo.find_blob(top.get_name("heads").ok_or_else(damaged)?.id())?;
    let text = std::str::from_utf8(heads.content()).map_err(|_| damaged())?;
    let fields: Vec<&str> = text.strip_suffix('\0').map(|t| t.split('\0').collect()).unwrap_or_default();
    let mut refs = Vec::new();
    for f in fields.chunks(3) {
        let [kind, name, oid] = f else { return Err(damaged()) };
        let kind = RefKind::parse(kind).ok_or_else(damaged)?;
        refs.push(KeptRef { kind, name: (*name).to_owned(), oid: Oid::from_str(oid).map_err(|_| damaged())? });
    }
    if refs.iter().filter(|r| r.kind == RefKind::Head).count() != 1 {
        return Err(damaged());
    }
    Ok(Some(KeptSteps { dir, tree: folder.id(), refs }))
}

/// Why `kept` can't come back now (`what` names the abort): its state folder exists again (a
/// new operation started), or one of its labels was made again at another commit.
pub(super) fn check(repo: &Repository, kept: &KeptSteps, what: &str) -> Option<String> {
    if repo.path().join(&kept.dir).symlink_metadata().is_ok() {
        return Some(format!("Another operation was started since the {what}"));
    }
    let made = kept.refs.iter().filter(|r| r.kind == RefKind::Label).find(|r| repo.refname_to_id(&r.name).is_ok_and(|o| o != r.oid));
    made.map(|r| format!("{} was made again since the {what}", r.name))
}

/// Writes the files of `tree` into the folder `dest`.
fn write_folder(repo: &Repository, tree: &Tree, dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest)?;
    for e in tree.iter() {
        let name = e.name().ok_or_else(damaged)?;
        let path = dest.join(crate::repo::contained(name)?);
        match e.kind() {
            Some(ObjectType::Tree) => write_folder(repo, &repo.find_tree(e.id())?, &path)?,
            Some(ObjectType::Blob) => persist::atomic_write(&path, repo.find_blob(e.id())?.content())?,
            _ => return Err(damaged()),
        }
    }
    Ok(())
}

/// Puts `kept` back: the branch the abort moved, HEAD, the labels, then the state folder,
/// written to `glowfly-restoring-<dir>` first and renamed into place (over a folder a failed
/// abort left, which is moved aside first and removed once the new one is there).
pub(super) fn put(repo: &Repository, kept: &KeptSteps) -> Result<()> {
    for r in kept.refs.iter().filter(|r| r.kind == RefKind::Branch) {
        if !r.oid.is_zero() {
            repo.reference(&r.name, r.oid, true, REFLOG)?;
        } else if let Ok(mut gone) = repo.find_reference(&r.name) {
            gone.delete()?;
        }
    }
    let head = kept.refs.iter().find(|r| r.kind == RefKind::Head).ok_or_else(damaged)?;
    if head.name.is_empty() {
        repo.set_head_detached(head.oid)?;
    } else {
        repo.set_head(&format!("refs/heads/{}", head.name))?;
    }
    for r in kept.refs.iter().filter(|r| r.kind == RefKind::Label) {
        repo.reference(&r.name, r.oid, true, REFLOG)?;
    }
    let dest = repo.path().join(&kept.dir);
    let staging = repo.path().join(format!("glowfly-restoring-{}", kept.dir));
    persist::remove_tree(&staging)?;
    write_folder(repo, &repo.find_tree(kept.tree)?, &staging)?;
    if dest.symlink_metadata().is_ok() {
        let backup = persist::move_aside(&dest)?;
        if let Err(e) = persist::rename_new(&staging, &dest) {
            let _ = std::fs::rename(&backup, &dest);
            return Err(e.into());
        }
        // The new folder is in place; a backup that won't go only takes room.
        let _ = persist::remove_tree(&backup);
    } else {
        persist::rename_new(&staging, &dest)?;
    }
    Ok(())
}
