//! `classify` on git's untranslated failure texts and `with_quiet_apply` on command lines.

use super::{Cause, classify, with_quiet_apply};
use crate::auth_failure::AuthNeed;
use crate::gitcli::args;
use std::borrow::Cow;

fn local(files: &[&str]) -> Option<Cause> {
    Some(Cause::LocalChanges { files: files.iter().map(|f| f.to_string()).collect() })
}

fn s(v: &str) -> String {
    v.to_owned()
}

#[test]
fn local_changes_listed_or_not() {
    let checkout = "error: Your local changes to the following files would be overwritten by checkout:\n\th\nPlease commit your changes or stash them before you switch branches.\nAborting";
    assert_eq!(classify(checkout), local(&["h"]));
    let ff = "$ git pull --ff\nerror: Your local changes to the following files would be overwritten by merge:\n\tf.txt\nPlease commit your changes or stash them before you merge.\nAborting\nUpdating 7aa04be..d9ba930";
    assert_eq!(classify(ff), local(&["f.txt"]));
    // ort's list is indented with two spaces; the restore's patch warnings follow it.
    let ort = "error: Your local changes to the following files would be overwritten by merge:\n  g.txt\n<stdin>:7: trailing whitespace.\nx   \n<stdin>:8: trailing whitespace.\n  y \nwarning: 2 lines add whitespace errors.\nMerge with strategy ort failed.";
    assert_eq!(classify(ort), local(&["g.txt"]));
    // stash pop: the status listing after "Aborting" isn't counted.
    let pop = "error: Your local changes to the following files would be overwritten by merge:\n\tf\nPlease commit your changes or stash them before you merge.\nAborting\nOn branch main\nChanges not staged for commit:\n\tmodified:   f\n\nThe stash entry is kept in case you need it again.";
    assert_eq!(classify(pop), local(&["f"]));
    let quoted = "error: Your local changes to the following files would be overwritten by checkout:\n\t\"caf\\303\\251.txt\"\nAborting";
    assert_eq!(classify(quoted), local(&["café.txt"]));
    for text in [
        "error: cannot pull with rebase: You have unstaged changes.\nerror: Please commit or stash them.",
        "error: cannot rebase: Your index contains uncommitted changes.\nerror: Please commit or stash them.",
        "error: your local changes would be overwritten by cherry-pick.\nhint: commit your changes or stash them to proceed.\nfatal: cherry-pick failed",
    ] {
        assert_eq!(classify(text), local(&[]), "{text}");
    }
}

#[test]
fn untracked_beats_merge_refused() {
    let merge = "error: The following untracked working tree files would be overwritten by merge:\n\tg\nPlease move or remove them before you merge.\nAborting\nMerge with strategy ort failed.";
    assert_eq!(classify(merge), Some(Cause::UntrackedInTheWay { files: vec![s("g")] }));
    let checkout = "error: The following untracked working tree files would be overwritten by checkout:\n\tg\nPlease move or remove them before you switch branches.\nAborting";
    assert_eq!(classify(checkout), Some(Cause::UntrackedInTheWay { files: vec![s("g")] }));
    let removed = "error: The following untracked working tree files would be removed by checkout:\n\tg\n\tk\nAborting";
    assert_eq!(classify(removed), Some(Cause::UntrackedInTheWay { files: vec![s("g"), s("k")] }));
}

#[test]
fn conflicts_unresolved_and_merge_in_progress() {
    let content =
        "Auto-merging f\nCONFLICT (content): Merge conflict in f\nAutomatic merge failed; fix conflicts and then commit the result.";
    assert_eq!(classify(content), Some(Cause::Conflicts { files: vec![s("f")] }));
    let mixed = "CONFLICT (content): Merge conflict in f\nCONFLICT (modify/delete): gone.txt deleted in side and modified in HEAD.  Version HEAD of gone.txt left in tree.\nCONFLICT (content): Merge conflict in f\nAutomatic merge failed; fix conflicts and then commit the result.";
    assert_eq!(classify(mixed), Some(Cause::Conflicts { files: vec![s("f"), s("gone.txt")] }), "deduplicated, in order");
    let rebase = "Auto-merging f\nCONFLICT (content): Merge conflict in f\nerror: could not apply 1a2b3c4... edit f\nhint: Resolve all conflicts manually";
    assert_eq!(classify(rebase), Some(Cause::Conflicts { files: vec![s("f")] }));
    assert_eq!(classify("error: could not apply 1a2b3c4... edit f"), Some(Cause::Conflicts { files: vec![] }));
    assert_eq!(classify("Rebasing (1/1)error: could not apply 1a2b3c4... edit f"), Some(Cause::Conflicts { files: vec![] }));
    for text in [
        "error: Pulling is not possible because you have unmerged files.\nhint: Fix them up in the work tree.\nfatal: Exiting because of an unresolved conflict.",
        "error: Merging is not possible because you have unmerged files.",
        "error: you need to resolve your current index first\nf: needs merge",
    ] {
        assert_eq!(classify(text), Some(Cause::Unresolved), "{text}");
    }
    let in_progress = "fatal: You have not concluded your merge (MERGE_HEAD exists).\nPlease, commit your changes before you merge.";
    assert_eq!(classify(in_progress), Some(Cause::MergeInProgress));
}

