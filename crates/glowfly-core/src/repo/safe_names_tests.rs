//! A repository whose config and refs hold names git would read as options: the load leaves
//! them out, and no command built from what it keeps has a name in option position.

use super::*;
use crate::testkit::git_out;
use crate::testutil::*;

/// The names a repository supplies, as the snapshot holds them.
fn supplied_names(s: &Snapshot) -> Vec<String> {
    let mut out: Vec<String> = s.remotes.iter().map(|(r, _)| r.clone()).collect();
    for (r, branches) in &s.remotes {
        out.extend(branches.iter().map(|(b, _)| b.clone()));
        out.extend(branches.iter().map(|(b, _)| format!("{r}/{b}")));
    }
    for b in &s.locals {
        out.push(b.name.clone());
        out.extend(b.tracking.iter().flat_map(|t| [t.remote.clone(), t.merge.clone()]));
        out.extend(b.upstream.clone());
    }
    out.extend(s.remote_info.iter().map(|r| r.name.clone()));
    out.extend(s.remote_heads.iter().flat_map(|(r, b)| [r.clone(), b.clone()]));
    out.extend(s.tags.iter().map(|(t, _)| t.clone()));
    out.extend(s.refs.values().flatten().map(|l| l.name.clone()));
    out.extend(s.worktrees.iter().filter_map(|w| w.branch.clone()));
    out.extend(s.head_branch.clone());
    out
}

/// Fails when an argument before the command's `--` (or anywhere, without one) after the
/// subcommand is one of `names`: git could parse it as an option.
fn assert_not_in_option_position(cmd: &[String], names: &[String]) {
    let end = cmd.iter().position(|a| a == "--").unwrap_or(cmd.len());
    for a in &cmd[1..end] {
        assert!(!names.contains(a), "{a:?} is in option position in {cmd:?}");
    }
}

#[test]
fn names_that_read_as_options_never_load_or_reach_a_command() {
    let (work, bare) = with_origin("safe-names-load");
    let url = bare.display().to_string();
    set_config(
        &work,
        &[
            ("remote.-x.url", url.as_str()),
            ("remote.-x.fetch", "+refs/heads/*:refs/remotes/-x/*"),
            ("remote.--upload-pack=evil.url", url.as_str()),
            ("branch.main.remote", "-x"),
            ("branch.main.merge", "refs/heads/main"),
        ],
    );
    let head = git_out(&work, &["rev-parse", "HEAD"]);
    let head = head.trim();
    for r in
        ["refs/heads/-b", "refs/heads/team/--force", "refs/heads/side", "refs/tags/-t", "refs/remotes/-x/main", "refs/remotes/origin/-r"]
    {
        git(&work, &["update-ref", r, head]);
    }
    git(&work, &["config", "branch.side.remote", "origin"]);
    git(&work, &["config", "branch.side.merge", "refs/heads/-r"]);

    let s = load(&work, &LoadOptions::all(10)).unwrap();
    let names = supplied_names(&s);
    for n in &names {
        assert!(n.split('/').all(|p| !p.starts_with('-')), "{n:?} was loaded");
    }
    let remotes: Vec<&str> = s.remotes.iter().map(|(r, _)| r.as_str()).collect();
    assert_eq!(remotes, ["origin"]);
    let locals: Vec<&str> = s.locals.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(locals, ["main", "side"]);
    assert!(s.locals.iter().all(|b| b.tracking.is_none() && b.upstream.is_none()), "an upstream on -x or of -r is dropped");
    assert!(s.tags.is_empty());
    assert_eq!(s.head_branch.as_deref(), Some("main"));

    // Every builder these names reach.
    use crate::{branch_ops, remote_ops, tag_ops};
    let mut cmds: Vec<Vec<String>> = Vec::new();
    for (r, branches) in &s.remotes {
        cmds.push(remote_ops::fetch_cmd(r));
        cmds.push(remote_ops::fetch_ref_cmd(r, "refs/pull/1/head", false));
        cmds.push(remote_ops::remove_cmd(r));
        cmds.extend(crate::partial::fetch_cmds(r, &[git2::Oid::zero()]));
        cmds.push(crate::shallow_ops::deepen_cmd(r, crate::shallow_ops::Deepen::Full));
        for (b, oid) in branches {
            cmds.extend(branch_ops::delete_remote_cmds(r, b, std::slice::from_ref(b)));
            cmds.push(branch_ops::merge_cmd(&format!("{r}/{b}")));
            cmds.push(branch_ops::rebase_cmd(&format!("{r}/{b}")));
            cmds.extend(branch_ops::set_upstream_cmds("main", r, b, true));
            cmds.extend(branch_ops::rename_with_remote_cmds("main", "next", r, b, *oid));
        }
        for b in &s.locals {
            cmds.push(branch_ops::push_cmd(&b.name, r, &b.name, true));
            let t = Tracking { remote: r.clone(), merge: b.name.clone() };
            cmds.extend(branch_ops::delete_with_remote_cmds(&b.name, &t));
        }
        for (t, _) in &s.tags {
            cmds.push(tag_ops::push_tag_cmd(r, t));
            cmds.push(tag_ops::delete_remote_tag_cmd(r, t));
        }
    }
    for b in &s.locals {
        cmds.push(branch_ops::merge_cmd(&b.name));
        cmds.push(branch_ops::rebase_cmd(&b.name));
        cmds.push(branch_ops::rename_cmd(&b.name, "renamed"));
        cmds.push(branch_ops::unset_upstream_cmd(&b.name));
        cmds.push(crate::journal::restore::create_branch(&b.name, git2::Oid::zero()));
    }
    for c in &cmds {
        assert_not_in_option_position(c, &names);
    }
    let _ = std::fs::remove_dir_all(&work);
    let _ = std::fs::remove_dir_all(&bare);
}

