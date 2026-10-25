//! glowfly:// links off the UI thread: building one (the root commit), checking candidate
//! folders for the linked repository, and the scheme registration.

use super::execute::{Cx, Execute};
use crate::deep_link::{DeepLinkEffect, DeepLinkMsg};
use crate::msg::Msg;
use crate::state::deep_link::NOTHING_TO_LINK;
use glowfly_core::link::{Link, find};
use glowfly_core::url_scheme;
use std::path::PathBuf;

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

impl Execute for DeepLinkEffect {
    fn execute(self, _cx: &Cx<'_>) -> Option<Msg> {
        Some(run(self))
    }
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

type SchemeStep = fn(&std::path::Path) -> Result<url_scheme::SchemeStatus, String>;

/// Runs a registration step for this executable: for an AppImage copy, the AppImage file
/// rather than the program inside its temporary mount. Inside Flatpak the exported desktop file
/// handles links, so nothing is checked or registered.
fn with_exe(step: SchemeStep) -> Result<url_scheme::SchemeStatus, String> {
    let exe = || {
        let exe = std::env::current_exe().map_err(|e| format!("Glowfly's own path is unknown: {e}"))?;
        Ok(glowfly_core::app_update::appimage_file(&exe).unwrap_or(exe))
    };
    scheme_step(glowfly_core::flatpak::in_flatpak(), exe, step)
}

/// [`with_exe`] with its inputs: whether this copy runs in Flatpak, and how to find its program.
fn scheme_step(
    in_flatpak: bool,
    exe: impl FnOnce() -> Result<PathBuf, String>,
    step: impl FnOnce(&std::path::Path) -> Result<url_scheme::SchemeStatus, String>,
) -> Result<url_scheme::SchemeStatus, String> {
    if in_flatpak {
        return Ok(url_scheme::SchemeStatus::Unsupported(url_scheme::FLATPAK_NOTE));
    }
    step(&exe()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deep_link::FindOrigin;
    use glowfly_core::link;

    #[test]
    fn flatpak_never_registers_links() {
        let told = scheme_step(true, || panic!("no program path inside Flatpak"), |_| panic!("no step inside Flatpak"));
        assert_eq!(told, Ok(url_scheme::SchemeStatus::Unsupported(url_scheme::FLATPAK_NOTE)));
        let exe = glowfly_core::paths::temp_dir().join("glowfly");
        let mut seen = None;
        let ran = scheme_step(
            false,
            || Ok(exe.clone()),
            |p| {
                seen = Some(p.to_path_buf());
                Ok(url_scheme::SchemeStatus::NotRegistered)
            },
        );
        assert_eq!((ran, seen), (Ok(url_scheme::SchemeStatus::NotRegistered), Some(exe)));
    }

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
