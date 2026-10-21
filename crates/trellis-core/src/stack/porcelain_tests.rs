//! The stack push's command lines and the reading of `git push --porcelain` (pure).

use super::porcelain::{commands, parse};
use super::push::{PushPlan, PushTarget};
use git2::Oid;

fn target(branch: &str, remote: &str, dest: &str, set_upstream: bool, seen: Option<Oid>) -> PushTarget {
    PushTarget { branch: branch.into(), remote: remote.into(), dest: dest.into(), set_upstream, seen }
}

fn oid(n: u8) -> Oid {
    Oid::from_bytes(&[n; 20]).unwrap()
}

#[test]
fn layers_are_grouped_by_remote_and_upstream_in_first_seen_order() {
    let low = target("low", "origin", "low", true, None);
    let mid = target("mid", "origin", "mid", false, Some(oid(1)));
    let top = target("top", "origin", "top", true, None);
    let fork = target("side", "fork", "side-x", false, Some(oid(2)));
    let layers = [(0, &low, PushPlan::New), (1, &mid, PushPlan::Rewritten), (2, &top, PushPlan::New), (3, &fork, PushPlan::FastForward)];
    let cmds = commands(&layers);
    assert_eq!(cmds.len(), 3, "{cmds:?}");
    assert_eq!(
        cmds[0],
        (
            ["push", "--porcelain", "-u", "origin", "refs/heads/low:refs/heads/low", "refs/heads/top:refs/heads/top"]
                .map(String::from)
                .to_vec(),
            vec![0, 2]
        )
    );
    let lease = format!("--force-with-lease=refs/heads/mid:{}", oid(1));
    assert_eq!(
        cmds[1],
        (vec!["push".into(), "--porcelain".into(), lease, "origin".into(), "refs/heads/mid:refs/heads/mid".into()], vec![1])
    );
    assert_eq!(cmds[2], (["push", "--porcelain", "fork", "refs/heads/side:refs/heads/side-x"].map(String::from).to_vec(), vec![3]));
    assert!(!cmds.iter().flat_map(|(c, _)| c).any(|a| a == "--force" || a.starts_with('+') || a == "--force-with-lease"));
}

#[test]
fn only_rewritten_layers_get_a_lease() {
    let a = target("a", "origin", "a", false, Some(oid(1)));
    let b = target("b", "origin", "b", false, Some(oid(2)));
    let cmds = commands(&[(0, &a, PushPlan::FastForward), (1, &b, PushPlan::Rewritten)]);
    let leases: Vec<&String> = cmds[0].0.iter().filter(|x| x.starts_with("--force-with-lease")).collect();
    assert_eq!(leases, [&format!("--force-with-lease=refs/heads/b:{}", oid(2))]);
}

#[test]
fn layers_that_dont_push_are_left_out() {
    let t = target("a", "origin", "a", false, Some(oid(1)));
    for plan in [PushPlan::UpToDate, PushPlan::Behind, PushPlan::Diverged, PushPlan::Gone] {
        assert!(commands(&[(0, &t, plan)]).is_empty(), "{plan:?}");
    }
    let n = target("n", "origin", "n", true, None);
    let cmds = commands(&[(0, &t, PushPlan::Behind), (1, &n, PushPlan::New)]);
    assert_eq!(cmds.len(), 1);
    assert_eq!(cmds[0].1, vec![1]);
}

#[test]
fn porcelain_ref_lines_are_read_and_the_rest_skipped() {
    let out = "To /srv/origin.git\n\
               *\trefs/heads/a:refs/heads/a\t[new branch]\n\
               \x20\trefs/heads/m:refs/heads/m\t1111111..2222222\n\
               +\trefs/heads/f:refs/heads/f\t1111111...3333333 (forced update)\n\
               =\trefs/heads/u:refs/heads/u\t[up to date]\n\
               !\trefs/heads/b:refs/heads/b\t[rejected] (stale info)\n\
               !\trefs/heads/c:refs/heads/c-remote\t[remote rejected] (pre-receive hook declined)\n\
               branch 'a' set up to track 'origin/a'.\n\
               \n\
               Done\n";
    let lines = parse(out);
    let flags: Vec<char> = lines.iter().map(|l| l.0).collect();
    assert_eq!(flags, ['*', ' ', '+', '=', '!', '!']);
    assert_eq!(lines[0], ('*', "refs/heads/a".into(), "[new branch]".into()));
    assert_eq!(lines[4].2, "[rejected] (stale info)");
    assert_eq!(lines[5], ('!', "refs/heads/c-remote".into(), "[remote rejected] (pre-receive hook declined)".into()));
}
