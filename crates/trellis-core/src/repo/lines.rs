//! Line-level staging: stage, unstage or discard some of a hunk's changed lines by building a
//! patch that holds only those lines and applying it to the index or the working tree. With
//! whitespace changes hidden, whole hunks go this way too (`apply_shown`, with `shown_ws`).

use super::diff::{file_patch, hunk_of, target_diff};
use super::shown_ws::{changed, exact_context, raw_lines, side_lines};
use super::{DiffTarget, Hunk, HunkOp, LoadDiffOptions};
use crate::error::{Error, Result};
use git2::{ApplyLocation, Delta, Diff, Repository};
use std::collections::BTreeSet;
use std::path::Path;

/// Applies the `+`/`-` lines at `lines` (indexes into the hunk's lines, as `load_diff` returns
/// them) of hunk `hunk_index`. `expected` is that hunk as the user saw it: unless git produces
/// exactly the same hunk now (header and every line), the indexes may point at other lines, so
/// the selection is refused as stale with `NotFound`. Stage and Discard act on the unstaged
/// diff, Unstage on the staged one.
pub fn apply_lines(workdir: &Path, path: &str, hunk_index: usize, expected: &Hunk, lines: &[usize], op: HunkOp) -> Result<()> {
    apply_in(workdir, path, hunk_index, expected, Some(lines), op, false)
}

/// `apply_lines` (or, with `lines` `None`, the whole hunk) on a diff shown with whitespace
/// changes hidden (`LoadDiffOptions::ignore_whitespace`), whose hunk and line indexes it counts.
/// Only the changes shown are applied: unchanged lines keep the whitespace they have on the
/// side being patched, so hidden whitespace changes stay as they are. Refused with
/// `Unsupported` in the rare case that an unchanged line of the hunk differs from the file by
/// more than whitespace.
pub fn apply_shown(workdir: &Path, path: &str, hunk_index: usize, expected: &Hunk, lines: Option<&[usize]>, op: HunkOp) -> Result<()> {
    apply_in(workdir, path, hunk_index, expected, lines, op, true)
}

fn apply_in(
    workdir: &Path,
    path: &str,
    hunk_index: usize,
    expected: &Hunk,
    lines: Option<&[usize]>,
    op: HunkOp,
    ignore_whitespace: bool,
) -> Result<()> {
    if op == HunkOp::Discard {
        super::collide::refuse(workdir, path, "discard changes to")?;
    }
    let repo = Repository::open(workdir)?;
    if op != HunkOp::Unstage {
        super::attrs::refuse_converted(&repo, path, if op == HunkOp::Stage { "staging" } else { "discarding" })?;
    }
    // Always the forward diff that `load_diff` shows, so the line indexes match; the patch
    // builder swaps the roles of `+` and `-` when the patch goes the other way.
    let shown = if op == HunkOp::Unstage { DiffTarget::Staged(path.into()) } else { DiffTarget::Unstaged(path.into()) };
    let (location, reverse) = match op {
        HunkOp::Stage => (ApplyLocation::Index, false),
        HunkOp::Discard => (ApplyLocation::WorkDir, true),
        HunkOp::Unstage => (ApplyLocation::Index, true),
    };
    let diff = target_diff(&repo, &shown, LoadDiffOptions { ignore_whitespace, ..Default::default() }, None)?;
    let again = if lines.is_some() { "select the lines again" } else { "try again on the updated diff" };
    let stale = || Error::NotFound(format!("the diff changed; {again}"));
    let patch = file_patch(&diff, path)?.ok_or_else(stale)?;
    if hunk_index >= patch.num_hunks() {
        return Err(stale());
    }
    if hunk_of(&patch, hunk_index)? != *expected {
        return Err(stale());
    }
    let (hunk, _) = patch.hunk(hunk_index)?;
    let raw = raw_lines(&patch, hunk_index)?;
    let selected: BTreeSet<usize> = match lines {
        Some(l) => l.iter().copied().filter(|&i| raw.get(i).is_some_and(|r| matches!(r.origin, '+' | '-'))).collect(),
        None => changed(&raw),
    };
    if selected.is_empty() {
        return Err(Error::NotFound("no changed lines selected".into()));
    }
    let (content, selected) = if ignore_whitespace {
        let full = target_diff(&repo, &shown, LoadDiffOptions::default(), None)?;
        let full = file_patch(&full, path)?.ok_or_else(stale)?;
        exact_context(&raw, &side_lines(&full, reverse)?, reverse, &selected)?
    } else {
        (raw.into_iter().map(|r| (r.origin, r.bytes)).collect(), selected)
    };
    let pre_start = if reverse { hunk.new_start() } else { hunk.old_start() };

    // The side being patched is the old side going forward and the new side in reverse.
    let delta = patch.delta();
    let mode = |new: bool| i32::from(if new { delta.new_file() } else { delta.old_file() }.mode()) as u32;
    let target = match (reverse, delta.status()) {
        (false, Delta::Untracked | Delta::Added) => Patched::Create(mode(true)),
        (true, Delta::Deleted) => Patched::Create(mode(false)),
        _ => Patched::Existing(mode(reverse)),
    };
    let borrowed: Vec<(char, &[u8])> = content.iter().map(|(o, c)| (*o, c.as_slice())).collect();
    let Some(bytes) = build_patch(path, target, pre_start, &borrowed, &selected, reverse) else { return Ok(()) };
    let partial = Diff::from_buffer(&bytes)?;
    match location {
        ApplyLocation::WorkDir => repo.apply(&partial, location, None)?,
        _ => super::exact::apply_to_index(&repo, &partial, path, &mut git2::ApplyOptions::new())?,
    }
    Ok(())
}

