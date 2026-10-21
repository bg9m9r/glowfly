//! glowfly:// links off the UI thread: building one (the root commit), checking candidate
//! folders for the linked repository, and the scheme registration.

use crate::deep_link::{DeepLinkEffect, DeepLinkMsg};
use crate::msg::Msg;
use crate::state::deep_link::NOTHING_TO_LINK;
use glowfly_core::link::{Link, find};
use glowfly_core::url_scheme;

pub(super) fn run(e: DeepLinkEffect) -> Msg {
    let msg = match e {
        DeepLinkEffect::Build { workdir, url, target, what } => {
            let result = find::root_commit(&workdir).map_err(|e| e.to_string()).and_then(|root| {
                if url.is_none() && root.is_none() {
                    return Err(NOTHING_TO_LINK.to_owned());
                }
                Ok(Link { url, root, target }.to_uri())
            });
            DeepLinkMsg::Built { what, result }
        }
        DeepLinkEffect::Find { link, candidates, seq, origin } => {
            DeepLinkMsg::Found { seq, origin, result: Ok(matches(&link, &candidates)) }
        }
        DeepLinkEffect::SchemeStatus => DeepLinkMsg::SchemeChecked(with_exe(url_scheme::status)),
        DeepLinkEffect::Register => DeepLinkMsg::SchemeChecked(with_exe(url_scheme::register)),
        DeepLinkEffect::Unregister => DeepLinkMsg::SchemeChecked(with_exe(url_scheme::unregister)),
    };
    Msg::DeepLink(msg)
}

/// The candidates that are copies of the linked repository. One that can't be read (a share
/// that is offline, a repository another user owns) is skipped, not the whole search.
fn matches(link: &Link, candidates: &[std::path::PathBuf]) -> Vec<find::Found> {
    candidates
        .iter()
        .filter_map(|path| match find::check(path, link) {
            Ok(found) => found,
            Err(e) => {
                tracing::debug!(path = %path.display(), "skipped while looking for a linked repository: {e}");
                None
            }
        })
        .collect()
}

/// Runs a registration step for this executable.
fn with_exe(step: fn(&std::path::Path) -> Result<url_scheme::SchemeStatus, String>) -> Result<url_scheme::SchemeStatus, String> {
    let exe = std::env::current_exe().map_err(|e| format!("Glowfly's own path is unknown: {e}"))?;
    step(&exe)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deep_link::FindOrigin;
    use glowfly_core::link;

    #[test]
    fn a_repository_without_commits_or_remotes_has_no_link() {
        let dir = glowfly_core::paths::temp_dir().join(format!("glowfly-app-link-empty-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        git2::Repository::init(&dir).unwrap();
        let build = DeepLinkEffect::Build { workdir: dir.clone(), url: None, target: link::Target::Repository, what: "repository".into() };
        match run(build) {
            Msg::DeepLink(DeepLinkMsg::Built { result: Err(e), .. }) => assert_eq!(e, NOTHING_TO_LINK),
            _ => panic!("expected a refusal"),
        }
        // A folder that can't be read is skipped.
        let link = Link { url: None, root: Some("a".repeat(40)), target: link::Target::Repository };
        let find = DeepLinkEffect::Find { link, candidates: vec![dir.join("missing")], seq: 3, origin: FindOrigin::Search };
        assert!(matches!(run(find), Msg::DeepLink(DeepLinkMsg::Found { seq: 3, result: Ok(m), .. }) if m.is_empty()));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
