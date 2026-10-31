use super::*;
use crate::gitcli::run;
use crate::repo::{LoadOptions, load};
use crate::testutil::*;
use git2::{Oid, Repository};
use std::path::Path;

fn branch(name: &str, ahead: usize, behind: usize, tracked: bool, upstream: bool) -> BranchInfo {
    BranchInfo {
        name: name.into(),
        oid: None,
        is_head: false,
        upstream: upstream.then(|| format!("origin/{name}")),
        ahead,
        behind,
        tracking: tracked.then(|| Tracking { remote: "origin".into(), merge: name.into() }),
        upstream_ref: upstream.then(|| format!("refs/remotes/origin/{name}")),
        merged: true,
    }
}

#[test]
fn branch_names_are_checked() {
    let existing = ["main", "feature/a", "Login"];
    let check = |n: &str| check_branch_name(n, &existing, None);
    assert_eq!(check(""), Err(NameProblem::Empty));
    for bad in ["a b", "a..b", "-x", "x.lock", "x/", "HEAD", "a~1"] {
        assert_eq!(check(bad), Err(NameProblem::Invalid), "{bad}");
    }
    assert_eq!(check("main"), Err(NameProblem::Exists("main".into())));
    assert_eq!(check("login"), Err(NameProblem::Exists("Login".into())), "case-only variant");
    assert_eq!(check("feature"), Err(NameProblem::Clashes("feature/a".into())));
    assert_eq!(check("feature/a/b"), Err(NameProblem::Clashes("feature/a".into())));
    assert_eq!(check("feature/new-ui"), Ok(()));
    assert_eq!(check_branch_name("LOGIN", &existing, Some("Login")), Ok(()), "renaming ignores the branch itself");
    assert_eq!(NameProblem::Clashes("feature/a".into()).message("feature"), "feature clashes with branch feature/a.");
}

#[test]
fn fast_forward_states_and_commands() {
    assert_eq!(fast_forward_state(&branch("x", 0, 0, false, false)), FastForward::NoUpstream);
    assert_eq!(fast_forward_state(&branch("x", 0, 0, true, false)), FastForward::UpstreamGone);
    assert_eq!(fast_forward_state(&branch("x", 0, 0, true, true)), FastForward::UpToDate);
    assert_eq!(fast_forward_state(&branch("x", 2, 0, true, true)), FastForward::Ahead(2));
    assert_eq!(fast_forward_state(&branch("x", 1, 2, true, true)), FastForward::Diverged { ahead: 1, behind: 2 });
    let ready = branch("x", 0, 3, true, true);
    assert_eq!(fast_forward_state(&ready), FastForward::Ready { behind: 3 });

    for b in [branch("x", 0, 0, true, true), branch("x", 2, 0, true, true), branch("x", 1, 2, true, true), branch("x", 0, 0, true, false)] {
        assert_eq!(fast_forward_cmd(&b), None);
    }
    assert_eq!(fast_forward_cmd(&ready), Some(args(&["fetch", "--no-recurse-submodules", ".", "refs/remotes/origin/x:refs/heads/x"])));
    let head = BranchInfo { is_head: true, ..ready };
    assert_eq!(fast_forward_cmd(&head), Some(args(&["merge", "--ff-only", "refs/remotes/origin/x"])));
}

#[test]
fn command_builders() {
    assert_eq!(rename_cmd("a", "b"), args(&["branch", "-m", "--", "a", "b"]));
    assert_eq!(unset_upstream_cmd("a"), args(&["branch", "--unset-upstream", "--", "a"]));
    assert_eq!(set_upstream_cmds("a", "origin", "b", true), vec![args(&["branch", "--set-upstream-to=origin/b", "--", "a"])]);
    assert_eq!(
        set_upstream_cmds("a", "origin", "b", false),
        vec![args(&["config", "branch.a.remote", "origin"]), args(&["config", "branch.a.merge", "refs/heads/b"])]
    );
    let t = Tracking { remote: "origin".into(), merge: "other".into() };
    assert_eq!(
        delete_with_remote_cmds("a", &t),
        vec![
            args(&["merge-base", "--is-ancestor", "refs/heads/a", "HEAD"]),
            args(&["branch", "-d", "--", "a"]),
            args(&["push", "--delete", "--", "origin", "other"])
        ]
    );
    assert_eq!(push_cmd("a", "origin", "b", true), args(&["push", "-u", "--", "origin", "refs/heads/a:refs/heads/b"]));
    assert_eq!(push_cmd("a", "backup", "a", false), args(&["push", "--", "backup", "refs/heads/a:refs/heads/a"]));
    assert_eq!(t.display(), "origin/other");
    assert_eq!(Tracking { remote: ".".into(), merge: "main".into() }.display(), "main");
}

