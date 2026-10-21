//! Choosing the mainline from a loaded snapshot whose remotes and branches are set by hand.

use super::*;
use crate::repo::{LoadOptions, Snapshot, load};
use crate::testutil::{git, scratch_repo, write};

fn oid(n: u8) -> Oid {
    Oid::from_bytes(&[n; 20]).unwrap()
}

/// A loaded one-commit repository with `remotes` and local branches `locals` put in by hand.
fn snapshot(remotes: &[(&str, &[&str])], heads: &[(&str, &str)], locals: &[&str]) -> Snapshot {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let dir = scratch_repo(&format!("rd-mainline-{}", NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    write(&dir, "a.txt", "a\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    let mut snap = load(&dir, &LoadOptions::all(10)).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let mut n = 1;
    let mut next = || {
        n += 1;
        oid(n)
    };
    snap.remotes = remotes.iter().map(|(r, bs)| (r.to_string(), bs.iter().map(|b| (b.to_string(), next())).collect())).collect();
    snap.remote_heads = heads.iter().map(|(r, b)| (r.to_string(), b.to_string())).collect();
    let template = snap.locals[0].clone();
    snap.locals =
        locals.iter().map(|name| crate::repo::BranchInfo { name: name.to_string(), oid: Some(next()), ..template.clone() }).collect();
    snap
}

fn name(snap: &Snapshot, remote: Option<&str>, exclude: &[&str]) -> Option<String> {
    let m = mainline(snap, remote, exclude)?;
    let expected = snap.local_oid(&m.0).or_else(|| {
        let (r, b) = m.0.split_once('/')?;
        snap.remote_oid(r, b)
    });
    assert_eq!(Some(m.1), expected, "the oid is the mainline's tip");
    Some(m.0)
}

#[test]
fn prefers_the_recorded_head_then_main_then_master() {
    let snap = snapshot(&[("origin", &["develop", "main", "master"])], &[("origin", "develop")], &["main", "master"]);
    assert_eq!(name(&snap, None, &[]).as_deref(), Some("origin/develop"));
    assert_eq!(name(&snap, None, &["origin/develop"]).as_deref(), Some("origin/main"));
    assert_eq!(name(&snap, None, &["origin/develop", "origin/main"]).as_deref(), Some("origin/master"));
    assert_eq!(name(&snap, None, &["origin/develop", "origin/main", "origin/master"]).as_deref(), Some("main"));
    assert_eq!(name(&snap, None, &["origin/develop", "origin/main", "origin/master", "main"]).as_deref(), Some("master"));
    assert_eq!(name(&snap, None, &["origin/develop", "origin/main", "origin/master", "main", "master"]), None);
}

#[test]
fn without_a_recorded_head_main_comes_first() {
    let snap = snapshot(&[("origin", &["master", "main"])], &[], &[]);
    assert_eq!(name(&snap, None, &[]).as_deref(), Some("origin/main"));
    let locals_only = snapshot(&[], &[], &["master", "topic"]);
    assert_eq!(name(&locals_only, None, &[]).as_deref(), Some("master"));
    let nothing = snapshot(&[], &[], &["topic"]);
    assert_eq!(name(&nothing, None, &[]), None);
}

#[test]
fn the_given_remote_then_origin_then_the_first() {
    let snap = snapshot(&[("fork", &["main"]), ("origin", &["main"]), ("up", &["main"])], &[], &[]);
    assert_eq!(name(&snap, Some("up"), &[]).as_deref(), Some("up/main"));
    assert_eq!(name(&snap, Some("gone"), &[]).as_deref(), Some("origin/main"), "an unknown remote falls back to origin");
    assert_eq!(name(&snap, None, &[]).as_deref(), Some("origin/main"));
    let no_origin = snapshot(&[("fork", &["main"]), ("up", &["main"])], &[], &[]);
    assert_eq!(name(&no_origin, None, &[]).as_deref(), Some("fork/main"), "else the first remote");
}
