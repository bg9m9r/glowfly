use super::*;
use crate::testutil::*;
use git2::Oid;
use std::collections::HashMap;
use std::path::PathBuf;

/// A commit of the empty tree with `parents`, by and committed by Test <test@example.com> at
/// `time` (Unix seconds); no ref is moved.
pub(super) fn raw_commit(repo: &git2::Repository, parents: &[Oid], time: i64, msg: &str) -> Oid {
    let sig = git2::Signature::new("Test", "test@example.com", &git2::Time::new(time, 0)).unwrap();
    let tree = repo.find_tree(repo.treebuilder(None).unwrap().write().unwrap()).unwrap();
    let parents: Vec<git2::Commit> = parents.iter().map(|p| repo.find_commit(*p).unwrap()).collect();
    let refs: Vec<&git2::Commit> = parents.iter().collect();
    repo.commit(None, &sig, &sig, msg, &tree, &refs).unwrap()
}

/// Points `name` at `oid`.
pub(super) fn point(repo: &git2::Repository, name: &str, oid: Oid) {
    repo.reference(name, oid, true, "test").unwrap();
}

/// A history of about 130 commits on `main`, newest last: branches `a`, `b` and `c` forked at
/// different depths and merged back by "Merge branch" commits, an octopus merge of two more, an
/// unrelated root `docs` merged in, an unmerged `feature`, `origin/main` behind `main` and a tag.
/// Every child is newer than its parents. Returns the folder and main's first-parent line,
/// oldest first.
pub(super) fn branchy(name: &str) -> (PathBuf, Vec<Oid>) {
    let dir = scratch_repo(name);
    let repo = git2::Repository::open(&dir).unwrap();
    repo.remote("origin", &file_url(&dir)).unwrap();
    let base = 1_600_000_000;
    let at = |i: usize| base + 1000 * i as i64;
    // `n` commits on top of `from` (None: a new root), `step` seconds apart after `start`.
    let line = |from: Option<Oid>, start: i64, n: usize, step: i64, label: &str| -> Vec<Oid> {
        let mut out: Vec<Oid> = Vec::new();
        for j in 0..n {
            let parent: Vec<Oid> = out.last().copied().or(from).into_iter().collect();
            out.push(raw_commit(&repo, &parent, start + step * (j as i64 + 1), &format!("{label} {j}")));
        }
        out
    };
    let mut main = vec![raw_commit(&repo, &[], at(0), "root")];
    for i in 1..=60 {
        let prev = main[i - 1];
        let fork = |k: usize| (main[k], at(k));
        let oid = match i {
            40 => {
                let (f, t) = fork(10);
                let a = line(Some(f), t, 15, 600, "a");
                point(&repo, "refs/heads/a", *a.last().unwrap());
                raw_commit(&repo, &[prev, *a.last().unwrap()], at(i), "Merge branch 'a'")
            }
            46 => {
                let (f, t) = fork(45);
                let feature = line(Some(f), t, 10, 300, "feature");
                point(&repo, "refs/heads/feature", *feature.last().unwrap());
                raw_commit(&repo, &[prev], at(i), "main 46")
            }
            50 => {
                let (f, t) = fork(20);
                let b = line(Some(f), t, 10, 900, "b");
                raw_commit(&repo, &[prev, *b.last().unwrap()], at(i), "Merge branch 'b' into main")
            }
            55 => {
                let (f, t) = fork(5);
                let c = line(Some(f), t, 20, 2000, "c");
                point(&repo, "refs/heads/c", *c.last().unwrap());
                raw_commit(&repo, &[prev, *c.last().unwrap()], at(i), "Merge branch 'c'")
            }
            57 => {
                let (f, t) = fork(56);
                let x = line(Some(f), t, 3, 100, "x");
                let y = line(Some(f), t, 3, 150, "y");
                raw_commit(&repo, &[prev, *x.last().unwrap(), *y.last().unwrap()], at(i), "Merge branches 'x' and 'y'")
            }
            58 => {
                let docs = line(None, base + 30_500, 8, 1000, "docs");
                point(&repo, "refs/heads/docs", *docs.last().unwrap());
                raw_commit(&repo, &[prev, *docs.last().unwrap()], at(i), "Merge branch 'docs'")
            }
            _ if i % 7 == 0 => raw_commit(&repo, &[prev], at(i), &format!("main {i}\n\nA body that\nspans  lines.\n")),
            _ => raw_commit(&repo, &[prev], at(i), &format!("main {i}")),
        };
        main.push(oid);
    }
    point(&repo, "refs/heads/main", *main.last().unwrap());
    point(&repo, "refs/remotes/origin/main", main[50]);
    point(&repo, "refs/tags/v1", main[30]);
    (dir, main)
}