// ---- against real repositories

/// Runs each command in order, stopping at the first failure (as the app's job runner does).
fn run_all(dir: &Path, cmds: &[Vec<String>]) -> bool {
    cmds.iter().all(|c| run(dir, c).0)
}

fn ref_oid(dir: &Path, name: &str) -> Option<Oid> {
    Repository::open(dir).unwrap().refname_to_id(name).ok()
}

fn info(dir: &Path, name: &str) -> BranchInfo {
    let s = load(dir, &LoadOptions::all(100)).unwrap();
    s.locals.into_iter().find(|b| b.name == name).unwrap_or_else(|| panic!("no branch {name}"))
}

fn config(dir: &Path, key: &str) -> Option<String> {
    Repository::open(dir).unwrap().config().unwrap().snapshot().unwrap().get_string(key).ok()
}

/// Adds a commit on top of `branch` without checking it out.
fn local_commit(dir: &Path, branch: &str, msg: &str) {
    let (ok, oid) = run(dir, &args(&["commit-tree", &format!("{branch}^{{tree}}"), "-p", branch, "-m", msg]));
    assert!(ok, "{oid}");
    git(dir, &["update-ref", &format!("refs/heads/{branch}"), oid.trim()]);
}

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn load_fills_tracking_and_upstream_ref() {
    let (work, bare) = with_origin("bops-load");
    git(&work, &["branch", "feature"]);
    git(&work, &["push", "-q", "-u", "origin", "feature"]);
    let b = info(&work, "feature");
    assert_eq!(b.tracking, Some(Tracking { remote: "origin".into(), merge: "feature".into() }));
    assert_eq!(b.upstream_ref.as_deref(), Some("refs/remotes/origin/feature"));
    assert_eq!(b.upstream.as_deref(), Some("origin/feature"));

    git(&work, &["update-ref", "-d", "refs/remotes/origin/feature"]);
    let b = info(&work, "feature");
    assert_eq!((&b.upstream, &b.upstream_ref), (&None, &None));
    assert!(b.tracking.is_some(), "the configured upstream stays");
    assert_eq!(fast_forward_state(&b), FastForward::UpstreamGone);

    git(&work, &["branch", "--track", "loc", "main"]);
    let b = info(&work, "loc");
    assert_eq!(b.tracking, Some(Tracking { remote: ".".into(), merge: "main".into() }));
    assert_eq!(b.upstream_ref.as_deref(), Some("refs/heads/main"));

    git(&work, &["push", "-q", "origin", "main:other"]);
    git(&work, &["branch", "x"]);
    git(&work, &["branch", "-u", "origin/other", "x"]);
    assert_eq!(info(&work, "x").tracking.map(|t| t.merge), Some("other".into()));
    cleanup(&[&work, &bare]);
}

#[test]
fn fast_forward_without_checkout() {
    let (work, bare) = with_origin("bops-ff");
    git(&work, &["branch", "feature"]);
    git(&work, &["push", "-q", "-u", "origin", "feature"]);
    remote_commit(&bare, "feature", "f1.txt", "f1");
    remote_commit(&bare, "feature", "f2.txt", "f2");
    git(&work, &["fetch", "-q", "origin"]);
    write(&work, "a.txt", "dirty\n");

    let b = info(&work, "feature");
    assert_eq!(fast_forward_state(&b), FastForward::Ready { behind: 2 });
    let cmd = fast_forward_cmd(&b).unwrap();
    assert!(run(&work, &cmd).0);
    assert_eq!(ref_oid(&work, "refs/heads/feature"), ref_oid(&work, "refs/remotes/origin/feature"));
    let s = load(&work, &LoadOptions::all(100)).unwrap();
    assert_eq!(s.head_branch.as_deref(), Some("main"), "no checkout");
    assert_eq!(std::fs::read_to_string(work.join("a.txt")).unwrap(), "dirty\n");

    // Diverged: git refuses the same command and leaves the branch alone.
    local_commit(&work, "feature", "local");
    remote_commit(&bare, "feature", "f3.txt", "f3");
    git(&work, &["fetch", "-q", "origin"]);
    let before = ref_oid(&work, "refs/heads/feature");
    let b = info(&work, "feature");
    assert_eq!(fast_forward_state(&b), FastForward::Diverged { ahead: 1, behind: 1 });
    assert_eq!(fast_forward_cmd(&b), None);
    assert!(!run(&work, &cmd).0, "not a fast-forward");
    assert_eq!(ref_oid(&work, "refs/heads/feature"), before);

    // The checked-out branch merges with --ff-only.
    remote_commit(&bare, "main", "m.txt", "m");
    git(&work, &["fetch", "-q", "origin"]);
    let b = info(&work, "main");
    let cmd = fast_forward_cmd(&b).unwrap();
    assert_eq!(cmd[0], "merge");
    assert!(run(&work, &cmd).0);
    assert_eq!(ref_oid(&work, "refs/heads/main"), ref_oid(&work, "refs/remotes/origin/main"));
    cleanup(&[&work, &bare]);
}