/// Whether the file being patched exists, with the mode it has (or gets, when created).
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Patched {
    Create(u32),
    Existing(u32),
}

#[derive(Clone, Copy, PartialEq)]
enum Role {
    Ctx,
    Rm,
    Add,
}

struct Line<'a> {
    index: usize,
    role: Role,
    text: &'a [u8],
    /// Empty when the line has no newline (the file's last line).
    eol: &'a [u8],
    /// The EOL to give this line if it stops being the last one.
    fallback: &'a [u8],
}

fn split_eol(content: &[u8]) -> (&[u8], &[u8]) {
    if let Some(text) = content.strip_suffix(b"\r\n") {
        (text, &content[text.len()..])
    } else if let Some(text) = content.strip_suffix(b"\n") {
        (text, &content[text.len()..])
    } else {
        (content, b"")
    }
}

/// A patch replacing one hunk's region with the selected lines applied: every line of the
/// region as it is (`-`), then every line as it should be (`+`). `lines` are the hunk's
/// (origin, raw content) pairs from the forward diff; with `reverse` the patch goes from new
/// to old, so `+` lines are the ones present and `-` lines the ones to bring back. Returns
/// `None` when the selection changes nothing.
pub(crate) fn build_patch(
    path: &str,
    target: Patched,
    pre_start: u32,
    lines: &[(char, &[u8])],
    selected: &BTreeSet<usize>,
    reverse: bool,
) -> Option<Vec<u8>> {
    let (pre, post) = images(lines, selected, reverse);
    (pre != post).then(|| emit(path, target, pre_start, &pre, &post))
}

/// One line of a patch image: its text and its line ending (empty for a last line without one).
pub(crate) type ImageLine<'a> = (&'a [u8], &'a [u8]);

/// A hunk's region before and after the selected lines are applied, as (text, EOL) lines: `pre`
/// is the side being patched (the old side going forward, the new side with `reverse`) and
/// `post` is it with the selection applied. `lines` are the hunk's (origin, raw content) pairs
/// from the forward diff.
pub(crate) fn images<'a>(
    lines: &[(char, &'a [u8])],
    selected: &BTreeSet<usize>,
    reverse: bool,
) -> (Vec<ImageLine<'a>>, Vec<ImageLine<'a>>) {
    let mut kept: Vec<Line> = Vec::with_capacity(lines.len());
    let mut last_eol: &[u8] = b"\n";
    for (index, &(origin, content)) in lines.iter().enumerate() {
        if matches!(origin, '=' | '<' | '>') {
            // "No newline at end of file" for the line before it.
            if let Some(prev) = kept.last_mut() {
                prev.eol = b"";
            }
            continue;
        }
        let (text, eol) = split_eol(content);
        let role = match (origin, reverse) {
            ('+', false) | ('-', true) => Role::Add,
            ('-', false) | ('+', true) => Role::Rm,
            _ => Role::Ctx,
        };
        kept.push(Line { index, role, text, eol, fallback: last_eol });
        if !eol.is_empty() {
            last_eol = eol;
        }
    }
    let pre: Vec<ImageLine> = kept.iter().filter(|l| l.role != Role::Add).map(|l| (l.text, l.eol)).collect();
    let post_lines: Vec<&Line> = kept
        .iter()
        .filter(|l| match l.role {
            Role::Ctx => true,
            Role::Rm => !selected.contains(&l.index),
            Role::Add => selected.contains(&l.index),
        })
        .collect();
    // Only the file's real last line may lack a newline; that line keeps its own state.
    let last = post_lines.len().saturating_sub(1);
    let post: Vec<ImageLine> =
        post_lines.iter().enumerate().map(|(i, l)| (l.text, if l.eol.is_empty() && i != last { l.fallback } else { l.eol })).collect();
    (pre, post)
}

/// A patch turning the whole of `old` into `new` (file contents in repository form), or `None`
/// when they are the same. An empty `old` is patched from line 0.
pub(crate) fn replace_patch(path: &str, target: Patched, old: &[u8], new: &[u8]) -> Option<Vec<u8>> {
    let (pre, post) = (whole_image(old), whole_image(new));
    (pre != post).then(|| emit(path, target, if old.is_empty() { 0 } else { 1 }, &pre, &post))
}

fn whole_image(content: &[u8]) -> Vec<ImageLine<'_>> {
    content.split_inclusive(|&c| c == b'\n').map(split_eol).collect()
}

