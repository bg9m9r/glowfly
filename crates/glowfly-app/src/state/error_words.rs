//! The sentences for why a git command failed, in the user's language (`cause`, `explained`):
//! the one place job toasts (update/job_errors.rs), other flows' failure headlines (auto-stash,
//! clone, submodule, Git Flow, worktree) and the Fetch/Pull repositories rows (repo_sync.rs) get
//! them from; and a hosting service's refusal (`hosting`), for every call that shows one. Pure.

use crate::effect::ForgeError;
use glowfly_core::auth_failure::AuthNeed;
use glowfly_core::error::advice::{self, Ask};
use glowfly_core::forge::Provider;
use glowfly_core::git_errors::{Cause, classify};
use glowfly_core::i18n::Lang;
use glowfly_core::remote_failure::RemoteFailure;

/// What `cause` means and what to do, in a sentence or two.
pub fn cause(t: Lang, cause: &Cause) -> String {
    match cause {
        Cause::LocalChanges { files } if files.is_empty() => t.tr("git_errors.local_changes.unlisted").to_owned(),
        Cause::LocalChanges { files } => t.trn("git_errors.local_changes", files.len(), &[]),
        Cause::MergeRefused => t.tr("git_errors.merge_refused").to_owned(),
        Cause::FetchMovedHead { from } if from.is_empty() => t.tr("git_errors.fetch_moved_head.unnamed").to_owned(),
        Cause::FetchMovedHead { from } => t.trf("git_errors.fetch_moved_head", &[("commit", &from.get(..7).unwrap_or(from))]),
        Cause::UntrackedInTheWay { files } => t.trn("git_errors.untracked", files.len(), &[]),
        Cause::Conflicts { files, unsure } => conflicts(t, files.len() + unsure.len()),
        Cause::Unresolved => t.tr("git_errors.unresolved").to_owned(),
        Cause::MergeInProgress => t.tr("git_errors.merge_in_progress").to_owned(),
        Cause::PushBehind { .. } => t.tr("git_errors.push_behind").to_owned(),
        Cause::StaleLease { .. } => t.tr("git_errors.stale_lease").to_owned(),
        Cause::PushRefused { reason } => t.trf("git_errors.push_refused", &[("reason", reason)]),
        Cause::NoUpstream { push_branch: Some(b) } => t.trf("git_errors.no_upstream.push", &[("branch", b)]),
        Cause::NoUpstream { push_branch: None } => t.tr("git_errors.no_upstream.pull").to_owned(),
        Cause::UpstreamGone { branch } => t.trf("git_errors.upstream_gone", &[("branch", branch)]),
        Cause::Diverged { .. } => t.tr("git_errors.diverged").to_owned(),
        Cause::DetachedHead => t.tr("git_errors.detached").to_owned(),
        Cause::UnknownRef { name } => t.trf("git_errors.unknown_ref", &[("name", name)]),
        Cause::Locked { file } => t.trf("git_errors.locked", &[("file", file)]),
        Cause::SignIn(AuthNeed::Https) => t.tr("git_errors.sign_in.https").to_owned(),
        Cause::SignIn(AuthNeed::Ssh) => t.tr("git_errors.sign_in.ssh").to_owned(),
        Cause::SignIn(AuthNeed::HostKey) => t.tr("git_errors.host_key").to_owned(),
        Cause::HostKeyChanged => t.tr("git_errors.host_key_changed").to_owned(),
        Cause::Remote(f) => remote(t, f),
    }
}

/// "Stopped on conflicts in <n> files", or without a count when there is none to give.
pub fn conflicts(t: Lang, n: usize) -> String {
    if n == 0 { t.tr("git_errors.conflicts.unlisted").to_owned() } else { t.trn("git_errors.conflicts", n, &[]) }
}

/// `cause`'s sentence for a failed command's `output`, when Glowfly recognises why it failed.
pub fn explained(t: Lang, output: &str) -> Option<String> {
    classify(output).map(|c| cause(t, &c))
}

/// What went wrong reaching or reading the remote, and what to do.
pub fn remote(t: Lang, f: &RemoteFailure) -> String {
    match f {
        RemoteFailure::Unreachable { host: Some(host) } => t.trf("git_errors.remote.unreachable_host", &[("host", host)]),
        RemoteFailure::Unreachable { host: None } => t.tr("git_errors.remote.unreachable").to_owned(),
        RemoteFailure::NotFound => t.tr("git_errors.remote.not_found").to_owned(),
        RemoteFailure::NoAccess => t.tr("git_errors.remote.no_access").to_owned(),
        RemoteFailure::Unreadable => t.tr("git_errors.remote.unreadable").to_owned(),
        RemoteFailure::Certificate { host: Some(host) } => t.trf("git_errors.remote.certificate_host", &[("host", host)]),
        RemoteFailure::Certificate { host: None } => t.tr("git_errors.remote.certificate").to_owned(),
    }
}

/// A failed call to `provider`'s service while doing `ask`, for a person: `advice`'s sentence
/// when Glowfly recognises the failure, else the service's own words (with a pointer to signing
/// in again when that could fix it).
pub fn hosting(t: Lang, provider: Provider, ask: Ask, e: &ForgeError) -> String {
    advice::service(t, provider.name(), ask, &e.message)
        .unwrap_or_else(|| if e.auth { format!("{} {}", e.message, t.tr("service_errors.sign_in_again.help")) } else { e.message.clone() })
}

