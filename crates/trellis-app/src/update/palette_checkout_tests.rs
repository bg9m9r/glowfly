//! Headless tests for the palette's remote branch and tag checkouts: which refs are offered,
//! what running one does, the order and keywords of ref entries, and the row cap that keeps a
//! repository with thousands of refs responsive.

use super::branch_tests::with_origin;
use super::palette_tests::repo_with_topic;
use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::palette::{Entry, MAX_COMMAND_ROWS, PaletteCommand as C, entries, listed, shown};
use std::path::Path;
use std::time::Instant;
use trellis_core::gitcli::args;
use trellis_core::undo::UndoKind;

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn labels(st: &AppState, query: &str) -> Vec<String> {
    shown(st, query).into_iter().map(|s| s.entry.label).collect()
}

fn find(st: &AppState, label: &str) -> Option<Entry> {
    shown(st, "").into_iter().map(|s| s.entry).find(|e| e.label == label)
}

fn open_palette(st: &mut AppState) {
    run(st, Msg::TogglePalette);
    assert!(st.palette.is_some());
}

/// The commands of the single `RunGit` in `fx`, and what it records for undo.
fn run_git(fx: &[Effect]) -> (Vec<Vec<String>>, Option<UndoKind>) {
    match fx {
        [Effect::RunGit { cmds, record, .. }] => (cmds.clone(), record.clone()),
        other => panic!("expected one RunGit, got {other:?}"),
    }
}

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

/// `with_origin` plus remote branches `other` (untracked) and `shared` (tracked by local
/// `mine`), and origin/HEAD set.
fn with_more_remotes(name: &str) -> (AppState, std::path::PathBuf, std::path::PathBuf) {
    let (mut st, work, bare) = with_origin(name);
    git(&work, &["push", "-q", "origin", "main:other"]);
    git(&work, &["push", "-q", "origin", "main:shared"]);
    git(&work, &["branch", "-q", "--track", "mine", "origin/shared"]);
    git(&work, &["remote", "set-head", "origin", "main"]);
    git(&work, &["fetch", "-q", "origin"]);
    drive(&mut st, Msg::Refresh);
    (st, work, bare)
}

#[test]
fn remote_entries_skip_local_tracked_and_head() {
    let (st, work, bare) = with_more_remotes("pal-co-remote");
    let other = find(&st, "Checkout origin/other").expect("origin/other offered");
    assert_eq!(other.detail.as_deref(), Some("creates local branch"));
    assert_eq!(other.command, C::CheckoutRemote { remote: "origin".into(), branch: "other".into() });
    let all = labels(&st, "");
    for gone in ["Checkout origin/main", "Checkout origin/feature", "Checkout origin/shared", "Checkout origin/HEAD"] {
        assert!(!all.iter().any(|l| l == gone), "{gone} should not be listed: {all:?}");
    }
    cleanup(&[&work, &bare]);
}

#[test]
fn remote_entry_runs_checkout_track() {
    let (mut st, work, bare) = with_more_remotes("pal-co-track");
    let cmd = C::CheckoutRemote { remote: "origin".into(), branch: "other".into() };
    open_palette(&mut st);
    let fx = run(&mut st, Msg::RunPaletteCommand(cmd.clone()));
    assert!(st.palette.is_none());
    let (cmds, record) = run_git(&fx);
    assert_eq!(cmds, vec![args(&["checkout", "--track", "origin/other"])]);
    assert_eq!(record, Some(UndoKind::Checkout));

    let mut st = state();
    drive(&mut st, Msg::Open(work.clone()));
    drive(&mut st, Msg::RunPaletteCommand(cmd));
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.head_branch.as_deref(), Some("other"));
    assert_eq!(snap.head_info().and_then(|b| b.upstream.as_deref()), Some("origin/other"));
    assert!(find(&st, "Checkout origin/other").is_none());
    cleanup(&[&work, &bare]);
}

#[test]
fn remote_name_with_slash() {
    let (mut st, dir) = repo_with_topic("pal-co-slash");
    let snap = st.snap.as_mut().unwrap();
    let oid = snap.head_oid.unwrap();
    snap.remotes = vec![("team/fork".into(), vec![("fix/x".into(), oid), ("topic".into(), oid)])];
    let e = find(&st, "Checkout team/fork/fix/x").expect("listed");
    let cmd = C::CheckoutRemote { remote: "team/fork".into(), branch: "fix/x".into() };
    assert_eq!(e.command, cmd);
    assert!(find(&st, "Checkout team/fork/topic").is_none(), "local topic exists");
    let (cmds, _) = run_git(&run(&mut st, Msg::RunPaletteCommand(cmd)));
    assert_eq!(cmds, vec![args(&["checkout", "--track", "team/fork/fix/x"])]);
    cleanup(&[&dir]);
}