/// Asserts `a` holds the history `b` does, laid out alike (`what` names the case).
#[track_caller]
pub(super) fn same_history(a: &Snapshot, b: &Snapshot, what: &str) {
    let oids = |s: &Snapshot| s.commits.iter().map(|c| c.oid).collect::<Vec<_>>();
    assert_eq!(oids(a), oids(b), "{what}: commits");
    for (x, y) in a.commits.iter().zip(b.commits.iter()) {
        let fields = |c: &CommitInfo| {
            (c.parents.to_vec(), c.summary.clone(), c.body.clone(), c.author.name().to_owned(), c.author.email().to_owned(), c.time)
        };
        assert_eq!(fields(x), fields(y), "{what}: commit {}", x.oid);
    }
    assert_eq!(a.row_of, b.row_of, "{what}: rows");
    assert_eq!(a.truncated, b.truncated, "{what}: truncated");
    assert_eq!(a.frontier.as_deref().map(Frontier::sorted), b.frontier.as_deref().map(Frontier::sorted), "{what}: frontier");
    assert_eq!(a.graph.rows, b.graph.rows, "{what}: graph rows");
    assert_eq!(
        (&a.graph.wip, &a.graph.worktree_wips, a.graph.max_width),
        (&b.graph.wip, &b.graph.worktree_wips, b.graph.max_width),
        "{what}"
    );
    assert_eq!(a.graph.owners, b.graph.owners, "{what}: owners");
    assert_eq!(a.graph.swatches, b.graph.swatches, "{what}: swatches");
    assert_eq!(a.graph, b.graph, "{what}: the whole layout");
}

fn page(dir: &std::path::Path, s: &Snapshot, max: usize) -> HistoryPage {
    match load_more(dir, &s.prior(), max).unwrap() {
        MoreLoaded::Page(p) => *p,
        MoreLoaded::NeedsFullLoad => panic!("a page through {max}"),
    }
}

