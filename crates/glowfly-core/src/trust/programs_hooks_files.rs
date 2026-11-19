//! What a file a hook runs is, for `programs_hooks.rs`: whether it is in the working tree
//! (`in_tree`, `rel_text`) or among installed packages (`in_packages`), whether it is a shell script whose
//! words are read in turn (`is_shell`), its text (`text_of_file`), and where a link to it leads
//! (`link_end`).

use super::runs::SHELLS;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

/// Whether `path` is in the working tree and not in a `.git` folder at its top: a file there is
/// one the branch may carry. `..` parts are applied as written (`.husky/_/../check.sh`); one
/// that climbs out is outside.
pub(super) fn in_tree(workdir: &Path, path: &Path) -> bool {
    let Some(parts) = parts_in(workdir, path) else { return false };
    // `.git` as a folding file system may name it (`.GIT`, Windows' `.git.` and `.git `).
    parts.first().is_none_or(|first| super::super::super::follow::fold(first) != ".git")
}

/// `path` relative to `workdir`, `/`-separated, `..` applied; None when it is not there.
pub(super) fn rel_text(workdir: &Path, path: &Path) -> Option<String> {
    parts_in(workdir, path).map(|parts| parts.join("/"))
}

/// Whether `path` in the working tree is under a `node_modules` folder: installed packages,
/// whose files are followed by their bytes but never read for what they run.
pub(super) fn in_packages(workdir: &Path, path: &Path) -> bool {
    parts_in(workdir, path).is_some_and(|parts| parts.iter().any(|p| p.eq_ignore_ascii_case("node_modules")))
}

/// The names of `path` below `workdir`, `..` applied; None when it is not there. A name that
/// isn't UTF-8 is kept as `?`, which matches neither `.git` nor `node_modules`.
fn parts_in(workdir: &Path, path: &Path) -> Option<Vec<String>> {
    let rel = path.strip_prefix(workdir).ok()?;
    let mut parts: Vec<String> = Vec::new();
    for c in rel.components() {
        match c {
            Component::Normal(name) => parts.push(name.to_str().unwrap_or("?").to_owned()),
            Component::ParentDir if parts.pop().is_some() => {}
            Component::CurDir => {}
            _ => return None,
        }
    }
    Some(parts)
}

/// Whether the file at `path`, whose first bytes are `head` (`super::HEAD_BYTES` of them), is a
/// shell script: named `*.sh`, or starting with a `#!` line that runs sh or a shell like it
/// (`#!/bin/bash`, `#!/usr/bin/env -S zsh -e`). A `#!` line whose program can't be told counts
/// as one: one longer than `super::MAX_LINE_BYTES` (a system that can't read it all leaves the
/// file to sh, and one that reads further may find a shell), or one that names no program.
pub(super) fn is_shell(path: &Path, head: Option<&[u8]>) -> bool {
    if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("sh")) {
        return true;
    }
    let Some(rest) = head.and_then(|h| h.strip_prefix(b"#!")) else { return false };
    let Some(end) = rest.iter().position(|b| *b == b'\n').or((rest.len() + 2 <= super::MAX_LINE_BYTES).then_some(rest.len())) else {
        return true;
    };
    let line = String::from_utf8_lossy(&rest[..end]).into_owned();
    let mut words = line.split_whitespace().map(|w| w.rsplit(['/', '\\']).next().unwrap_or(w));
    let mut program = words.next();
    if program == Some("env") {
        program = words.find(|w| !w.starts_with('-') && !w.contains('='));
    }
    program.is_none_or(|p| SHELLS.contains(&p))
}

/// The first `max` bytes of the file at `path`, and whether they are all of it; None when it
/// can't be read.
pub(super) fn text_of_file(path: &Path, max: usize) -> Option<(Vec<u8>, bool)> {
    let mut bytes = Vec::new();
    std::fs::File::open(path).ok()?.take(max as u64 + 1).read_to_end(&mut bytes).ok()?;
    let whole = bytes.len() <= max;
    bytes.truncate(max);
    Some((bytes, whole))
}

/// Where the link at `path` leads.
pub(super) enum LinkEnd {
    /// A regular file in the working tree: what the link runs, read through it.
    File(PathBuf),
    /// A place that may not be followed (`follow::far`): never looked at.
    Far(PathBuf),
    /// Anywhere else: outside the working tree, a folder, or nothing.
    Elsewhere,
}

/// Where the link at `path` in `workdir` leads, its target checked before anything is read
/// there.
pub(super) fn link_end(workdir: &Path, path: &Path) -> LinkEnd {
    let far = |p: &Path| super::super::super::follow::far(p, workdir);
    let Ok(target) = std::fs::read_link(path) else { return LinkEnd::Elsewhere };
    let spelled = path.parent().map_or(target.clone(), |dir| dir.join(&target));
    if far(&spelled) {
        return LinkEnd::Far(spelled);
    }
    let real = crate::paths::resolve(path);
    if far(&real) {
        return LinkEnd::Far(real);
    }
    let top = crate::paths::resolve(workdir);
    if in_tree(&top, &real) && std::fs::symlink_metadata(&real).is_ok_and(|m| m.is_file()) {
        LinkEnd::File(real)
    } else {
        LinkEnd::Elsewhere
    }
}