#[test]
fn push_and_fetch_rejections() {
    let tail = "error: failed to push some refs to '/tmp/r-origin.git'\nhint: Updates were rejected because the remote contains work that you do not";
    for reason in ["fetch first", "non-fast-forward"] {
        let text = format!("To /tmp/r-origin.git\n ! [rejected]        main -> main ({reason})\n{tail}");
        assert_eq!(classify(&text), Some(Cause::PushBehind { branch: s("main") }), "{reason}");
    }
    let stale = format!(" ! [rejected]        main -> main (stale info)\n{tail}");
    assert_eq!(classify(&stale), Some(Cause::StaleLease { branch: s("main") }));
    let remote = format!(" ! [remote rejected] main -> main (branch is currently checked out)\n{tail}");
    assert_eq!(classify(&remote), Some(Cause::PushRefused { reason: s("branch is currently checked out") }));
    let fetch = "From /tmp/r-origin.git\n ! [rejected] main       -> side  (non-fast-forward)";
    assert_eq!(classify(fetch), Some(Cause::Diverged { branch: Some(s("side")) }));
    let ff_only = "hint: Diverging branches can't be fast-forwarded, you need to either:\nhint:\nhint:   git merge --no-ff\nfatal: Not possible to fast-forward, aborting.";
    assert_eq!(classify(ff_only), Some(Cause::Diverged { branch: None }));
}

#[test]
fn upstreams_detached_and_unknown_refs() {
    let push = "fatal: The current branch feat has no upstream branch.\nTo push the current branch and set the remote as upstream, use\n\n    git push --set-upstream origin feat\n\nTo have this happen automatically for branches without a tracking\nupstream, see 'push.autoSetupRemote' in 'git help config'.";
    assert_eq!(classify(push), Some(Cause::NoUpstream { push_branch: Some(s("feat")) }));
    let pull = "There is no tracking information for the current branch.\nPlease specify which branch you want to merge with.\nSee git-pull(1) for details.";
    assert_eq!(classify(pull), Some(Cause::NoUpstream { push_branch: None }));
    let gone = "Your configuration specifies to merge with the ref 'refs/heads/gone'\nfrom the remote, but no such ref was fetched.";
    assert_eq!(classify(gone), Some(Cause::UpstreamGone { branch: s("gone") }));
    for text in [
        "You are not currently on a branch.\nPlease specify which branch you want to merge with.",
        "fatal: You are not currently on a branch.\nTo push the history leading to the current (detached HEAD)\nstate now, use",
    ] {
        assert_eq!(classify(text), Some(Cause::DetachedHead), "{text}");
    }
    for text in [
        "error: pathspec 'nope' did not match any file(s) known to git",
        "merge: nope - not something we can merge",
        "fatal: couldn't find remote ref nope",
        "error: src refspec nope does not match any\nerror: failed to push some refs to 'origin'",
        "fatal: 'nope' is not a commit and a branch 'x' cannot be created from it",
        "fatal: ambiguous argument 'nope': unknown revision or path not in the working tree.",
        "fatal: bad revision 'nope'",
        "fatal: invalid upstream 'nope'",
        "fatal: invalid reference: nope",
    ] {
        assert_eq!(classify(text), Some(Cause::UnknownRef { name: s("nope") }), "{text}");
    }
}

