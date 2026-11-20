//! A whole load against libgit2's per-item reads (what the load did before it read the refs in
//! one pass): the same locals with their upstreams, remote branches, tags, labels and walk
//! starts, in the same order, on a repository with loose and packed refs, symbolic refs, names
//! git could read as options, a hidden ref and a remote's HEAD.

use crate::repo::*;
use crate::testutil::*;
use git2::{BranchType, Oid, Repository};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn commit(dir: &Path, msg: &str) -> Oid {
    git(dir, &["commit", "-q", "--allow-empty", "-m", msg]);
    Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

/// A repository whose refs were packed, then given loose ones that add to and override them.
fn fixture(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    let c1 = commit(&dir, "c1");
    let c2 = commit(&dir, "c2");
    let c3 = commit(&dir, "c3");
    git(&dir, &["remote", "add", "origin", "https://example.invalid/o.git"]);
    git(&dir, &["remote", "add", "fork", "https://example.invalid/f.git"]);
    for (branch, at) in [("b1", c1), ("b2", c2), ("hid", c2), ("topic/a", c1), ("topic/b", c3), ("gone", c1), ("dot", c1), ("short", c2)] {
        git(&dir, &["branch", branch, &at.to_string()]);
    }
    for (r, at) in [("origin/main", c2), ("origin/b1", c1), ("origin/feature/nested/x", c3), ("origin/hid", c1), ("fork/main", c1)] {
        git(&dir, &["update-ref", &format!("refs/remotes/{r}"), &at.to_string()]);
    }
    git(&dir, &["symbolic-ref", "refs/remotes/origin/HEAD", "refs/remotes/origin/main"]);
    git(&dir, &["tag", "light", &c1.to_string()]);
    git(&dir, &["tag", "-a", "ann", "-m", "an annotation", &c2.to_string()]);
    git(&dir, &["tag", "-a", "nested", "-m", "of a tag", "ann"]);
    git(&dir, &["tag", "hidtag", &c3.to_string()]);
    git(&dir, &["pack-refs", "--all"]);

    // Loose refs beside the packed ones.
    git(&dir, &["branch", "late", &c3.to_string()]);
    git(&dir, &["update-ref", "refs/heads/b1", &c3.to_string()]);
    git(&dir, &["update-ref", "refs/remotes/origin/late", &c2.to_string()]);
    git(&dir, &["tag", "fresh", &c2.to_string()]);
    git(&dir, &["symbolic-ref", "refs/heads/alias", "refs/heads/main"]);
    git(&dir, &["symbolic-ref", "refs/tags/sym", "refs/heads/b2"]);
    git(&dir, &["symbolic-ref", "refs/heads/dead", "refs/heads/nope"]);
    let repo = Repository::open(&dir).unwrap();
    let tree = repo.find_commit(c1).unwrap().tree_id();
    repo.reference("refs/tags/on-tree", tree, false, "a tag on a tree").unwrap();
    for bad in ["refs/heads/-bad", "refs/remotes/origin/-bad", "refs/tags/-bad", "refs/heads/dir/-bad"] {
        repo.reference(bad, c2, false, "a name git could read as an option").unwrap();
    }
    // Upstreams of each kind: a remote's, '.', a missing tracking ref, a short merge on '.', and
    // the settings of a symbolic and of an unsafe branch.
    for (branch, remote, merge) in [
        ("b1", "origin", "refs/heads/b1"),
        ("late", "origin", "refs/heads/late"),
        ("dot", ".", "refs/heads/main"),
        ("gone", "origin", "refs/heads/gone"),
        ("short", ".", "main"),
        ("b2", "fork", "refs/heads/main"),
        ("topic/a", ".", "refs/heads/b2"),
        ("alias", "origin", "refs/heads/main"),
        ("-bad", "origin", "refs/heads/main"),
    ] {
        git(&dir, &["config", &format!("branch.{branch}.remote"), remote]);
        git(&dir, &["config", &format!("branch.{branch}.merge"), merge]);
    }
    git(&dir, &["config", "branch.b1.pushRemote", "fork"]);
    dir
}

type Local = (String, Option<Oid>, bool, Option<(String, String)>, Option<String>, Option<String>, usize, usize);
type Label = (String, String, u8, bool);
type Remotes = Vec<(String, Vec<(String, Oid)>)>;

/// What a load reads, as libgit2's per-item calls read it.
struct Old {
    locals: Vec<Local>,
    remotes: Remotes,
    tags: (Vec<(String, Oid)>, Vec<String>, Vec<String>),
    labels: HashMap<Oid, Vec<Label>>,
    starts: Vec<Oid>,
}

fn old_locals(repo: &Repository) -> Vec<Local> {
    let mut locals = Vec::new();
    for b in repo.branches(Some(BranchType::Local)).unwrap() {
        let (b, _) = b.unwrap();
        let name = b.name().unwrap().unwrap_or_default().to_owned();
        if !is_safe_ref_name(&name) {
            continue;
        }
        let oid = b.get().target();
        let full = format!("refs/heads/{name}");
        let tracking = match (repo.branch_upstream_remote(&full), repo.branch_upstream_merge(&full)) {
            (Ok(r), Ok(m)) => match (r.as_str(), m.as_str()) {
                (Some(r), Some(m)) => Some((r.to_owned(), m.strip_prefix("refs/heads/").unwrap_or(m).to_owned())),
                _ => None,
            },
            _ => None,
        };
        let tracking = tracking.filter(|(r, m)| (r == "." || crate::remote_ops::is_safe_remote_name(r)) && is_safe_ref_name(m));
        let (upstream, upstream_ref, ahead, behind) = match b.upstream() {
            Ok(u) if tracking.is_some() => {
                let (a, bh) = match (oid, u.get().target()) {
                    (Some(x), Some(y)) => repo.graph_ahead_behind(x, y).unwrap(),
                    _ => (0, 0),
                };
                (u.name().ok().flatten().map(str::to_owned), u.get().name().map(str::to_owned), a, bh)
            }
            _ => (None, None, 0, 0),
        };
        locals.push((name, oid, b.is_head(), tracking, upstream, upstream_ref, ahead, behind));
    }
    locals
}

fn old_remotes(repo: &Repository) -> Remotes {
    let names = repo.remotes().unwrap();
    let mut remotes: Remotes =
        names.iter().flatten().filter(|r| crate::remote_ops::is_safe_remote_name(r)).map(|r| (r.to_owned(), Vec::new())).collect();
    for b in repo.branches(Some(BranchType::Remote)).unwrap() {
        let (b, _) = b.unwrap();
        let (Some(name), Some(oid)) = (b.name().unwrap().map(str::to_owned), b.get().target()) else { continue };
        if name.ends_with("/HEAD") || !is_safe_ref_name(&name) {
            continue;
        }
        if let Some(slot) = remotes.iter_mut().filter(|(r, _)| name.starts_with(&format!("{r}/"))).max_by_key(|(r, _)| r.len()) {
            let short = name[slot.0.len() + 1..].to_owned();
            slot.1.push((short, oid));
        }
    }
    remotes
}

fn old_tags(repo: &Repository) -> (Vec<(String, Oid)>, Vec<String>, Vec<String>) {
    let (mut tags, mut other, mut light) = (Vec::new(), Vec::new(), Vec::new());
    for t in repo.tag_names(None).unwrap().iter().flatten().filter(|t| is_safe_ref_name(t)) {
        let found = repo.revparse_single(&format!("refs/tags/{t}")).and_then(|o| {
            let is_light = o.kind() == Some(git2::ObjectType::Commit);
            o.peel_to_commit().map(|c| (c, is_light))
        });
        match found {
            Ok((c, is_light)) => {
                if is_light {
                    light.push(t.to_owned());
                }
                tags.push((t.to_owned(), c.id()));
            }
            Err(_) => other.push(t.to_owned()),
        }
    }
    tags.reverse();
    light.sort();
    (tags, other, light)
}

/// The labels and walk starts of the refs, with `head` as (commit, branch, detached).
fn old_labels(repo: &Repository, opts: &LoadOptions, head: (Option<Oid>, Option<&str>, bool)) -> (HashMap<Oid, Vec<Label>>, Vec<Oid>) {
    let (head_oid, head_branch, detached) = head;
    let mut labels: HashMap<Oid, Vec<Label>> = HashMap::new();
    if detached && let Some(o) = head_oid {
        labels.entry(o).or_default().push(("HEAD".into(), String::new(), RefKind::Head as u8, true));
    }
    let mut starts: Vec<Oid> = head_oid.into_iter().collect();
    for r in repo.references().unwrap().flatten() {
        let Some(full) = r.name() else { continue };
        let (kind, short) = if let Some(s) = full.strip_prefix("refs/heads/") {
            (RefKind::Local, s)
        } else if let Some(s) = full.strip_prefix("refs/remotes/") {
            (RefKind::Remote, s)
        } else if let Some(s) = full.strip_prefix("refs/tags/") {
            (RefKind::Tag, s)
        } else {
            continue;
        };
        let hidden = opts.hidden.iter().any(|h| h == full);
        let commit = r.peel_to_commit();
        if !hidden && let Ok(c) = &commit {
            starts.push(c.id());
        }
        if (kind == RefKind::Remote && short.ends_with("/HEAD")) || hidden || !is_safe_ref_name(short) {
            continue;
        }
        let Ok(commit) = commit else { continue };
        let is_head = kind == RefKind::Local && Some(short) == head_branch;
        labels.entry(commit.id()).or_default().push((short.to_owned(), full.to_owned(), kind as u8, is_head));
    }
    for l in labels.values_mut() {
        l.sort_by(|a, b| (!a.3, a.2, &a.0).cmp(&(!b.3, b.2, &b.0)));
    }
    (labels, starts)
}

fn old_reads(repo: &Repository, opts: &LoadOptions, head: (Option<Oid>, Option<&str>, bool)) -> Old {
    let (labels, starts) = old_labels(repo, opts, head);
    Old { locals: old_locals(repo), remotes: old_remotes(repo), tags: old_tags(repo), labels, starts }
}

fn check(dir: &Path, opts: &LoadOptions) {
    let s = load(dir, opts).unwrap();
    let repo = Repository::open(dir).unwrap();
    let detached = s.head_branch.is_none() && s.head_oid.is_some();
    let old = old_reads(&repo, opts, (s.head_oid, s.head_branch.as_deref(), detached));

    let new_locals: Vec<Local> = s
        .locals
        .iter()
        .map(|b| {
            let t = b.tracking.as_ref().map(|t| (t.remote.clone(), t.merge.clone()));
            (b.name.clone(), b.oid, b.is_head, t, b.upstream.clone(), b.upstream_ref.clone(), b.ahead, b.behind)
        })
        .collect();
    assert_eq!(new_locals, old.locals, "locals, in order");
    assert_eq!(s.remotes, old.remotes, "remote branches, in order");
    let mut light: Vec<String> = s.lightweight_tags.iter().cloned().collect();
    light.sort();
    assert_eq!((s.tags.clone(), s.other_tags.clone(), light), old.tags, "tags");
    let new_labels: HashMap<Oid, Vec<Label>> =
        s.refs.iter().map(|(o, ls)| (*o, ls.iter().map(|l| (l.name.clone(), l.full.clone(), l.kind as u8, l.is_head)).collect())).collect();
    assert_eq!(new_labels, old.labels, "labels");
    assert_eq!(s.starts, old.starts, "walk starts, in order");
}

#[test]
fn a_whole_load_matches_libgit2s_per_item_reads() {
    let dir = fixture("load-refs-parity");
    let hidden = ["refs/heads/hid", "refs/remotes/origin/hid", "refs/tags/hidtag"].map(str::to_owned).to_vec();
    let opts = LoadOptions { hidden, ..LoadOptions::all(50) };
    let s = load(&dir, &opts).unwrap();
    // What the fixture is for.
    let local = |n: &str| s.locals.iter().find(|b| b.name == n);
    assert!(local("hid").is_some() && local("-bad").is_none() && local("alias").is_some_and(|b| b.oid.is_none()));
    assert!(local("gone").is_some_and(|b| b.tracking.is_some() && b.upstream.is_none()));
    assert!(local("dot").is_some_and(|b| b.upstream.as_deref() == Some("main")));
    assert!(local("short").is_some_and(|b| b.tracking.is_some() && b.upstream.is_none()), "a short merge on '.' names no ref");
    assert!(local("b1").is_some_and(|b| b.upstream.as_deref() == Some("origin/b1") && (b.ahead, b.behind) == (2, 0)));
    assert!(s.remotes.iter().any(|(r, bs)| r == "origin" && bs.iter().any(|(b, _)| b == "hid")));
    assert!(s.tags.iter().any(|(t, _)| t == "hidtag") && s.other_tags == ["on-tree"]);
    check(&dir, &opts);
    check(&dir, &LoadOptions::all(50));
    git(&dir, &["checkout", "-q", "--detach", "late"]);
    check(&dir, &opts);
    git(&dir, &["checkout", "-q", "b2"]);
    check(&dir, &opts);
}