/// `hosting`'s sentence as the first line of a log text whose second line is `line` (the
/// failure in the service's words); just `line` when there is no sentence. A toast of it shows
/// the first line.
pub fn hosting_log(t: Lang, provider: Provider, ask: Ask, e: &ForgeError, line: String) -> String {
    match advice::service(t, provider.name(), ask, &e.message) {
        Some(sentence) => format!("{sentence}\n{line}"),
        None => line,
    }
}

/// A repository row's sentence for a fetch or pull that ran with prompts off: remote, sign-in
/// and host-key failures, a pull that can't fast-forward and a lock file in the way (each says
/// what to do); None for anything else.
pub fn sync_failure(t: Lang, output: &str) -> Option<String> {
    Some(match classify(output)? {
        Cause::Remote(f) => remote(t, &f),
        Cause::SignIn(AuthNeed::Https) => t.tr("repo_sync.failed.sign_in").to_owned(),
        Cause::SignIn(AuthNeed::Ssh) => t.tr("repo_sync.failed.ssh_key").to_owned(),
        Cause::SignIn(AuthNeed::HostKey) => t.tr("repo_sync.failed.host_key").to_owned(),
        Cause::HostKeyChanged => t.tr("git_errors.host_key_changed").to_owned(),
        Cause::Diverged { .. } => t.tr("repo_sync.failed.diverged").to_owned(),
        Cause::Locked { file } => t.trf("repo_sync.failed.locked", &[("file", &file)]),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const EN: Lang = Lang::En;

    #[test]
    fn each_remote_failure_has_its_sentence() {
        let named = remote(EN, &RemoteFailure::Unreachable { host: Some("git.example.com".into()) });
        assert_eq!(named, "Couldn't reach git.example.com. Check your network connection (and VPN or proxy), then try again.");
        let unnamed = remote(EN, &RemoteFailure::Unreachable { host: None });
        assert!(unnamed.starts_with("Couldn't reach the server."), "{unnamed}");
        assert!(remote(EN, &RemoteFailure::NotFound).starts_with("The remote repository wasn't found."));
        assert!(remote(EN, &RemoteFailure::NoAccess).starts_with("Your account can't access the remote repository."));
        assert!(remote(EN, &RemoteFailure::Unreadable).starts_with("Couldn't read from the remote repository."));
    }

    #[test]
    fn sync_failures_cover_sign_in_and_host_keys() {
        let says = |out: &str| sync_failure(EN, out).unwrap_or_default();
        assert!(
            says("fatal: could not read Username for 'https://h': terminal prompts disabled")
                .starts_with("The server needs you to sign in.")
        );
        assert!(
            says("git@h: Permission denied (publickey).\nfatal: Could not read from remote repository.")
                .starts_with("The server didn't accept your SSH key.")
        );
        assert!(
            says("Host key verification failed.\nfatal: Could not read from remote repository.")
                .starts_with("The server's SSH host key isn't confirmed yet.")
        );
        let changed = "@@@@@@@@\n@    WARNING: REMOTE HOST IDENTIFICATION HAS CHANGED!     @\n@@@@@@@@";
        assert_eq!(says(changed), EN.tr("git_errors.host_key_changed"));
        assert!(
            says("fatal: unable to access 'https://h.example/r/': Could not resolve host: h.example")
                .starts_with("Couldn't reach h.example.")
        );
        assert!(says("fatal: Not possible to fast-forward, aborting.").starts_with("This branch and its upstream have both moved on"));
        assert!(says("fatal: Need to specify how to reconcile divergent branches.").starts_with("This branch and its upstream"));
        let locked =
            "fatal: Unable to create '/r/.git/index.lock': File exists.\n\nAnother git process seems to be running in this repository";
        assert_eq!(says(locked), "Another git program is using this repository (index.lock exists). Wait for it to finish, then retry.");
        let tls =
            "fatal: unable to access 'https://git.corp.example/a.git/': SSL certificate problem: unable to get local issuer certificate";
        assert!(says(tls).starts_with("Git couldn't verify the security certificate of git.corp.example."), "{}", says(tls));
        assert_eq!(sync_failure(EN, "Merge with strategy ort failed."), None, "other causes keep git's words");
        assert_eq!(sync_failure(EN, "fatal: offline"), None);
    }

    #[test]
    fn hosting_failures_in_plain_words_or_the_service_own() {
        let e = |message: &str, auth: bool| ForgeError { message: message.into(), auth };
        let denied = e("access denied: the server said \u{201c}Must have push access\u{201d}", false);
        let said = hosting(EN, Provider::GitLab, Ask::Change, &denied);
        assert!(said.starts_with("Your GitLab account can't make this change in the repository."), "{said}");
        assert_eq!(hosting(EN, Provider::GitHub, Ask::Read, &e("unexpected response: x", false)), "unexpected response: x");
        let unknown_auth = hosting(EN, Provider::GitHub, Ask::Read, &e("token expired", true));
        assert_eq!(unknown_auth, "token expired Sign in again in Preferences > Integrations.");
        let net = e("network error: timed out", false);
        let log = hosting_log(EN, Provider::GitHub, Ask::Change, &net, "Creating the pull request failed: network error: timed out".into());
        assert_eq!(
            log,
            "Glowfly couldn't reach GitHub. Check your network connection, then try again.\nCreating the pull request failed: network error: timed out"
        );
        let other = e("unexpected response: x", false);
        assert_eq!(hosting_log(EN, Provider::GitHub, Ask::Change, &other, "line".into()), "line");
    }
}