/// The fast-forward a remote branch checkout runs first: a strict fast-forward, leased on the
/// branch's tip as the snapshot saw it.
#[test]
fn fast_forward_ref_only_moves_forward() {
    let (work, bare) = with_origin("bops-ff-ref");
    git(&work, &["branch", "feature"]);
    git(&work, &["push", "-q", "-u", "origin", "feature"]);
    remote_commit(&bare, "feature", "f1.txt", "f1");
    git(&work, &["fetch", "-q", "origin"]);
    let (from, to) = (ref_oid(&work, "refs/heads/feature").unwrap(), ref_oid(&work, "refs/remotes/origin/feature").unwrap());
    let cmds = fast_forward_ref_cmds("feature", from, to, "origin/feature");
    assert_eq!(cmds[0], args(&["merge-base", "--is-ancestor", &from.to_string(), &to.to_string()]));
    assert!(run_all(&work, &cmds));
    assert_eq!(ref_oid(&work, "refs/heads/feature"), Some(to));

    // Moved since (the lease no longer holds): nothing changes.
    local_commit(&work, "feature", "local");
    let moved = ref_oid(&work, "refs/heads/feature");
    assert!(!run_all(&work, &fast_forward_ref_cmds("feature", to, to, "origin/feature")[1..]), "update-ref leased on the old tip");
    assert_eq!(ref_oid(&work, "refs/heads/feature"), moved);
    // Diverged: the ancestry check stops it before update-ref.
    remote_commit(&bare, "feature", "f2.txt", "f2");
    git(&work, &["fetch", "-q", "origin"]);
    let remote = ref_oid(&work, "refs/remotes/origin/feature").unwrap();
    assert!(!run_all(&work, &fast_forward_ref_cmds("feature", moved.unwrap(), remote, "origin/feature")));
    assert_eq!(ref_oid(&work, "refs/heads/feature"), moved);
    cleanup(&[&work, &bare]);
}

#[test]
fn create_tracking_branch_under_another_name() {
    let (work, bare) = with_origin("bops-track-as");
    git(&work, &["push", "-q", "origin", "main:feature"]);
    git(&work, &["fetch", "-q", "origin"]);
    assert_eq!(
        create_tracking_cmd("mine", "origin", "feature", false),
        args(&["branch", "--track", "mine", "refs/remotes/origin/feature"])
    );
    assert!(run(&work, &create_tracking_cmd("mine", "origin", "feature", false)).0);
    assert_eq!(info(&work, "mine").tracking, Some(Tracking { remote: "origin".into(), merge: "feature".into() }));
    assert!(run(&work, &create_tracking_cmd("also", "origin", "feature", true)).0);
    let s = load(&work, &LoadOptions::all(100)).unwrap();
    assert_eq!(s.head_branch.as_deref(), Some("also"));
    assert_eq!(info(&work, "also").upstream.as_deref(), Some("origin/feature"));
    cleanup(&[&work, &bare]);
}

#[test]
fn rename_moves_the_upstream_config() {
    let (work, bare) = with_origin("bops-rename");
    git(&work, &["branch", "feature"]);
    git(&work, &["push", "-q", "-u", "origin", "feature"]);
    assert!(run(&work, &rename_cmd("feature", "feature-2")).0);
    assert_eq!(config(&work, "branch.feature-2.remote").as_deref(), Some("origin"));
    assert_eq!(config(&work, "branch.feature-2.merge").as_deref(), Some("refs/heads/feature"));
    assert_eq!(config(&work, "branch.feature.remote"), None);
    assert_eq!(config(&work, "branch.feature.merge"), None);

    assert!(run(&work, &rename_cmd("main", "trunk")).0);
    let s = load(&work, &LoadOptions::all(100)).unwrap();
    assert_eq!(s.head_branch.as_deref(), Some("trunk"));
    cleanup(&[&work, &bare]);
}