/// The patch text replacing the `pre` lines at line `pre_start` with the `post` lines.
fn emit(path: &str, target: Patched, pre_start: u32, pre: &[ImageLine], post: &[ImageLine]) -> Vec<u8> {
    let a = quote_path(&format!("a/{path}"));
    let b = quote_path(&format!("b/{path}"));
    let mut out = format!("diff --git {a} {b}\n").into_bytes();
    match target {
        Patched::Create(mode) => out.extend(format!("new file mode {mode:o}\n--- /dev/null\n").bytes()),
        Patched::Existing(mode) => out.extend(format!("index 0000000..0000000 {:o}\n--- {a}\n", nonzero(mode)).bytes()),
    }
    let (n, m) = (pre.len() as u32, post.len() as u32);
    let post_start = if n == 0 { pre_start + 1 } else { pre_start };
    out.extend(format!("+++ {b}\n@@ -{pre_start},{n} +{post_start},{m} @@\n").bytes());
    let mut line = |sign: u8, &(text, eol): &ImageLine| {
        out.push(sign);
        out.extend_from_slice(text);
        if eol.is_empty() {
            out.extend_from_slice(b"\n\\ No newline at end of file\n");
        } else {
            out.extend_from_slice(eol);
        }
    };
    pre.iter().for_each(|l| line(b'-', l));
    post.iter().for_each(|l| line(b'+', l));
    out
}

/// Keeps the file's mode; an unknown one is a regular file.
fn nonzero(mode: u32) -> u32 {
    if mode == 0 { 0o100644 } else { mode }
}

