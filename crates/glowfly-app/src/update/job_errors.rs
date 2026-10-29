//! A failed job's toast: git's cause in plain words and the fix that answers it
//! (glowfly_core::git_errors), or "<label> failed: <last line>". Every fix only opens a dialog
//! or a Preferences page, or runs what the failed job meant to run; none discards work.

use crate::msg::{BranchMsg, ConflictMsg, JobMsg, Msg, PrefsMsg, StashMsg};
use crate::remote::RemoteMsg;
use crate::state::auto_stash::StashRetry;
use crate::state::prefs::PrefPage;
use crate::state::{AppState, RunningJob, ToastAction};
use glowfly_core::auth_failure::AuthNeed;
use glowfly_core::git_errors::{Cause, classify};
use glowfly_core::i18n::Lang;
use glowfly_core::paths::same_spelling;
use glowfly_core::remote_failure::RemoteFailure;
use glowfly_core::repo::{RemoteInfo, Snapshot};
use glowfly_core::stash_ops::StashScope;

/// A failure toast's text and the button that fixes its cause, if Glowfly has one.
pub(super) struct Explained {
    pub text: String,
    pub fix: Option<ToastAction>,
}

/// Explains `job`'s failure from its `output`, when git's cause is one Glowfly recognises.
/// `retry` is the guarded operation it ran without stashing (update/auto_stash.rs), for
/// Stash and <op>.
pub(super) fn explain(st: &AppState, job: &RunningJob, retry: Option<StashRetry>, output: &str) -> Option<Explained> {
    let cause = classify(output)?;
    let t = st.settings.lang();
    // What the fixes look at: the job's own repository, while it is the active tab.
    let snap = st.snap.as_ref().filter(|s| job.workdir.as_deref().is_some_and(|w| same_spelling(w, &s.workdir)));
    let head = snap.and_then(|s| s.head_branch.clone());
    let is_head = |b: &str| head.as_deref() == Some(b);
    let fix = |label: &str, msg: Msg| Some(ToastAction { label: label.to_owned(), msg: Box::new(msg), workdir: job.workdir.clone() });
    let stash_dialog =
        || fix(t.tr("git_errors.stash_dialog.button"), Msg::Stash(StashMsg::PushDialog { scope: StashScope::All, path: None }));
    let stash_and =
        |r: StashRetry| fix(crate::state::auto_stash::button(t, &r.op), Msg::AutoStash(crate::msg::AutoStashMsg::Retry(Box::new(r))));
    let view_conflicts =
        |f: &String| fix(t.tr("details.banner.view_conflicts.button"), Msg::Conflict(ConflictMsg::OpenConflict(f.clone())));
    let pull = || fix(t.tr("git_errors.pull.button"), Msg::git("Pull", &["pull", "--ff"]));
    let set_upstream = || head.clone().and_then(|b| fix(t.tr("branch.menu.set_upstream"), Msg::Branch(BranchMsg::SetUpstreamDialog(b))));
    let (text, fix) = match cause {
        Cause::LocalChanges { files } => {
            let text = if files.is_empty() {
                t.tr("git_errors.local_changes.unlisted").to_owned()
            } else {
                t.trn("git_errors.local_changes", files.len(), &[])
            };
            (text, retry.map_or_else(stash_dialog, stash_and))
        }
        Cause::MergeRefused => (t.tr("git_errors.merge_refused").to_owned(), retry.and_then(stash_and)),
        Cause::UntrackedInTheWay { files } => (t.trn("git_errors.untracked", files.len(), &[]), stash_dialog()),
        Cause::Conflicts { files } => match files.first() {
            Some(first) => (t.trn("git_errors.conflicts", files.len(), &[]), view_conflicts(first)),
            None => (t.tr("git_errors.conflicts.unlisted").to_owned(), None),
        },
        Cause::Unresolved => (t.tr("git_errors.unresolved").to_owned(), first_conflict(snap).and_then(|f| view_conflicts(&f))),
        Cause::MergeInProgress => (t.tr("git_errors.merge_in_progress").to_owned(), None),
        Cause::PushBehind { branch } => (t.tr("git_errors.push_behind").to_owned(), if is_head(&branch) { pull() } else { None }),
        Cause::StaleLease { .. } => (
            t.tr("git_errors.stale_lease").to_owned(),
            fix(t.tr("git_errors.fetch.button"), Msg::git("Fetch all", &["fetch", "--all", "--prune"])),
        ),
        Cause::PushRefused { reason } => (t.trf("git_errors.push_refused", &[("reason", &reason)]), None),
        Cause::NoUpstream { push_branch: Some(b) } => (
            t.trf("git_errors.no_upstream.push", &[("branch", &b)]),
            fix(t.tr("git_errors.push.button"), Msg::Job(JobMsg::Push { force: false })),
        ),
        Cause::NoUpstream { push_branch: None } => (t.tr("git_errors.no_upstream.pull").to_owned(), set_upstream()),
        Cause::UpstreamGone { branch } => (t.trf("git_errors.upstream_gone", &[("branch", &branch)]), set_upstream()),
        Cause::Diverged { branch } => {
            let ours = branch.as_deref().is_none_or(is_head) && head.is_some();
            (t.tr("git_errors.diverged").to_owned(), if ours { pull() } else { None })
        }
        Cause::DetachedHead => (
            t.tr("git_errors.detached").to_owned(),
            fix(t.tr("git_errors.create_branch.button"), Msg::Branch(BranchMsg::NewBranch { start: None, name: String::new() })),
        ),
        Cause::UnknownRef { name } => (t.trf("git_errors.unknown_ref", &[("name", &name)]), None),
        Cause::Locked { file } => (t.trf("git_errors.locked", &[("file", &file)]), None),
        Cause::SignIn(need) => sign_in(t, need, fix),
        Cause::HostKeyChanged => (t.tr("git_errors.host_key_changed").to_owned(), None),
        Cause::Remote(f) => {
            // A remote that isn't there, or can't be read, often has a wrong URL.
            let wrong_url = matches!(f, RemoteFailure::NotFound | RemoteFailure::Unreadable);
            let edit = snap.filter(|_| wrong_url).and_then(|s| failing_remote(s, output));
            let edit = edit.and_then(|name| fix(t.tr("git_errors.edit_remote.button"), Msg::Remote(RemoteMsg::EditDialog(name))));
            (crate::state::error_words::remote(t, &f), edit)
        }
    };
    Some(Explained { text, fix })
}