#[test]
fn resumed_pages_match_one_walk() {
    let (dir, _) = branchy("more-pages");
    let mut fresh: HashMap<usize, Snapshot> = HashMap::new();
    let total = load(&dir, &LoadOptions::all(10_000)).unwrap().commits.len();
    assert!(total > 120, "{total}");
    for initial in [1, 5, 40] {
        for step in [1, 3, 17, 64] {
            let mut s = load(&dir, &LoadOptions::all(initial)).unwrap();
            assert!(s.truncated && s.frontier.is_some());
            while s.truncated {
                let want = s.commits.len() + step;
                let p = page(&dir, &s, want);
                assert_eq!(p.added(), step.min(total - s.commits.len()));
                s.extend_history(p).unwrap_or_else(|_| panic!("the page goes on from this history"));
                assert_eq!(s.loaded_with.max_commits, want);
                let f = fresh.entry(want).or_insert_with(|| load(&dir, &LoadOptions::all(want)).unwrap());
                same_history(&s, f, &format!("from {initial} by {step}, at {want}"));
            }
            assert_eq!(s.commits.len(), total);
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_cap_ends_a_resumed_walk_where_a_full_one_ends() {
    let (dir, _) = branchy("more-cap");
    let total = load(&dir, &LoadOptions::all(10_000)).unwrap().commits.len();
    let first = load(&dir, &LoadOptions::all(10)).unwrap();

    let mut s = load(&dir, &LoadOptions::all(10)).unwrap();
    s.extend_history(page(&dir, &s, total)).ok().unwrap();
    assert_eq!((s.commits.len(), s.truncated, s.frontier.is_none()), (total, false, true));
    assert!(s.prior().frontier.is_none());
    assert!(matches!(load_more(&dir, &s.prior(), total + 10).unwrap(), MoreLoaded::NeedsFullLoad), "nothing to go on from");

    let mut s = load(&dir, &LoadOptions::all(10)).unwrap();
    s.extend_history(page(&dir, &s, total - 1)).ok().unwrap();
    assert!(s.truncated);
    assert_eq!(s.frontier.as_ref().map(|f| f.len()), Some(1), "the one commit left");
    same_history(&s, &load(&dir, &LoadOptions::all(total - 1)).unwrap(), "one short");

    for max in [5, 10] {
        assert!(matches!(load_more(&dir, &first.prior(), max).unwrap(), MoreLoaded::NeedsFullLoad), "{max} is loaded already");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_skewed_child_past_the_page_boundary_needs_a_full_load() {
    let dir = scratch_repo("more-skew");
    let repo = git2::Repository::open(&dir).unwrap();
    let t = 1_600_000_000;
    let base = raw_commit(&repo, &[], t + 500, "base");
    let parent = raw_commit(&repo, &[base], t + 10_000, "parent");
    // The clock went back: "child" is far older than its parent.
    let child = raw_commit(&repo, &[parent], t + 100, "child");
    let tip = raw_commit(&repo, &[parent], t + 5_000, "tip");
    point(&repo, "refs/heads/main", tip);
    point(&repo, "refs/heads/side", child);

    let s = load(&dir, &LoadOptions::all(2)).unwrap();
    let order: Vec<&str> = s.commits.iter().map(|c| &*c.summary).collect();
    assert_eq!(order, ["tip", "parent"]);
    assert!(matches!(load_more(&dir, &s.prior(), 4).unwrap(), MoreLoaded::NeedsFullLoad), "child goes above parent");
    let reused = load_reusing(&dir, &LoadOptions::all(4), &GraphCache::default(), Some(&s.prior())).unwrap();
    let order: Vec<&str> = reused.commits.iter().map(|c| &*c.summary).collect();
    assert_eq!(order, ["tip", "child", "parent", "base"]);
    same_history(&reused, &load(&dir, &LoadOptions::all(4)).unwrap(), "skewed");

    // A page that only adds the root is fine.
    let mut s = s;
    s.extend_history(page(&dir, &s, 3)).ok().unwrap();
    same_history(&s, &load(&dir, &LoadOptions::all(3)).unwrap(), "the root only");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_worktree_row_past_the_first_page_lands_with_its_commit() {
    let (dir, main) = branchy("more-worktree");
    let wt = dir.with_file_name(format!("{}-wt", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&wt);
    git(&dir, &["worktree", "add", "-q", "--detach", &wt.display().to_string(), &main[3].to_string()]);
    write(&wt, "new.txt", "changed\n");
    write(&dir, "dirty.txt", "changed\n");

    let opts = |n| LoadOptions::all(n);
    let mut s = load(&dir, &opts(20)).unwrap();
    assert!(s.graph.wip.is_some(), "the main worktree is dirty");
    assert!(s.graph.worktree_wips.is_empty(), "the worktree's commit isn't loaded yet");
    while s.truncated {
        let want = s.commits.len() + 40;
        s.extend_history(page(&dir, &s, want)).ok().unwrap();
        same_history(&s, &load(&dir, &opts(want)).unwrap(), &format!("at {want}"));
    }
    let [w] = &s.graph.worktree_wips[..] else { panic!("one worktree row: {:?}", s.graph.worktree_wips) };
    assert_eq!(s.commits[w.before].oid, main[3]);
    git(&dir, &["worktree", "remove", "--force", &wt.display().to_string()]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_page_for_another_history_is_refused() {
    let (dir, _) = branchy("more-other");
    let a = load(&dir, &LoadOptions::all(10)).unwrap();
    let mut b = load(&dir, &LoadOptions::all(10)).unwrap();
    let before = std::sync::Arc::clone(&b.commits);
    let p = page(&dir, &a, 30);
    let back = b.extend_history(p).err().expect("refused");
    assert_eq!(back.added(), 20);
    assert!(std::sync::Arc::ptr_eq(&before, &b.commits));
    assert_eq!((b.commits.len(), b.graph.rows.len(), b.loaded_with.max_commits), (10, 10, 10));
    let _ = std::fs::remove_dir_all(&dir);
}