#[test]
fn tag_entries_check_out_detached() {
    let dir = scratch_repo("pal-co-tags");
    git(&dir, &["commit", "-qam", "second"]);
    git(&dir, &["tag", "v1.0", "HEAD~1"]);
    git(&dir, &["tag", "-a", "v2.0", "-m", "rel"]);
    let repo = git2::Repository::open(&dir).unwrap();
    let first = repo.revparse_single("HEAD~1").unwrap().id();
    let head = repo.revparse_single("HEAD").unwrap().id();
    let mut st = state();
    drive(&mut st, Msg::Open(dir.clone()));

    let v1 = find(&st, "Checkout tag v1.0").expect("v1.0");
    assert_eq!(v1.detail, Some(format!("detached at {}", &first.to_string()[..7])));
    // The annotated tag shows the commit it points at, not the tag object.
    let v2 = find(&st, "Checkout tag v2.0").expect("v2.0 at HEAD, listed while on a branch");
    assert_eq!(v2.detail, Some(format!("detached at {}", &head.to_string()[..7])));

    let (cmds, record) = run_git(&run(&mut st, Msg::RunPaletteCommand(C::CheckoutTag("v1.0".into()))));
    assert_eq!(cmds, vec![args(&["checkout", "refs/tags/v1.0"])]);
    assert_eq!(record, Some(UndoKind::Checkout));
    st.job = None;

    drive(&mut st, Msg::RunPaletteCommand(C::CheckoutTag("v1.0".into())));
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.head_branch, None);
    assert_eq!(snap.head_oid, Some(first));
    assert!(find(&st, "Checkout tag v1.0").is_none(), "already detached there");
    assert!(find(&st, "Checkout tag v2.0").is_some());
    cleanup(&[&dir]);
}

/// `repo_with_topic` with a remote branch origin/x, a tag v1.0 and a recent repository.
fn with_synthetic_refs(name: &str) -> (AppState, std::path::PathBuf) {
    let (mut st, dir) = repo_with_topic(name);
    st.recent = vec!["C:/elsewhere/other-repo".into()];
    let snap = st.snap.as_mut().unwrap();
    let oid = snap.head_oid.unwrap();
    snap.remotes = vec![("origin".into(), vec![("x".into(), oid)])];
    snap.tags = vec![("v1.0".into(), oid)];
    (st, dir)
}

#[test]
fn ref_entries_order_and_keywords() {
    let (st, dir) = with_synthetic_refs("pal-co-order");
    let all = labels(&st, "");
    let at = |label: &str| all.iter().position(|l| l == label).unwrap_or_else(|| panic!("{label} missing: {all:?}"));
    assert!(at("Checkout topic") < at("Open other-repo"));
    assert!(at("Open other-repo") < at("Checkout origin/x"));
    assert!(at("Checkout origin/x") < at("Checkout tag v1.0"));

    let first = |q: &str| labels(&st, q).into_iter().next();
    assert_eq!(first("detached").as_deref(), Some("Checkout tag v1.0"));
    assert_eq!(first("track").as_deref(), Some("Checkout origin/x"));
    assert_eq!(first("v1").as_deref(), Some("Checkout tag v1.0"));
    assert!(labels(&st, "x").iter().any(|l| l == "Checkout origin/x"));
    let create = labels(&st, "create");
    assert!(!create.is_empty() && !create.iter().any(|l| l.starts_with("Checkout")), "{create:?}");
    cleanup(&[&dir]);
}

#[test]
fn many_refs_are_capped() {
    let (mut st, dir) = repo_with_topic("pal-co-many");
    let snap = st.snap.as_mut().unwrap();
    let oid = snap.head_oid.unwrap();
    snap.tags = (0..3000).map(|i| (format!("v{i}"), oid)).collect();
    snap.remotes = vec![("origin".into(), (0..3000).map(|i| (format!("r{i}"), oid)).collect())];

    let rows = shown(&st, "");
    assert_eq!(rows.len(), MAX_COMMAND_ROWS);
    let all = entries(&st);
    assert_eq!(listed(&st, "").total, all.len());
    assert!(rows.iter().any(|s| s.entry.label == "Push"));
    for e in all.iter().filter(|e| !matches!(e.command, C::CheckoutRemote { .. } | C::CheckoutTag(_))) {
        assert!(rows.iter().any(|s| s.entry.command == e.command), "{} pushed out by refs", e.label);
    }
    assert_eq!(labels(&st, "v2999").first().map(String::as_str), Some("Checkout tag v2999"));

    open_palette(&mut st);
    run(&mut st, Msg::PaletteMove(-1));
    assert_eq!(st.palette.as_ref().unwrap().selected, MAX_COMMAND_ROWS - 1);
    cleanup(&[&dir]);
}