/// The sign-in failures' texts; an SSH key the server refused offers Preferences > SSH.
fn sign_in(t: Lang, need: AuthNeed, fix: impl Fn(&str, Msg) -> Option<ToastAction>) -> (String, Option<ToastAction>) {
    match need {
        AuthNeed::Https => (t.tr("git_errors.sign_in.https").to_owned(), None),
        AuthNeed::Ssh => (
            t.tr("git_errors.sign_in.ssh").to_owned(),
            fix(t.tr("git_errors.ssh_keys.button"), Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Ssh))),
        ),
        AuthNeed::HostKey => (t.tr("git_errors.host_key").to_owned(), None),
    }
}

/// The configured remote a failed fetch, pull or push was talking to, judged from its
/// `output`: the one git names ("'origin' does not appear to be a git repository", "could not
/// fetch origin") or whose URL it prints; else the checked-out branch's upstream remote, else
/// the only remote. None when that leaves it open.
fn failing_remote(snap: &Snapshot, output: &str) -> Option<String> {
    let remotes = &snap.remote_info;
    let named = |r: &&RemoteInfo| {
        let fetch = format!("could not fetch {}", r.name);
        output.contains(&format!("'{}'", r.name)) || output.lines().any(|l| l.trim_end().ends_with(&fetch))
    };
    let printed =
        |r: &&RemoteInfo| r.urls.iter().chain(&r.push_urls).map(|u| u.trim_end_matches('/')).any(|u| !u.is_empty() && output.contains(u));
    let upstream = || {
        let tracking = snap.locals.iter().find(|b| b.is_head)?.tracking.as_ref().filter(|t| t.is_remote())?;
        remotes.iter().find(|r| r.name == tracking.remote)
    };
    let only = || if let [r] = remotes.as_slice() { Some(r) } else { None };
    let found = remotes.iter().find(named).or_else(|| remotes.iter().find(printed)).or_else(upstream).or_else(only);
    found.map(|r| r.name.clone())
}

/// The first conflicted file the snapshot shows.
fn first_conflict(snap: Option<&Snapshot>) -> Option<String> {
    snap?.unstaged.iter().find(|f| f.status == '!').map(|f| f.path.clone())
}

/// "<label> succeeded", or "<label> failed: <the last line git printed>".
pub(super) fn default_headline(label: &str, ok: bool, output: &str) -> String {
    if ok { format!("{label} succeeded") } else { format!("{label} failed: {}", super::ops::last_line(output)) }
}