#[test]
fn set_and_unset_upstream() {
    let (work, bare) = with_origin("bops-upstream");
    git(&work, &["push", "-q", "origin", "main:feature"]);
    git(&work, &["branch", "x"]);
    assert!(run_all(&work, &set_upstream_cmds("x", "origin", "feature", true)));
    let b = info(&work, "x");
    assert_eq!(b.tracking, Some(Tracking { remote: "origin".into(), merge: "feature".into() }));
    assert_eq!(b.upstream.as_deref(), Some("origin/feature"));

    assert!(run_all(&work, &set_upstream_cmds("x", "origin", "nothere", false)));
    let b = info(&work, "x");
    assert_eq!(b.tracking, Some(Tracking { remote: "origin".into(), merge: "nothere".into() }));
    assert_eq!(b.upstream, None);

    assert!(run(&work, &unset_upstream_cmd("x")).0);
    let b = info(&work, "x");
    assert_eq!((b.tracking, b.upstream), (None, None));
    cleanup(&[&work, &bare]);
}

#[test]
fn delete_local_and_remote() {
    let (work, bare) = with_origin("bops-delete");
    git(&work, &["branch", "feature"]);
    git(&work, &["push", "-q", "-u", "origin", "feature"]);
    let b = info(&work, "feature");
    assert!(b.merged);
    let t = b.tracking.unwrap();
    assert!(run_all(&work, &delete_with_remote_cmds("feature", &t)));
    assert_eq!(ref_oid(&work, "refs/heads/feature"), None);
    assert_eq!(ref_oid(&work, "refs/remotes/origin/feature"), None);
    assert_eq!(ref_oid(&bare, "refs/heads/feature"), None);

    // Unmerged: the local delete fails first and the remote branch stays.
    git(&work, &["branch", "wip"]);
    git(&work, &["push", "-q", "-u", "origin", "wip"]);
    local_commit(&work, "wip", "unpushed");
    let t = info(&work, "wip").tracking.unwrap();
    assert!(!run_all(&work, &delete_with_remote_cmds("wip", &t)));
    assert!(ref_oid(&work, "refs/heads/wip").is_some());
    assert!(ref_oid(&bare, "refs/heads/wip").is_some());

    // Pushed but not merged into HEAD: `branch -d` alone accepts it (it is merged into its
    // upstream), and the remote delete would then leave its commit on no branch.
    git(&work, &["branch", "topic"]);
    local_commit(&work, "topic", "pushed, never merged");
    git(&work, &["push", "-q", "-u", "origin", "topic"]);
    let b = info(&work, "topic");
    assert_eq!((b.ahead, b.behind, b.merged), (0, 0, false));
    let tip = ref_oid(&work, "refs/heads/topic");
    assert!(!run_all(&work, &delete_with_remote_cmds("topic", &b.tracking.unwrap())));
    assert_eq!(ref_oid(&work, "refs/heads/topic"), tip, "the local branch stays");
    assert_eq!(ref_oid(&bare, "refs/heads/topic"), tip, "the remote branch stays");
    cleanup(&[&work, &bare]);
}

#[test]
fn merged_means_head_contains_the_branch() {
    let (work, bare) = with_origin("bops-merged");
    git(&work, &["branch", "behind"]);
    local_commit(&work, "main", "main moves on");
    git(&work, &["branch", "side"]);
    local_commit(&work, "side", "only on side");
    git(&work, &["branch", "local-only", "behind"]);
    git(&work, &["push", "-q", "-u", "origin", "behind", "side"]);
    let s = load(&work, &LoadOptions::all(100)).unwrap();
    let merged = |n: &str| s.locals.iter().find(|b| b.name == n).unwrap().merged;
    assert!(merged("main"), "the checked-out branch");
    assert!(merged("behind"), "an ancestor of HEAD");
    assert!(!merged("side"));
    // Only deleting a branch with its remote branch asks, so others aren't walked.
    assert!(!merged("local-only"), "not worked out without a remote upstream");
    cleanup(&[&work, &bare]);
}

