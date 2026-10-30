//! The paths git's `CONFLICT (…)` lines name, for counting a stopped operation's conflicted
//! files when the refreshed status isn't there to count them (git_errors.rs). Each line names
//! the paths it leaves unmerged in the index: a content or add/add conflict its file, a
//! modify/delete its file, a rename/delete the new name, a rename/rename both new names and the
//! old one, a file/directory or file location conflict the path git moved it to.
//!
//! Paths are printed raw, so one holding a line's own words (" in ", " renamed to ") can be cut
//! more than one way, and so can a rebase's side label ("1a2b3c4 (Fix typo in docs)"). A path
//! with one possible reading is named; one with several is kept as its readings, counted but not
//! named, until the status's unmerged paths tell which one it is (`ordered`). Pure.

/// The paths of `lines`' CONFLICT lines, in order: (named, each once; the readings of each path
/// that can be read more than one way).
pub(crate) fn conflict_paths(lines: &[&str]) -> (Vec<String>, Vec<Vec<String>>) {
    let mut named: Vec<String> = Vec::new();
    let mut unsure: Vec<Vec<String>> = Vec::new();
    for l in lines.iter().filter_map(|l| l.strip_prefix("CONFLICT (")) {
        let Some((kind, text)) = l.split_once("): ") else { continue };
        for readings in paths(kind, text.trim_end()) {
            match readings.as_slice() {
                [] => {}
                [one] => {
                    if !named.iter().any(|x| x == one) {
                        named.push((*one).to_owned());
                    }
                }
                many => {
                    let many: Vec<String> = many.iter().map(|s| (*s).to_owned()).collect();
                    if !unsure.contains(&many) {
                        unsure.push(many);
                    }
                }
            }
        }
    }
    (named, unsure)
}

/// The conflicted paths of a status (`unmerged`), each once, in the order git's lines named them
/// (`named`, then each unsure path's reading that is unmerged), the others after them in
/// `unmerged`'s order. A name or reading the status doesn't list is left out.
pub fn ordered(named: &[String], unsure: &[Vec<String>], unmerged: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut add = |p: &String| {
        if unmerged.contains(p) && !out.contains(p) {
            out.push(p.clone());
        }
    };
    named.iter().for_each(&mut add);
    unsure.iter().filter_map(|r| r.iter().find(|p| unmerged.contains(p))).for_each(&mut add);
    unmerged.iter().for_each(add);
    out
}

/// The readings of each path one CONFLICT line of `kind` ("content", "rename/delete"…) says is
/// conflicted (none for a kind it doesn't know).
fn paths<'a>(kind: &str, text: &'a str) -> Vec<Vec<&'a str>> {
    if let Some(p) = text.strip_prefix("Merge conflict in ") {
        return vec![vec![p]];
    }
    let one = |r: Vec<&'a str>| vec![r];
    match kind {
        // "<old> renamed to <new> in <side>, but deleted in <side>."
        "rename/delete" => one(spans(text, " renamed to ", " in ", ", but deleted in ")),
        // "<old> renamed to <a> in <side> and to <b> in <side>." The old name comes last: it
        // is deleted on both sides, so there is no file to open.
        "rename/rename" => vec![
            spans(text, " renamed to ", " in ", " and to "),
            spans(text, " and to ", " in ", ""),
            before(text, " renamed to ", " and to "),
        ],
        // "<path> deleted in <side> and modified in <side>.  Version …"
        "modify/delete" => one(before(text, " deleted in ", " and modified in ")),
        // "directory in the way of <path> from <side>; moving it to <new> instead."
        "file/directory" => one(text.strip_suffix(" instead.").map(|t| after(t, "; moving it to ")).unwrap_or_default()),
        // "… suggesting it should perhaps be moved to <new>."
        "file location" => one(text.strip_suffix('.').map(|t| after(t, " moved to ")).unwrap_or_default()),
        _ => Vec::new(),
    }
}

/// Every text between a `start` and a later `end` in `text` that has `then` somewhere after it,
/// each once.
fn spans<'a>(text: &'a str, start: &str, end: &str, then: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    for (i, _) in text.match_indices(start) {
        let from = i + start.len();
        for (j, _) in text[from..].match_indices(end) {
            let rest = &text[from + j + end.len()..];
            let p = &text[from..from + j];
            if (then.is_empty() || rest.contains(then)) && !p.is_empty() && !out.contains(&p) {
                out.push(p);
            }
        }
    }
    out
}

