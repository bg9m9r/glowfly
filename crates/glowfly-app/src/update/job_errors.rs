//! A failed job's toast: git's cause in plain words and the fix that answers it
//! (glowfly_core::git_errors), or "<label> failed: <last line>". Every fix only opens a dialog
//! or a Preferences page, or runs what the failed job meant to run (Try again on a lock file or
//! an HTTPS sign-in, update/job_retry.rs); none discards work.

use crate::msg::{BranchMsg, ConflictMsg, JobMsg, Msg, PrefsMsg, StashMsg};
use crate::remote::RemoteMsg;
use crate::state::auto_stash::StashRetry;
use crate::state::job_spec::Retry;
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
    /// It stopped on conflicts: the count in `text` is from git's CONFLICT lines until the
    /// refreshed status counts them (update/conflict_count.rs).
    pub conflicts: bool,
}

/// Explains `job`'s failure from its `output`, when git's cause is one Glowfly recognises.
/// `retry` is the guarded operation it ran without stashing (update/auto_stash.rs), for
/// Stash and <op>; `again` the job's spec, for Try again.
pub(super) fn explain(
    st: &AppState,
    job: &RunningJob,
    retry: Option<StashRetry>,
    again: Option<&Retry>,
    output: &str,
) -> Option<Explained> {
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
    let try_again = || again.map(|r| super::job_retry::button(t, r));
    let pull = || fix(t.tr("git_errors.pull.button"), Msg::git("Pull", &["pull", "--ff"]));
    let set_upstream = || head.clone().and_then(|b| fix(t.tr("branch.menu.set_upstream"), Msg::Branch(BranchMsg::SetUpstreamDialog(b))));
    let text = crate::state::error_words::cause(t, &cause);
    let conflicts = matches!(cause, Cause::Conflicts { .. });
    let fix = match cause {
        Cause::LocalChanges { .. } => retry.map_or_else(stash_dialog, stash_and),
        Cause::MergeRefused => retry.and_then(stash_and),
        Cause::UntrackedInTheWay { .. } => stash_dialog(),
        Cause::Conflicts { files } => files.first().and_then(view_conflicts),
        Cause::Unresolved => first_conflict(snap).and_then(|f| view_conflicts(&f)),
        Cause::MergeInProgress | Cause::PushRefused { .. } | Cause::UnknownRef { .. } | Cause::HostKeyChanged => None,
        Cause::PushBehind { branch } => is_head(&branch).then(pull).flatten(),
        Cause::StaleLease { .. } => fix(t.tr("git_errors.fetch.button"), Msg::git("Fetch all", &["fetch", "--all", "--prune"])),
        Cause::NoUpstream { push_branch: Some(_) } => fix(t.tr("git_errors.push.button"), Msg::Job(JobMsg::Push { force: false })),
        Cause::NoUpstream { push_branch: None } | Cause::UpstreamGone { .. } => set_upstream(),
        Cause::Diverged { branch } => {
            let ours = branch.as_deref().is_none_or(is_head) && head.is_some();
            ours.then(pull).flatten()
        }
        Cause::DetachedHead => {
            fix(t.tr("git_errors.create_branch.button"), Msg::Branch(BranchMsg::NewBranch { start: None, name: String::new() }))
        }
        Cause::Locked { .. } | Cause::SignIn(AuthNeed::Https) => try_again(),
        Cause::SignIn(AuthNeed::Ssh) => fix(t.tr("git_errors.ssh_keys.button"), Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Ssh))),
        Cause::SignIn(AuthNeed::HostKey) => None,
        Cause::Remote(f) => {
            // A remote that isn't there, or can't be read, often has a wrong URL.
            let wrong_url = matches!(f, RemoteFailure::NotFound | RemoteFailure::Unreadable);
            let edit = snap.filter(|_| wrong_url).and_then(|s| failing_remote(s, output));
            edit.and_then(|name| fix(t.tr("git_errors.edit_remote.button"), Msg::Remote(RemoteMsg::EditDialog(name))))
        }
    };
    Some(Explained { text, fix, conflicts })
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

/// "<label> succeeded", or "<label> failed: <the last line git printed>", with the label in the
/// UI language (state/job_spec/labels.rs).
pub(super) fn default_headline(t: Lang, label: &str, ok: bool, output: &str) -> String {
    let job = crate::state::job_spec::labels::shown(t, label);
    if ok {
        t.trf("job.headline.succeeded", &[("job", &job)])
    } else {
        t.trf("job.headline.failed", &[("job", &job), ("why", &super::ops::last_line(output))])
    }
}

/// Why a flow's git command failed, for its own headline: the cause in plain words
/// (state/error_words.rs), else the last line git printed.
pub(super) fn reason(t: Lang, output: &str) -> String {
    crate::state::error_words::explained(t, output).unwrap_or_else(|| super::ops::last_line(output).to_owned())
}
