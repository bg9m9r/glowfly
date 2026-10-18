//! Reading an index conflict: the three stages, merged into clean runs and conflict chunks,
//! plus names for the two sides taken from the operation in progress.

use crate::error::{Error, Result};
use git2::{BranchType, ErrorCode, IndexEntry, MergeFileOptions, Oid, Repository, RepositoryState};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    Ours,
    Theirs,
}

/// The index stage blob ids when the conflict was loaded; writing a resolution checks them.
#[derive(Clone, Debug, PartialEq)]
pub struct ConflictIds {
    pub ancestor: Option<Oid>,
    pub ours: Option<Oid>,
    pub theirs: Option<Oid>,
}

impl ConflictIds {
    pub fn side(&self, s: Side) -> Option<Oid> {
        match s {
            Side::Ours => self.ours,
            Side::Theirs => self.theirs,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Operation {
    Merge,
    Rebase,
    CherryPick,
    Revert,
    /// A stash, a patch or anything else whose source has no name.
    Other,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SideLabels {
    pub operation: Operation,
    pub ours: String,
    pub theirs: String,
}

/// One conflicting region. Every line keeps its terminator, so concatenation is exact.
#[derive(Clone, Debug, PartialEq)]
pub struct Chunk {
    pub base: Vec<String>,
    pub ours: Vec<String>,
    pub theirs: Vec<String>,
}

impl Chunk {
    pub fn side(&self, s: Side) -> &[String] {
        match s {
            Side::Ours => &self.ours,
            Side::Theirs => &self.theirs,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Segment {
    Clean(Vec<String>),
    Conflict(Chunk),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ConflictContent {
    Text(Vec<Segment>),
    Binary,
    /// One side deleted the file; the other changed it.
    DeletedBy(Side),
    /// Content that can't be merged line by line (the reason, e.g. "a submodule").
    Unsupported(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConflictFile {
    pub path: String,
    pub ids: ConflictIds,
    pub labels: SideLabels,
    pub content: ConflictContent,
    /// Both sides added the file (no common ancestor).
    pub no_ancestor: bool,
    /// `merge.conflictStyle` is diff3 or zdiff3: markers written for chunks not picked yet
    /// include the base.
    pub base_markers: bool,
}

impl ConflictFile {
    pub fn chunks(&self) -> impl Iterator<Item = &Chunk> {
        let segs: &[Segment] = match &self.content {
            ConflictContent::Text(s) => s,
            _ => &[],
        };
        segs.iter().filter_map(|s| match s {
            Segment::Conflict(c) => Some(c),
            Segment::Clean(_) => None,
        })
    }

    /// Conflict chunks to pick a side for; 0 for anything but text.
    pub fn chunk_count(&self) -> usize {
        self.chunks().count()
    }
}

/// Reads the conflict at `path` (repo-relative, `/`-separated) from the index.
pub fn load_conflict(workdir: &Path, path: &str) -> Result<ConflictFile> {
    let repo = Repository::open(workdir)?;
    let index = repo.index()?;
    let conflict = match index.conflict_get(Path::new(path)) {
        Ok(c) => c,
        Err(e) if e.code() == ErrorCode::NotFound => return Err(Error::NotFound(format!("{path} is not conflicted"))),
        Err(e) => return Err(e.into()),
    };
    let ids = ConflictIds {
        ancestor: conflict.ancestor.as_ref().map(|e| e.id),
        ours: conflict.our.as_ref().map(|e| e.id),
        theirs: conflict.their.as_ref().map(|e| e.id),
    };
    let no_ancestor = conflict.ancestor.is_none();
    let content = match (&conflict.our, &conflict.their) {
        (None, _) => ConflictContent::DeletedBy(Side::Ours),
        (_, None) => ConflictContent::DeletedBy(Side::Theirs),
        (Some(ours), Some(theirs)) => merged(&repo, conflict.ancestor.as_ref(), ours, theirs)?,
    };
    let style = repo.config().and_then(|c| c.get_string("merge.conflictStyle")).unwrap_or_default();
    let base_markers = matches!(style.as_str(), "diff3" | "zdiff3");
    Ok(ConflictFile { path: path.to_owned(), ids, labels: side_labels(&repo), content, no_ancestor, base_markers })
}

/// Merges both sides with markers and splits the result into segments.
fn merged(repo: &Repository, ancestor: Option<&IndexEntry>, ours: &IndexEntry, theirs: &IndexEntry) -> Result<ConflictContent> {
    let present: Vec<&IndexEntry> = ancestor.into_iter().chain([ours, theirs]).collect();
    for e in &present {
        match e.mode & 0o170000 {
            0o120000 => return Ok(ConflictContent::Unsupported("a symbolic link".into())),
            0o160000 => return Ok(ConflictContent::Unsupported("a submodule".into())),
            _ => {}
        }
    }
    let mut texts = Vec::new();
    for e in &present {
        let blob = repo.find_blob(e.id)?;
        if blob.is_binary() {
            return Ok(ConflictContent::Binary);
        }
        match std::str::from_utf8(blob.content()) {
            Ok(t) => texts.push(t.to_owned()),
            Err(_) => return Ok(ConflictContent::Unsupported("not UTF-8 text".into())),
        }
    }
    let n = marker_size(texts.iter().map(String::as_str));
    let mut opts = MergeFileOptions::new();
    opts.style_zdiff3(true).marker_size(n as u16).our_label("ours").their_label("theirs").ancestor_label("base");
    // Both sides added the file: merge against an empty ancestor.
    let empty;
    let ancestor = match ancestor {
        Some(a) => a,
        None => {
            empty = IndexEntry {
                ctime: ours.ctime,
                mtime: ours.mtime,
                dev: ours.dev,
                ino: ours.ino,
                mode: ours.mode,
                uid: ours.uid,
                gid: ours.gid,
                file_size: 0,
                id: repo.blob(&[])?,
                flags: ours.flags,
                flags_extended: ours.flags_extended,
                path: ours.path.clone(),
            };
            &empty
        }
    };
    let result = repo.merge_file_from_index(ancestor, ours, theirs, Some(&mut opts))?;
    let Ok(text) = std::str::from_utf8(result.content()) else {
        return Ok(ConflictContent::Unsupported("not UTF-8 text".into()));
    };
    let mut segs = parse_merged(text, n)?;
    // libgit2 ends a side that lacks a final newline with one before the next marker.
    let (our_text, their_text) = (&texts[texts.len() - 2], &texts[texts.len() - 1]);
    if let Some(Segment::Conflict(c)) = segs.last_mut() {
        for (lines, src) in [(&mut c.ours, our_text), (&mut c.theirs, their_text)] {
            if !src.is_empty()
                && !src.ends_with('\n')
                && let Some(last) = lines.last_mut()
            {
                let trimmed = last.trim_end_matches('\n').trim_end_matches('\r').len();
                last.truncate(trimmed);
            }
        }
    }
    Ok(ConflictContent::Text(segs))
}

/// A marker size no line of `texts` can be mistaken for: one more than the longest run of
/// `<`, `|`, `=` or `>` that starts a line, and at least 7.
pub fn marker_size<'a>(texts: impl Iterator<Item = &'a str>) -> usize {
    let mut longest = 0;
    for line in texts.flat_map(str::lines) {
        if let Some(c) = line.chars().next().filter(|c| matches!(c, '<' | '|' | '=' | '>')) {
            longest = longest.max(line.chars().take_while(|&x| x == c).count());
        }
    }
    (longest + 1).max(7)
}

/// Splits libgit2's merged text (zdiff3 style, labels "ours", "base", "theirs", markers of
/// size `n`) into clean runs and conflict chunks.
pub(super) fn parse_merged(text: &str, n: usize) -> Result<Vec<Segment>> {
    #[derive(PartialEq)]
    enum State {
        Clean,
        Ours,
        Base,
        Theirs,
    }
    let marker = |c: char, label: &str| format!("{}{label}", c.to_string().repeat(n));
    let (start, base, mid, end) = (marker('<', " ours"), marker('|', " base"), marker('=', ""), marker('>', " theirs"));
    // libgit2 leaves the label off the base marker when there is no common ancestor.
    let bare_base = marker('|', "");
    let mut segs = Vec::new();
    let mut clean: Vec<String> = Vec::new();
    let mut chunk = Chunk { base: Vec::new(), ours: Vec::new(), theirs: Vec::new() };
    let mut state = State::Clean;
    let bad = || Error::Other("could not parse the merged file".into());
    for line in text.split_inclusive('\n') {
        let bare = line.trim_end_matches('\n').trim_end_matches('\r');
        match state {
            State::Clean if bare == start => {
                if !clean.is_empty() {
                    segs.push(Segment::Clean(std::mem::take(&mut clean)));
                }
                state = State::Ours;
            }
            State::Clean if bare == base || bare == bare_base || bare == mid || bare == end => return Err(bad()),
            State::Clean => clean.push(line.to_owned()),
            State::Ours if bare == base || bare == bare_base => state = State::Base,
            State::Ours | State::Base if bare == mid => state = State::Theirs,
            State::Ours => chunk.ours.push(line.to_owned()),
            State::Base => chunk.base.push(line.to_owned()),
            State::Theirs if bare == end => {
                segs.push(Segment::Conflict(std::mem::replace(
                    &mut chunk,
                    Chunk { base: Vec::new(), ours: Vec::new(), theirs: Vec::new() },
                )));
                state = State::Clean;
            }
            State::Theirs => chunk.theirs.push(line.to_owned()),
        }
    }
    if state != State::Clean {
        return Err(bad());
    }
    if !clean.is_empty() {
        segs.push(Segment::Clean(clean));
    }
    Ok(segs)
}

/// Names for the two sides, from the operation in progress. Never fails: anything unreadable
/// falls back to a short sha or a generic label.
fn side_labels(repo: &Repository) -> SideLabels {
    let git_dir = repo.path();
    let read = |p: &Path| std::fs::read_to_string(p).ok().map(|s| s.lines().next().unwrap_or("").trim().to_owned());
    let oid_in = |p: &Path| read(p).and_then(|s| Oid::from_str(&s).ok());
    let head_name = match repo.head() {
        Ok(h) if h.is_branch() => h.shorthand().unwrap_or("ours").to_owned(),
        Ok(h) => h.target().map(|o| name_of(repo, o)).unwrap_or_else(|| "ours".into()),
        Err(_) => "ours".into(),
    };
    let short = |o: Oid| o.to_string()[..7].to_owned();
    use RepositoryState::*;
    match repo.state() {
        Merge => SideLabels {
            operation: Operation::Merge,
            ours: head_name,
            theirs: oid_in(&git_dir.join("MERGE_HEAD")).map(|o| name_of(repo, o)).unwrap_or_else(|| "theirs".into()),
        },
        Rebase | RebaseInteractive | RebaseMerge => {
            let dir = if git_dir.join("rebase-merge").is_dir() { git_dir.join("rebase-merge") } else { git_dir.join("rebase-apply") };
            let theirs = read(&dir.join("head-name"))
                .map(|h| h.strip_prefix("refs/heads/").unwrap_or(&h).to_owned())
                .filter(|h| !h.is_empty())
                .unwrap_or_else(|| "theirs".into());
            let ours = oid_in(&dir.join("onto")).map(|o| name_of(repo, o)).unwrap_or_else(|| "ours".into());
            SideLabels { operation: Operation::Rebase, ours, theirs }
        }
        CherryPick | CherryPickSequence => {
            let theirs = oid_in(&git_dir.join("CHERRY_PICK_HEAD"))
                .map(|o| {
                    let summary = repo.find_commit(o).ok().and_then(|c| c.summary().map(|s| s.chars().take(40).collect::<String>()));
                    match summary {
                        Some(s) => format!("{} {s}", short(o)),
                        None => short(o),
                    }
                })
                .unwrap_or_else(|| "theirs".into());
            SideLabels { operation: Operation::CherryPick, ours: head_name, theirs }
        }
        Revert | RevertSequence => {
            let theirs = oid_in(&git_dir.join("REVERT_HEAD")).map(|o| format!("revert of {}", short(o))).unwrap_or_else(|| "theirs".into());
            SideLabels { operation: Operation::Revert, ours: head_name, theirs }
        }
        ApplyMailbox => {
            let theirs = crate::patch_ops::current_subject(git_dir).map(|s| format!("patch: {s}")).unwrap_or_else(|| "patch".into());
            SideLabels { operation: Operation::Other, ours: head_name, theirs }
        }
        _ => SideLabels { operation: Operation::Other, ours: head_name, theirs: "stash or patch".into() },
    }
}

/// The first local branch, remote branch or tag pointing at `oid`, else its short sha.
fn name_of(repo: &Repository, oid: Oid) -> String {
    let points_here = |r: &git2::Reference| r.peel_to_commit().is_ok_and(|c| c.id() == oid);
    for kind in [BranchType::Local, BranchType::Remote] {
        if let Ok(branches) = repo.branches(Some(kind)) {
            for (b, _) in branches.flatten() {
                let r = b.get();
                if let Some(name) = r.shorthand()
                    && !name.ends_with("/HEAD")
                    && points_here(r)
                {
                    return name.to_owned();
                }
            }
        }
    }
    if let Ok(tags) = repo.references_glob("refs/tags/*") {
        for r in tags.flatten() {
            if points_here(&r)
                && let Some(name) = r.shorthand()
            {
                return name.to_owned();
            }
        }
    }
    oid.to_string()[..7].to_owned()
}
