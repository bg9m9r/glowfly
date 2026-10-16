//! Conflicts that were resolved during the operation in progress, from git's resolve-undo
//! record (the index's REUC extension): listing them, marking conflicted files resolved, and
//! bringing a resolved file's conflict back (`git checkout -m`).

use super::{CLI_PATH_BYTES, FileEntry, conflict_markers, rpath};
use crate::error::{Error, Result};
use crate::gitcli::{NO_PROMPTS, READ_TIMEOUT, args, output_bytes};
use std::path::Path;

/// A path git holds resolve-undo data for: it was conflicted and has been resolved.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedFile {
    /// The path (status ' '), with `Hazard::NotUtf8` for a name that isn't UTF-8.
    pub entry: FileEntry,
    /// Both the ours and theirs versions were recorded, so the conflict can be brought back.
    /// False for a modify/delete conflict.
    pub both_sides: bool,
}

/// What the WIP panel shows about conflicts: the marker regions left in each conflicted file
/// (`conflict_markers`), and the files resolved so far.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConflictSummary {
    pub counts: Vec<(String, Option<usize>)>,
    pub resolved: Vec<ResolvedFile>,
}

/// The paths with resolve-undo data, in git's order (`git ls-files --resolve-undo`). Only reads
/// the index: no hook or filter runs, so this is safe in a repository that isn't trusted.
pub fn resolved_paths(workdir: &Path) -> Result<Vec<ResolvedFile>> {
    let cmd = args(&["-c", "core.fsmonitor=false", "ls-files", "--resolve-undo", "-z"]);
    let (out, stdout) = output_bytes(workdir, &cmd, NO_PROMPTS, Some(READ_TIMEOUT));
    if !out.ok {
        let why = out.stderr.trim();
        return Err(Error::Other(if why.is_empty() { "git ls-files --resolve-undo failed".into() } else { why.to_owned() }));
    }
    Ok(parse(&stdout))
}

/// Parses `<mode> SP <oid> SP <stage> TAB <path>` records, each ending in a NUL. The path is
/// taken whole (never trimmed); a path's records are grouped, keeping the stages seen.
fn parse(out: &[u8]) -> Vec<ResolvedFile> {
    let mut found: Vec<(&[u8], [bool; 4])> = Vec::new();
    for rec in out.split(|&b| b == 0).filter(|r| !r.is_empty()) {
        let Some(tab) = rec.iter().position(|&b| b == b'\t') else { continue };
        let (meta, path) = (&rec[..tab], &rec[tab + 1..]);
        let stage = meta.rsplit(|&b| b == b' ').next().and_then(|s| std::str::from_utf8(s).ok()).and_then(|s| s.parse::<usize>().ok());
        let Some(stage @ 1..=3) = stage else { continue };
        match found.last_mut() {
            Some((p, stages)) if *p == path => stages[stage] = true,
            _ => {
                let mut stages = [false; 4];
                stages[stage] = true;
                found.push((path, stages));
            }
        }
    }
    found.into_iter().map(|(path, s)| ResolvedFile { entry: rpath::entry(path, ' '), both_sides: s[2] && s[3] }).collect()
}

/// The marker counts of the `conflicted` paths, and with `with_resolved` the resolved files.
pub fn conflict_summary(workdir: &Path, conflicted: &[String], with_resolved: bool) -> Result<ConflictSummary> {
    let resolved = if with_resolved { resolved_paths(workdir)? } else { Vec::new() };
    Ok(ConflictSummary { counts: conflict_markers(workdir, conflicted), resolved })
}

/// Marks `paths` resolved by staging each working file as it is (`git add --all`), in as many
/// git runs as the command line needs.
pub fn mark_resolved(workdir: &Path, paths: &[String]) -> Result<()> {
    for batch in path_batches(paths, CLI_PATH_BYTES) {
        let mut a = vec!["add", "--all", "--"];
        a.extend(batch.iter().map(String::as_str));
        super::index::git(workdir, &a)?;
    }
    Ok(())
}

/// Brings back the conflict of each of `paths` from its resolve-undo record: git rewrites the
/// file with conflict markers from both sides (`git checkout -m`, so merge.conflictStyle,
/// filters and hooks apply), losing the staged resolution and any edits to the file. Refused,
/// with nothing changed, when any path has no record or lacks the ours or theirs version.
pub fn mark_conflicted(workdir: &Path, paths: &[String]) -> Result<()> {
    let resolved = resolved_paths(workdir)?;
    for p in paths {
        match resolved.iter().find(|r| r.entry.hazard.is_none() && r.entry.path == *p) {
            None => return Err(Error::Other(format!("{p} has no saved conflict to restore"))),
            Some(r) if !r.both_sides => {
                return Err(Error::Other(format!("git can only restore a conflict that has both versions ({p})")));
            }
            Some(_) => {}
        }
    }
    for batch in path_batches(paths, CLI_PATH_BYTES) {
        let mut a = vec!["checkout", "-m", "--"];
        a.extend(batch.iter().map(String::as_str));
        super::index::git(workdir, &a)?;
    }
    Ok(())
}

/// `paths` split into runs that take at most `max_bytes` written one after another with a
/// separator between each two; a path longer than that is a run of its own.
pub(super) fn path_batches(paths: &[String], max_bytes: usize) -> Vec<&[String]> {
    let mut out = Vec::new();
    let (mut start, mut size) = (0, 0);
    for (i, p) in paths.iter().enumerate() {
        let len = p.len() + 1;
        // `size` counts a separator after every path so far; the last one isn't written.
        if i > start && size + len - 1 > max_bytes {
            out.push(&paths[start..i]);
            (start, size) = (i, 0);
        }
        size += len;
    }
    if start < paths.len() {
        out.push(&paths[start..]);
    }
    out
}