#[test]
fn a_checked_out_branch_that_reads_as_an_option_shows_as_detached() {
    let work = scratch_repo("safe-names-head");
    write(&work, "a.txt", "one\n");
    git(&work, &["add", "."]);
    git(&work, &["commit", "-q", "-m", "first"]);
    git(&work, &["update-ref", "refs/heads/-b", "HEAD"]);
    git(&work, &["symbolic-ref", "HEAD", "refs/heads/-b"]);
    let s = load(&work, &LoadOptions::all(10)).unwrap();
    assert_eq!(s.head_branch, None);
    assert!(s.locals.iter().all(|b| b.name != "-b" && !b.is_head));
    assert!(s.refs.values().flatten().any(|l| l.kind == RefKind::Head), "HEAD is labelled as detached");
    let _ = std::fs::remove_dir_all(&work);
}

/// Each builder that takes a repository-supplied name ends its options before the name.
#[test]
fn every_builder_ends_options_before_a_supplied_name() {
    use crate::{branch_ops, remote_ops, tag_ops};
    let evil = "-evil".to_owned();
    let names = [evil.clone()];
    let e = evil.as_str();
    let edit = remote_ops::RemoteEdit { name: e, url: e, push_url: Some(e) };
    let old = RemoteInfo {
        name: e.into(),
        urls: vec!["u1".into(), "u2".into()],
        push_urls: vec!["p1".into(), "p2".into()],
        ..Default::default()
    };
    let renamed = RemoteInfo { name: "old".into(), ..old.clone() };
    let mut cmds = vec![
        remote_ops::fetch_cmd(e),
        remote_ops::fetch_ref_cmd(e, e, true),
        remote_ops::remove_cmd(e),
        crate::shallow_ops::deepen_cmd(e, crate::shallow_ops::Deepen::By(3)),
        branch_ops::push_cmd(e, e, e, true),
        branch_ops::merge_cmd(e),
        branch_ops::rebase_cmd(e),
        branch_ops::rename_cmd(e, e),
        branch_ops::unset_upstream_cmd(e),
        tag_ops::push_tag_cmd(e, e),
        tag_ops::delete_remote_tag_cmd(e, e),
        tag_ops::annotate_tag_cmd(e, "0123456", "message"),
        crate::journal::restore::create_branch(e, git2::Oid::zero()),
    ];
    cmds.extend(remote_ops::add_cmds(e, e, Some(e), true));
    cmds.extend(remote_ops::edit_cmds(&old, &edit));
    cmds.extend(remote_ops::edit_cmds(&renamed, &remote_ops::RemoteEdit { push_url: None, ..edit }));
    cmds.extend(crate::partial::fetch_cmds(e, &[git2::Oid::zero()]));
    cmds.extend(branch_ops::set_upstream_cmds(e, "origin", "x", true));
    cmds.extend(branch_ops::delete_with_remote_cmds(e, &Tracking { remote: e.into(), merge: e.into() }));
    cmds.extend(branch_ops::rename_with_remote_cmds(e, e, e, e, git2::Oid::zero()));
    cmds.extend(branch_ops::delete_remote_cmds(e, e, std::slice::from_ref(&evil)));
    for c in &cmds {
        // The sequences' own checks and lease options name the branch inside a ref or a value.
        let positional: Vec<String> = c.iter().filter(|a| *a == e || !a.contains(e)).cloned().collect();
        assert_not_in_option_position(&positional, &names);
        assert!(c.contains(&evil) || c.iter().any(|a| a.contains(e)), "{c:?} doesn't use the name");
    }
}