#[test]
fn locks_sign_in_and_merge_refused() {
    let now = "fatal: Unable to create '/tmp/r/.git/index.lock': File exists.\n\nAnother git process seems to be running in this repository, or the lock file may be stale";
    assert_eq!(classify(now), Some(Cause::Locked { file: s("index.lock") }));
    let older = "fatal: Unable to create '/tmp/r/.git/index.lock': File exists.\n\nAnother git process seems to be running in this repository, e.g.\nan editor opened by 'git commit'. Please make sure all processes\nare terminated then try again. If it still fails, a git process\nmay have crashed in this repository earlier:\nremove the file manually to continue.";
    assert_eq!(classify(older), Some(Cause::Locked { file: s("index.lock") }));
    let reflock = "error: cannot lock ref 'refs/heads/main': Unable to create '/tmp/r/.git/refs/heads/main.lock': File exists.";
    assert_eq!(classify(reflock), Some(Cause::Locked { file: s("main.lock") }));
    let https = "fatal: could not read Username for 'https://example.com': terminal prompts disabled";
    assert_eq!(classify(https), Some(Cause::SignIn(AuthNeed::Https)));
    let ssh = "git@example.com: Permission denied (publickey).\nfatal: Could not read from remote repository.";
    assert_eq!(classify(ssh), Some(Cause::SignIn(AuthNeed::Ssh)));
    assert_eq!(
        classify("Host key verification failed.\nfatal: Could not read from remote repository."),
        Some(Cause::SignIn(AuthNeed::HostKey))
    );
    let changed = "@@@@@@@@@@@\n@    WARNING: REMOTE HOST IDENTIFICATION HAS CHANGED!     @\n@@@@@@@@@@@\nHost key verification failed.";
    assert_eq!(classify(changed), Some(Cause::HostKeyChanged));
    assert_eq!(classify("Merge with strategy ort failed."), Some(Cause::MergeRefused));
}

#[test]
fn remote_failures_come_after_sign_in() {
    use crate::remote_failure::RemoteFailure;
    let resolve = "fatal: unable to access 'https://git.example.com/a.git/': Could not resolve host: git.example.com";
    assert_eq!(classify(resolve), Some(Cause::Remote(RemoteFailure::Unreachable { host: Some(s("git.example.com")) })));
    let bare =
        "fatal: Could not read from remote repository.\n\nPlease make sure you have the correct access rights\nand the repository exists.";
    assert_eq!(classify(bare), Some(Cause::Remote(RemoteFailure::Unreadable)));
    let ssh = "git@example.com: Permission denied (publickey).\nfatal: Could not read from remote repository.";
    assert_eq!(classify(ssh), Some(Cause::SignIn(AuthNeed::Ssh)), "the sign-in cause wins");
    let host_key = "Host key verification failed.\nfatal: Could not read from remote repository.";
    assert_eq!(classify(host_key), Some(Cause::SignIn(AuthNeed::HostKey)));
    let changed = "@@@@\n@    WARNING: REMOTE HOST IDENTIFICATION HAS CHANGED!     @\n@@@@\nfatal: Could not read from remote repository.";
    assert_eq!(classify(changed), Some(Cause::HostKeyChanged));
    assert_eq!(classify("fatal: offline"), None);
}

#[test]
fn unrecognised_echoes_and_hints() {
    assert_eq!(classify("fatal: offline"), None);
    assert_eq!(classify(""), None);
    assert_eq!(classify("$ git checkout nope\nfatal: boom"), None, "an echo line is never matched");
    assert_eq!(classify("hint: Not possible to fast-forward, aborting.\nfatal: boom"), None, "nor a hint");
    assert_eq!(classify("$ git merge 'Merge with strategy ort failed.'"), None);
}

fn quiet(a: &[&str]) -> Vec<String> {
    with_quiet_apply(&args(a)).into_owned()
}

#[test]
fn quiet_apply_on_merge_pull_and_stash() {
    for a in [
        &["merge", "side"][..],
        &["pull", "--ff"],
        &["pull", "--ff-only"],
        &["pull", "--rebase=false"],
        &["pull", "--no-rebase"],
        &["stash", "pop", "--index", "stash@{0}"],
        &["--literal-pathspecs", "stash", "push", "-u", "--", "a"],
        &["-c", "x=y", "merge", "z"],
    ] {
        let mut want = args(&["-c", "apply.whitespace=nowarn"]);
        want.extend(args(a));
        assert_eq!(quiet(a), want, "{a:?}");
    }
    for a in [
        &["pull", "--rebase"][..],
        &["pull", "-r"],
        &["pull", "--rebase=merges"],
        &["apply", "--check", "p.patch"],
        &["am"],
        &["fetch"],
        &["push"],
        &["rebase", "main"],
        &["-c", "merge"],
        &[],
    ] {
        let line = args(a);
        assert!(matches!(with_quiet_apply(&line), Cow::Borrowed(_)), "{a:?} is unchanged");
    }
}