/// Every text before an `anchor` in `text` that has `then` somewhere after it, each once.
fn before<'a>(text: &'a str, anchor: &str, then: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    for (i, _) in text.match_indices(anchor) {
        let p = &text[..i];
        if text[i + anchor.len()..].contains(then) && !p.is_empty() && !out.contains(&p) {
            out.push(p);
        }
    }
    out
}

/// Every text after an `anchor` in `text`, to its end, each once.
fn after<'a>(text: &'a str, anchor: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    for (i, _) in text.match_indices(anchor) {
        let p = &text[i + anchor.len()..];
        if !p.is_empty() && !out.contains(&p) {
            out.push(p);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{conflict_paths, ordered};

    fn read(output: &str) -> (Vec<String>, Vec<Vec<String>>) {
        conflict_paths(&output.lines().collect::<Vec<_>>())
    }

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| (*x).to_owned()).collect()
    }

    #[test]
    fn each_kind_names_its_unmerged_paths_once() {
        // What git 2.54 (merge-ort) printed for one merge, with `git ls-files -u` naming
        // b.txt, f~HEAD, r.txt, r1.txt and r2.txt.
        let out = "CONFLICT (rename/delete): a.txt renamed to b.txt in side, but deleted in HEAD.\n\
            CONFLICT (file/directory): directory in the way of f from HEAD; moving it to f~HEAD instead.\n\
            CONFLICT (modify/delete): f~HEAD deleted in side and modified in HEAD.  Version HEAD of f~HEAD left in tree.\n\
            CONFLICT (rename/rename): r.txt renamed to r2.txt in HEAD and to r1.txt in side.\n\
            Automatic merge failed; fix conflicts and then commit the result.";
        assert_eq!(read(out), (s(&["b.txt", "f~HEAD", "r2.txt", "r1.txt", "r.txt"]), vec![]));
    }

    #[test]
    fn content_add_add_and_file_location() {
        let out = "CONFLICT (content): Merge conflict in a b.txt\n\
            CONFLICT (add/add): Merge conflict in new.txt\n\
            CONFLICT (content): Merge conflict in a b.txt\n\
            CONFLICT (file location): x.txt added in side inside a directory that was renamed in HEAD, suggesting it should perhaps be moved to lib/x.txt.";
        assert_eq!(read(out), (s(&["a b.txt", "new.txt", "lib/x.txt"]), vec![]));
    }

    #[test]
    fn paths_and_side_labels_holding_the_lines_words() {
        // " in." isn't " in ": one reading.
        let sign_in = "CONFLICT (rename/delete): docs/a.md renamed to docs/Sign in.md in side, but deleted in HEAD.";
        assert_eq!(read(sign_in), (s(&["docs/Sign in.md"]), vec![]));
        // A rebase's side label with " in " in its subject still leaves one reading.
        let label = "CONFLICT (rename/delete): a.txt renamed to b.txt in HEAD, but deleted in 1a2b3c4 (Fix typo in docs).";
        assert_eq!(read(label), (s(&["b.txt"]), vec![]));
        let modified = "CONFLICT (modify/delete): f.txt deleted in 1a2b3c4 (Move it in here) and modified in HEAD.  Version HEAD of f.txt left in tree.";
        assert_eq!(read(modified), (s(&["f.txt"]), vec![]));
        // A new name holding " in " itself: counted, named only once the status tells.
        let cut = "CONFLICT (rename/delete): a.md renamed to docs/log in to it.md in side, but deleted in HEAD.";
        let (named, unsure) = read(cut);
        assert_eq!((named.len(), unsure.clone()), (0, vec![s(&["docs/log", "docs/log in to it.md"])]));
        let unmerged = s(&["z.txt", "docs/log in to it.md"]);
        assert_eq!(ordered(&named, &unsure, &unmerged), s(&["docs/log in to it.md", "z.txt"]));
        // A reading the status doesn't list is left out.
        assert_eq!(ordered(&s(&["gone.txt"]), &[s(&["x", "y"])], &s(&["z.txt"])), s(&["z.txt"]));
    }

    #[test]
    fn ordered_follows_git_then_the_status() {
        let unmerged = s(&["a", "b", "c"]);
        assert_eq!(ordered(&s(&["c", "a"]), &[], &unmerged), s(&["c", "a", "b"]));
    }

    #[test]
    fn unknown_kinds_name_nothing() {
        assert_eq!(read("CONFLICT (distinct types): odd"), (vec![], vec![]));
        assert_eq!(read("CONFLICT (content)"), (vec![], vec![]));
    }
}
