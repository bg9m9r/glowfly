//! The sentences for git failures reaching or reading a remote, in the user's language: the one
//! place job toasts (update/job_errors.rs) and the Fetch/Pull repositories rows (repo_sync.rs)
//! get them from. Pure.

use glowfly_core::auth_failure::AuthNeed;
use glowfly_core::git_errors::{Cause, classify};
use glowfly_core::i18n::Lang;
use glowfly_core::remote_failure::RemoteFailure;

/// What went wrong reaching or reading the remote, and what to do.
pub fn remote(t: Lang, f: &RemoteFailure) -> String {
    match f {
        RemoteFailure::Unreachable { host: Some(host) } => t.trf("git_errors.remote.unreachable_host", &[("host", host)]),
        RemoteFailure::Unreachable { host: None } => t.tr("git_errors.remote.unreachable").to_owned(),
        RemoteFailure::NotFound => t.tr("git_errors.remote.not_found").to_owned(),
        RemoteFailure::NoAccess => t.tr("git_errors.remote.no_access").to_owned(),
        RemoteFailure::Unreadable => t.tr("git_errors.remote.unreadable").to_owned(),
    }
}

/// A repository row's sentence for a fetch or pull that ran with prompts off: remote, sign-in
/// and host-key failures (each says to retry); None for anything else.
pub fn sync_failure(t: Lang, output: &str) -> Option<String> {
    Some(match classify(output)? {
        Cause::Remote(f) => remote(t, &f),
        Cause::SignIn(AuthNeed::Https) => t.tr("repo_sync.failed.sign_in").to_owned(),
        Cause::SignIn(AuthNeed::Ssh) => t.tr("repo_sync.failed.ssh_key").to_owned(),
        Cause::SignIn(AuthNeed::HostKey) => t.tr("repo_sync.failed.host_key").to_owned(),
        Cause::HostKeyChanged => t.tr("git_errors.host_key_changed").to_owned(),
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
        assert_eq!(sync_failure(EN, "fatal: Not possible to fast-forward, aborting."), None, "a diverged pull keeps git's words");
        assert_eq!(sync_failure(EN, "fatal: offline"), None);
    }
}
