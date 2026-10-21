//! Headless tests for a partial clone's missing file contents: a diff, File view or blame read
//! that needs them downloads exactly those and reads again; Cancel, failures, a retry, an
//! untrusted repository's download job and an old git.

use super::tests::{drive, finish, git, scratch_repo, state};
use super::trust_tests::{asking, real};
use super::update;
use crate::effect::partial::{PartialEffect, StopKey};
use crate::effect::{BlameRequest, Effect, FileTextRequest};
use crate::msg::{ContentFailure, Msg, PartialMsg, RepoMsg, TrustMsg};
use crate::runner;
use crate::state::AppState;
use crate::state::partial::{ContentPhase, ContentView, DOWNLOAD_LABEL, Wait};
use git2::Oid;
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::repo::DiffTarget;

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn file_url(p: &Path) -> String {
    let s = p.display().to_string().replace('\\', "/");
    if s.starts_with('/') { format!("file://{s}") } else { format!("file:///{s}") }
}

/// A source whose a.txt goes "one", then "one\ntwo", and a blobless clone of it (HEAD~1's a.txt
/// missing). `untrusted` gives the clone a setting that holds jobs for the trust prompt.
/// Returns (state with the clone open, source, clone).
fn partial(name: &str, untrusted: bool) -> (AppState, PathBuf, PathBuf) {
    let src = scratch_repo(name);
    git(&src, &["commit", "-qam", "second"]);
    git(&src, &["config", "uploadpack.allowFilter", "true"]);
    git(&src, &["config", "uploadpack.allowAnySHA1InWant", "true"]);
    let dest = src.with_file_name(format!("{}-partial", src.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&dest);
    git(src.parent().unwrap(), &["clone", "-q", "--filter=blob:none", &file_url(&src), &dest.display().to_string()]);
    super::tests::set_identity(&dest);
    if untrusted {
        git(&dest, &["config", "core.sshCommand", "ssh -v"]);
    }
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dest.clone())));
    assert_eq!(st.runs_git_unasked(), !untrusted);
    (st, src, dest)
}

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

fn head(st: &AppState) -> Oid {
    st.snap.as_ref().unwrap().head_oid.unwrap()
}

fn blob(dir: &Path, spec: &str) -> Oid {
    git2::Repository::open(dir).unwrap().revparse_single(spec).unwrap().id()
}

/// Opens HEAD's diff of a.txt and runs its read: the message it sends back.
fn read_head_diff(st: &mut AppState) -> Msg {
    let fx = run(st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Commit(head(st), "a.txt".into()))));
    let mut back: Vec<Msg> = fx.into_iter().filter(|e| matches!(e, Effect::LoadDiff { .. })).filter_map(runner::execute).collect();
    assert_eq!(back.len(), 1);
    back.pop().unwrap()
}

/// The message a scoped result carries.
fn unscoped(msg: Option<Msg>) -> Option<Msg> {
    match msg {
        Some(Msg::Scoped { msg, .. }) => Some(*msg),
        other => other,
    }
}

fn phase(st: &AppState) -> Option<ContentPhase> {
    st.partial.content.as_ref().map(|p| p.phase.clone())
}

fn fetch_id(fx: &[Effect]) -> u64 {
    match fx {
        [Effect::Partial(PartialEffect::FetchContent(r))] => r.id,
        other => panic!("expected one download, got {} effects", other.len()),
    }
}

fn diff_lines(st: &AppState) -> Vec<String> {
    let (_, d) = st.diff.as_ref().expect("a diff");
    d.hunks.iter().flat_map(|h| h.lines.iter().map(|l| l.text.clone())).collect()
}

#[test]
fn a_diff_downloads_its_missing_side_and_reads_again() {
    let (mut st, src, dest) = partial("partial-diff-read", false);
    let back = read_head_diff(&mut st);
    let fx = run(&mut st, back);
    let id = fetch_id(&fx);
    let p = st.for_diff().expect("the diff waits");
    assert_eq!(p.oids, [blob(&src, "HEAD~1:a.txt")]);
    assert_eq!((p.path.as_str(), p.remote.as_str()), ("a.txt", "origin"));
    assert_eq!(p.phase, ContentPhase::Fetching { id, stopping: false });
    assert!(st.diff_or_download());
    if let [Effect::Partial(PartialEffect::FetchContent(r))] = &fx[..] {
        assert!(r.cmds.iter().all(|c| trellis_core::gitcli::subcommand(c) == Some("fetch")));
    }
    finish(&mut st, fx);
    assert!(st.partial.content.is_none(), "nothing waits once it is there");
    assert_eq!(diff_lines(&st), ["one", "two"]);
    assert!(st.partial.fetched.contains(&blob(&src, "HEAD~1:a.txt")));
    cleanup(&[&src, &dest]);
}

#[test]
fn cancel_failure_and_try_again() {
    let (mut st, src, dest) = partial("partial-diff-cancel", false);
    let back = read_head_diff(&mut st);
    let id = fetch_id(&run(&mut st, back));
    let fx = run(&mut st, Msg::Partial(PartialMsg::StopContent));
    assert!(matches!(&fx[..], [Effect::StopGit(StopKey::Content(i))] if *i == id));
    assert_eq!(phase(&st), Some(ContentPhase::Fetching { id, stopping: true }));
    assert!(run(&mut st, Msg::Partial(PartialMsg::StopContent)).is_empty(), "asked once");
    let stopped = |id| Msg::Partial(PartialMsg::ContentFetched { id, result: Err(ContentFailure::Stopped) });
    assert!(run(&mut st, stopped(id + 7)).is_empty());
    assert_eq!(phase(&st), Some(ContentPhase::Fetching { id, stopping: true }), "another download's end is ignored");
    run(&mut st, stopped(id));
    assert_eq!(phase(&st), Some(ContentPhase::Waiting(Wait::Cancelled)));

    let again = fetch_id(&run(&mut st, Msg::Partial(PartialMsg::DownloadContent)));
    assert!(again > id);
    let failed = Err(ContentFailure::Failed("fatal: could not read from remote repository".into()));
    run(&mut st, Msg::Partial(PartialMsg::ContentFetched { id: again, result: failed }));
    assert_eq!(phase(&st), Some(ContentPhase::Waiting(Wait::Failed("fatal: could not read from remote repository".into()))));

    // Closing the diff leaves nothing waiting on screen.
    run(&mut st, Msg::Repo(RepoMsg::CloseDiff));
    assert!(st.for_diff().is_none() && !st.diff_or_download());
    cleanup(&[&src, &dest]);
}