/// Quotes a patch path the way git does (C-style, non-ASCII bytes in octal) when it holds a
/// quote, a backslash, a control character or a non-ASCII byte. Trailing whitespace is quoted
/// too, since the patch parser trims it from unquoted paths.
fn quote_path(p: &str) -> String {
    let needs = |c: u8| c == b'"' || c == b'\\' || !(0x20..0x7f).contains(&c);
    if !p.bytes().any(needs) && !p.ends_with([' ', '\t']) {
        return p.to_owned();
    }
    let mut q = String::from("\"");
    for c in p.bytes() {
        match c {
            b'"' => q.push_str("\\\""),
            b'\\' => q.push_str("\\\\"),
            b'\t' => q.push_str("\\t"),
            b'\n' => q.push_str("\\n"),
            b'\r' => q.push_str("\\r"),
            c if needs(c) => q.push_str(&format!("\\{c:03o}")),
            c => q.push(c as char),
        }
    }
    q.push('"');
    q
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(i: &[usize]) -> BTreeSet<usize> {
        i.iter().copied().collect()
    }

    fn build(lines: &[(char, &str)], sel: &[usize], reverse: bool, target: Patched, start: u32) -> Option<String> {
        let raw: Vec<(char, &[u8])> = lines.iter().map(|(o, t)| (*o, t.as_bytes())).collect();
        build_patch("f.txt", target, start, &raw, &set(sel), reverse).map(|b| String::from_utf8(b).unwrap())
    }

    const KEEP: Patched = Patched::Existing(0o100644);
    const HEAD: &str = "diff --git a/f.txt b/f.txt\nindex 0000000..0000000 100644\n--- a/f.txt\n+++ b/f.txt\n";

    #[test]
    fn forward_one_of_two_additions() {
        let lines = [(' ', "a\n"), ('+', "b\n"), ('+', "c\n"), (' ', "d\n")];
        let p = build(&lines, &[2], false, KEEP, 4).unwrap();
        assert_eq!(p, format!("{HEAD}@@ -4,2 +4,3 @@\n-a\n-d\n+a\n+c\n+d\n"));
    }

    #[test]
    fn removed_line_only_leaves_unselected_addition_out() {
        let lines = [(' ', "a\n"), ('-', "b\n"), ('+', "B\n")];
        let p = build(&lines, &[1], false, KEEP, 1).unwrap();
        assert_eq!(p, format!("{HEAD}@@ -1,2 +1,1 @@\n-a\n-b\n+a\n"));
    }

    #[test]
    fn unselected_removal_stays_in_post_image() {
        let lines = [('-', "b\n"), ('+', "B\n")];
        let p = build(&lines, &[1], false, KEEP, 3).unwrap();
        assert_eq!(p, format!("{HEAD}@@ -3,1 +3,2 @@\n-b\n+b\n+B\n"));
    }

    #[test]
    fn reverse_swaps_roles() {
        // Discarding `+B` from the working tree removes it; discarding `-b` brings it back.
        let lines = [(' ', "a\n"), ('-', "b\n"), ('+', "B\n")];
        let p = build(&lines, &[2], true, KEEP, 1).unwrap();
        assert_eq!(p, format!("{HEAD}@@ -1,2 +1,1 @@\n-a\n-B\n+a\n"));
        let p = build(&lines, &[1], true, KEEP, 1).unwrap();
        assert_eq!(p, format!("{HEAD}@@ -1,2 +1,3 @@\n-a\n-B\n+a\n+b\n+B\n"));
    }

    #[test]
    fn empty_region_creates_a_new_file() {
        let lines = [('+', "a\n"), ('+', "b\n"), ('+', "c\n")];
        let p = build(&lines, &[0, 2], false, Patched::Create(0o100755), 0).unwrap();
        let head = "diff --git a/f.txt b/f.txt\nnew file mode 100755\n--- /dev/null\n+++ b/f.txt\n";
        assert_eq!(p, format!("{head}@@ -0,0 +1,2 @@\n+a\n+c\n"));
    }

    #[test]
    fn nothing_effective_selected() {
        let lines = [(' ', "a\n"), ('+', "b\n")];
        assert_eq!(build(&lines, &[], false, KEEP, 1), None);
        assert_eq!(build(&lines, &[0], false, KEEP, 1), None, "context only");
    }

    #[test]
    fn missing_newline_moves_to_the_real_last_line() {
        // Old "x\na", new "x\nb", neither ending in a newline: staging only `+b`.
        let lines =
            [(' ', "x\n"), ('-', "a"), ('>', "\n\\ No newline at end of file\n"), ('+', "b"), ('<', "\n\\ No newline at end of file\n")];
        let p = build(&lines, &[3], false, KEEP, 1).unwrap();
        let nl = "\\ No newline at end of file\n";
        assert_eq!(p, format!("{HEAD}@@ -1,2 +1,3 @@\n-x\n-a\n{nl}+x\n+a\n+b\n{nl}"));
    }

    #[test]
    fn crlf_lines_keep_crlf() {
        let lines = [(' ', "a\r\n"), ('+', "b\r\n"), ('+', "c\r\n")];
        let p = build(&lines, &[1], false, KEEP, 1).unwrap();
        assert_eq!(p, format!("{HEAD}@@ -1,1 +1,2 @@\n-a\r\n+a\r\n+b\r\n"));
    }

    fn replace(old: &str, new: &str) -> Option<String> {
        replace_patch("f.txt", KEEP, old.as_bytes(), new.as_bytes()).map(|b| String::from_utf8(b).unwrap())
    }

    #[test]
    fn replace_patch_whole_file() {
        assert_eq!(replace("a\nb\n", "a\nb\n"), None);
        assert_eq!(replace("", "a\nb\n").unwrap(), format!("{HEAD}@@ -0,0 +1,2 @@\n+a\n+b\n"));
        assert_eq!(replace("a\nb\n", "a\n").unwrap(), format!("{HEAD}@@ -1,2 +1,1 @@\n-a\n-b\n+a\n"));
        let nl = "\\ No newline at end of file\n";
        assert_eq!(replace("a", "a\n").unwrap(), format!("{HEAD}@@ -1,1 +1,1 @@\n-a\n{nl}+a\n"));
        assert_eq!(replace("a\n", "b").unwrap(), format!("{HEAD}@@ -1,1 +1,1 @@\n-a\n+b\n{nl}"));
        assert_eq!(replace("a\r\nb\r\n", "a\r\n").unwrap(), format!("{HEAD}@@ -1,2 +1,1 @@\n-a\r\n-b\r\n+a\r\n"));
    }

    #[test]
    fn unusual_paths_are_quoted() {
        let raw: Vec<(char, &[u8])> = vec![('+', b"x\n")];
        let p = build_patch("say \"hi\".txt", KEEP, 0, &raw, &set(&[0]), false).unwrap();
        assert!(String::from_utf8(p).unwrap().starts_with("diff --git \"a/say \\\"hi\\\".txt\" \"b/say \\\"hi\\\".txt\"\n"));
        assert_eq!(quote_path("a/my file.txt"), "a/my file.txt");
        assert_eq!(quote_path("a/café"), "\"a/caf\\303\\251\"");
        assert_eq!(quote_path("a/back\\slash"), "\"a/back\\\\slash\"");
    }
}