#[test]
fn push_to_chosen_remotes() {
    let (work, bare) = with_origin("bops-push");
    let backup = bare.with_file_name(format!("{}-backup.git", work.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&backup);
    std::fs::create_dir_all(&backup).unwrap();
    git(&backup, &["init", "-q", "--bare", "-b", "main"]);
    git(&work, &["remote", "add", "backup", &backup.display().to_string()]);

    git(&work, &["branch", "new"]);
    assert!(run(&work, &push_cmd("new", "origin", "new", true)).0);
    assert!(ref_oid(&bare, "refs/heads/new").is_some());
    assert_eq!(info(&work, "new").tracking, Some(Tracking { remote: "origin".into(), merge: "new".into() }));

    assert!(run(&work, &push_cmd("new", "backup", "new", false)).0);
    assert!(ref_oid(&backup, "refs/heads/new").is_some());
    assert_eq!(info(&work, "new").tracking.map(|t| t.remote), Some("origin".into()), "the upstream stays on origin");

    git(&work, &["branch", "y"]);
    git(&work, &["push", "-q", "origin", "y:other"]);
    git(&work, &["branch", "-u", "origin/other", "y"]);
    local_commit(&work, "y", "more");
    assert!(run(&work, &push_cmd("y", "origin", "other", false)).0);
    assert_eq!(ref_oid(&bare, "refs/heads/other"), ref_oid(&work, "refs/heads/y"));
    cleanup(&[&work, &bare, &backup]);
}

#[test]
fn branch_folders_are_compared_ignoring_case_and_form() {
    let existing = ["feature/foo", "caf\u{e9}/menu", "main"];
    let check = |n: &str| check_branch_name_for(n, &existing, None, false);
    let folder = |typed: &str, existing: &str| Err(NameProblem::FolderCase { typed: typed.into(), existing: existing.into() });
    assert_eq!(check("Feature/bar"), folder("Feature", "feature"));
    assert_eq!(check("cafe\u{301}/drinks"), folder("cafe\u{301}", "caf\u{e9}"));
    assert_eq!(check("feature/bar"), Ok(()));
    assert_eq!(check("CAFE\u{301}/MENU"), Err(NameProblem::Exists("caf\u{e9}/menu".into())), "case and form at once");
    assert_eq!(check("Main/x"), Err(NameProblem::Clashes("main".into())));
    assert_eq!(check_branch_name_for("Feature/bar", &existing, Some("feature/foo"), false), Ok(()), "renaming the only branch in it");
    assert_eq!(
        NameProblem::FolderCase { typed: "Feature".into(), existing: "feature".into() }.message("Feature/bar"),
        "The folder Feature already exists as feature; use that spelling."
    );
}

#[test]
fn branch_names_windows_cant_store() {
    use crate::names::WinNameProblem::{Char, Device, Trailing};
    for (name, part, why) in [
        ("aux", "aux", Device("aux".into())),
        ("fix/con", "con", Device("con".into())),
        ("lpt1.x", "lpt1.x", Device("lpt1".into())),
        ("a|b", "a|b", Char('|')),
        ("say\"hi\"", "say\"hi\"", Char('"')),
        ("less<more", "less<more", Char('<')),
        ("v1./x", "v1.", Trailing),
    ] {
        assert!(is_valid_branch_name(name), "git takes {name:?}");
        assert_eq!(check_branch_name_for(name, &[], None, true), Err(NameProblem::Windows(part.into(), why.clone())), "{name:?}");
        assert_eq!(check_branch_name_for(name, &[], None, false), Ok(()), "{name:?}");
        assert_eq!(ref_name_warning(name, false), Some(why.warning(name)), "{name:?}");
        assert_eq!(ref_name_warning(name, true), None);
    }
    assert_eq!(branch_name_warning("feature/ok"), None);
    assert_eq!(check_branch_name("aux", &[], None).is_err(), cfg!(windows));
    assert_eq!(
        NameProblem::Windows("aux".into(), Device("aux".into())).message("aux"),
        "aux can't be used: Windows reserves aux for a device."
    );
}

#[test]
fn rename_with_remote_pushes_before_deleting() {
    let tip = git2::Oid::from_str("1111111111111111111111111111111111111111").unwrap();
    assert_eq!(
        rename_with_remote_cmds("feat", "feature", "origin", "feat-old", tip),
        vec![
            args(&["branch", "-m", "--", "feat", "feature"]),
            args(&["push", "-u", "--", "origin", "refs/heads/feature:refs/heads/feature"]),
            args(&["push", &format!("--force-with-lease=refs/heads/feat-old:{tip}"), "--delete", "--", "origin", "feat-old"]),
        ]
    );
    assert_eq!(
        delete_remote_cmds("origin", "x", &["a".into(), "b".into()]),
        vec![
            args(&["push", "--delete", "--", "origin", "x"]),
            args(&["branch", "--unset-upstream", "--", "a"]),
            args(&["branch", "--unset-upstream", "--", "b"])
        ]
    );
}