#[test]
fn a_newer_read_of_another_file_stops_the_download_and_a_stale_one_is_dropped() {
    let (mut st, src, dest) = partial("partial-diff-newer", false);
    let first = read_head_diff(&mut st);
    let stale = read_head_diff(&mut st);
    assert!(run(&mut st, first).is_empty(), "superseded while it read");
    let id = fetch_id(&run(&mut st, stale));
    // The same read again (a reload) keeps the download.
    let reload = read_head_diff(&mut st);
    assert!(run(&mut st, reload).is_empty());
    assert_eq!(phase(&st), Some(ContentPhase::Fetching { id, stopping: false }));
    assert_eq!(st.for_diff().map(|p| p.view.clone()), Some(ContentView::Diff { generation: st.diff_gen }));
    // The File view reports its missing version too.
    let request = FileTextRequest { workdir: dest.clone(), oid: blob(&dest, "HEAD~1"), path: "a.txt".into(), before: false, smudge: false };
    let Some(Msg::Partial(PartialMsg::ContentMissing { retry, .. })) = unscoped(runner::execute(Effect::LoadFileText(request))) else {
        panic!("the File view reports the missing version")
    };
    assert!(matches!(*retry, Effect::LoadFileText(_)));
    cleanup(&[&src, &dest]);
}

#[test]
fn blame_reports_the_versions_it_needs() {
    let (st, src, dest) = partial("partial-blame", false);
    let request = BlameRequest {
        workdir: dest.clone(),
        path: "a.txt".into(),
        at: None,
        opts: Default::default(),
        highlight: false,
        generation: 4,
        follow: None,
    };
    match unscoped(runner::execute(Effect::LoadBlame(request))) {
        Some(Msg::Partial(PartialMsg::ContentMissing { missing, retry })) => {
            assert_eq!(missing.oids, [blob(&src, "HEAD~1:a.txt")]);
            assert_eq!(ContentView::of(&retry), Some(ContentView::Blame { generation: 4 }));
        }
        _ => panic!("blame reports the missing version"),
    }
    drop(st);
    cleanup(&[&src, &dest]);
}

#[test]
fn an_untrusted_repository_downloads_as_a_job_once_trusted() {
    let (mut st, src, dest) = partial("partial-untrusted", true);
    st.modal = None;
    let (to, message) = (head(&st), "second".to_owned());
    st.undo.record(trellis_core::undo::Action::Commit {
        branch: Some("main".into()),
        from: blob(&dest, "HEAD~1"),
        to,
        message,
        amend: false,
    });
    let fetched_at = st.snap.as_ref().unwrap().fetched_at;
    let back = read_head_diff(&mut st);
    assert!(run(&mut st, back).is_empty(), "nothing runs in an untrusted repository unasked");
    assert_eq!(phase(&st), Some(ContentPhase::Waiting(Wait::Trust)));
    assert!(run(&mut st, Msg::Partial(PartialMsg::DownloadContent)).is_empty(), "held for the trust prompt");
    assert!(asking(&st));
    let real = real(&st);
    let fx = run(&mut st, Msg::Trust(TrustMsg::Decide { real, trusted: true }));
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some(DOWNLOAD_LABEL));
    assert_eq!(phase(&st), Some(ContentPhase::FetchingAsJob));
    assert!(st.job_cancellable(), "a download job only fetches");
    finish(&mut st, fx);
    assert!(st.job.is_none() && st.partial.content.is_none());
    assert_eq!(diff_lines(&st), ["one", "two"]);
    assert_eq!(st.undo.undo.len(), 1, "a download moves no branch, so Undo stays");
    assert_eq!(st.snap.as_ref().unwrap().fetched_at, fetched_at, "nor does it count as a fetch");
    cleanup(&[&src, &dest]);
}

#[test]
fn old_git_and_contents_that_never_came_say_so() {
    let (mut st, src, dest) = partial("partial-old-git", false);
    st.partial.fetched.insert(blob(&src, "HEAD~1:a.txt"));
    let back = read_head_diff(&mut st);
    assert!(run(&mut st, back).is_empty());
    assert_eq!(phase(&st), Some(ContentPhase::Waiting(Wait::StillMissing)), "downloaded once already");

    st.partial = Default::default();
    st.git.info = Some(trellis_core::git_version::GitInfo { version: Some((2, 28, 0)), ..Default::default() });
    let back = read_head_diff(&mut st);
    assert!(run(&mut st, back).is_empty());
    let Some(ContentPhase::Waiting(Wait::OldGit(why))) = phase(&st) else { panic!("waits for a newer git") };
    assert!(why.contains("2.29"), "{why}");
    assert!(run(&mut st, Msg::Partial(PartialMsg::DownloadContent)).is_empty(), "no button to press");
    cleanup(&[&src, &dest]);
}
