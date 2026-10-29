//! What the patches `git am` applies would write, asked of git (plumbing.rs): each mailbox is
//! split and decoded as am decodes it (base64, quoted-printable, MIME), and `git apply
//! --numstat` names the paths. When one of them is a place hooks run from that follows the
//! working tree, the hooks it writes run in that same `git am` (`post-applypatch`) or in the
//! next git command, so the job is held (`Unchecked::Patched`, with the mailbox's digest so
//! another mailbox under the same name isn't covered by that choice). A patch format other
//! than a mailbox (stgit, an hg export, a series file) is Unmodeled.

use crate::trust::Follow;
use crate::trust::follow::touches;
use crate::trust::incoming::Unchecked;
use git2::Repository;
use std::path::Path;

/// Whether a patch in the mailbox at `file` writes a place in `follows` (Patched); `apply`: the
/// options `git am` passes on to `git apply` (`-p2`, `--directory=x`, `--exclude=x`).
pub(super) fn check_mailbox(repo: &Repository, file: &Path, apply: &[String], follows: &[Follow]) -> Result<(), Unchecked> {
    let bytes = std::fs::read(file).map_err(|e| Unchecked::Unreadable(format!("{}: {e}", file.display())))?;
    if !is_mailbox(&bytes) {
        return Err(Unchecked::Unmodeled(format!("a patch that isn't a mailbox ({})", file.display())));
    }
    let paths = crate::trust::plumbing::mailbox_paths(repo, file, apply)?;
    held(&paths, follows, &bytes)
}

/// The same for the patches left in an `am` in progress (`rebase-apply`, from `next` to
/// `last`), each a mail as `git mailsplit` wrote it.
pub(super) fn check_applying(repo: &Repository, follows: &[Follow]) -> Result<(), Unchecked> {
    let apply = repo.path().join("rebase-apply");
    let number = |f: &str| std::fs::read_to_string(apply.join(f)).ok().and_then(|t| t.trim().parse::<u32>().ok());
    let (Some(next), Some(last)) = (number("next"), number("last")) else {
        return Err(Unchecked::Unmodeled("an am in progress Glowfly can't read".to_owned()));
    };
    let scratch = crate::trust::plumbing::scratch()?;
    (next..=last).try_for_each(|n| {
        let Ok(mail) = std::fs::read(apply.join(format!("{n:04}"))) else { return Ok(()) };
        held(&crate::trust::plumbing::mail_paths(repo, &mail, &scratch.0, &[])?, follows, &mail)
    })
}

/// Whether the plain patch at `file` (`git apply <file>`, with its `apply` options) writes a
/// place in `follows`.
pub(super) fn check_patch(repo: &Repository, file: &Path, apply: &[String], follows: &[Follow]) -> Result<(), Unchecked> {
    let bytes = std::fs::read(file).map_err(|e| Unchecked::Unreadable(format!("{}: {e}", file.display())))?;
    held(&crate::trust::plumbing::patch_paths(repo, file, apply)?, follows, &bytes)
}

fn held(paths: &[String], follows: &[Follow], mailbox: &[u8]) -> Result<(), Unchecked> {
    match paths.iter().find(|p| touches(p, follows)) {
        Some(p) => Err(Unchecked::Patched { path: p.clone(), digest: crate::trust::content_digest(mailbox) }),
        None => Ok(()),
    }
}

/// Whether `bytes` is what `git am` takes as a mailbox (its patch-format detection): a `From `
/// line or mail headers first, and not an stgit patch, stgit series or hg export.
fn is_mailbox(bytes: &[u8]) -> bool {
    let text = String::from_utf8_lossy(&bytes[..bytes.len().min(4096)]);
    let mut lines = text.lines().map(|l| l.trim_end_matches('\r'));
    let (l1, l2, l3) = (lines.next().unwrap_or(""), lines.next().unwrap_or(""), lines.next().unwrap_or(""));
    if l1.starts_with("# This series applies on GIT commit") || l1 == "# HG changeset patch" {
        return false;
    }
    if !l1.is_empty() && l2.is_empty() && (l3.starts_with("From:") || l3.starts_with("Author:") || l3.starts_with("Date:")) {
        return false;
    }
    let header =
        |l: &str| l.split_once(':').is_some_and(|(k, _)| !k.is_empty() && k.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'));
    l1.starts_with("From ") || header(l1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_mailbox_is_read_as_one() {
        assert!(is_mailbox(b"From 1a2b Mon Sep 17 00:00:00 2001\nFrom: a\n"));
        assert!(is_mailbox(b"From: a <a@b>\nSubject: x\n"));
        assert!(!is_mailbox(b"# HG changeset patch\n# User a\n"));
        assert!(!is_mailbox(b"# This series applies on GIT commit 1a2b\nx.patch\n"));
        assert!(!is_mailbox(b"Fix the thing\n\nFrom: a <a@b>\n"), "stgit");
        assert!(!is_mailbox(b"diff --git a/x b/x\n"), "a bare diff: git can't detect it either");
    }

    #[test]
    fn a_patch_reaches_a_folder_under_any_spelling() {
        let follows = [Follow { path: ".githooks".into(), tracked: None, hooks: true }];
        assert!(touches(".GITHOOKS/post-applypatch", &follows));
        assert!(touches(".githooks./x", &follows));
        assert!(touches("GITHO~1/x", &follows), "a short name only the volume resolves");
        assert!(!touches("src/main.rs", &follows));
    }
}
