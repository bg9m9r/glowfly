//! The paths git's `CONFLICT (…)` lines name, for counting a stopped operation's conflicted
//! files when the refreshed status isn't there to count them (git_errors.rs). Each line names
//! the paths it leaves unmerged in the index: a content or add/add conflict its file, a
//! modify/delete its file, a rename/delete the new name, a rename/rename the old name and both
//! new ones, a file/directory or file location conflict the path git moved it to. Pure.

/// The files `lines`' CONFLICT lines name, in order, each once.
pub(crate) fn conflict_files(lines: &[&str]) -> Vec<String> {
    let mut files: Vec<String> = Vec::new();
    for l in lines.iter().filter_map(|l| l.strip_prefix("CONFLICT (")) {
        let Some((kind, text)) = l.split_once("): ") else { continue };
        for f in paths(kind, text.trim_end()) {
            let f = f.trim();
            if !f.is_empty() && !files.iter().any(|x| x == f) {
                files.push(f.to_owned());
            }
        }
    }
    files
}

/// The paths one CONFLICT line of `kind` ("content", "rename/delete"…) says are conflicted.
fn paths<'a>(kind: &str, text: &'a str) -> Vec<&'a str> {
    if let Some((_, p)) = text.split_once("Merge conflict in ") {
        return vec![p];
    }
    match kind {
        // "<old> renamed to <new> in <side>, but deleted in <side>."
        "rename/delete" => between(text, " renamed to ", " in ").into_iter().collect(),
        // "<old> renamed to <a> in <side> and to <b> in <side>."
        "rename/rename" => {
            let old = text.split_once(" renamed to ").map(|(o, _)| o);
            let ours = between(text, " renamed to ", " in ");
            let theirs = between(text, " and to ", " in ");
            [old, ours, theirs].into_iter().flatten().collect()
        }
        // "<path> deleted in <side> and modified in <side>.  Version …"
        "modify/delete" => text.split_once(" deleted in ").map(|(p, _)| p).into_iter().collect(),
        // "directory in the way of <path> from <side>; moving it to <new> instead."
        "file/directory" => between(text, "; moving it to ", " instead.").into_iter().collect(),
        // "… suggesting it should perhaps be moved to <new>."
        "file location" => text.rsplit_once(" moved to ").and_then(|(_, p)| p.strip_suffix('.')).into_iter().collect(),
        _ => Vec::new(),
    }
}

/// The text between the first `start` in `text` and the first `end` after it.
fn between<'a>(text: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let (_, rest) = text.split_once(start)?;
    rest.split_once(end).map(|(p, _)| p)
}

#[cfg(test)]
mod tests {
    use super::conflict_files;

    fn names(output: &str) -> Vec<String> {
        conflict_files(&output.lines().collect::<Vec<_>>())
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
        assert_eq!(names(out), ["b.txt", "f~HEAD", "r.txt", "r2.txt", "r1.txt"]);
    }

    #[test]
    fn content_add_add_and_file_location() {
        let out = "CONFLICT (content): Merge conflict in a b.txt\n\
            CONFLICT (add/add): Merge conflict in new.txt\n\
            CONFLICT (content): Merge conflict in a b.txt\n\
            CONFLICT (file location): x.txt added in side inside a directory that was renamed in HEAD, suggesting it should perhaps be moved to lib/x.txt.";
        assert_eq!(names(out), ["a b.txt", "new.txt", "lib/x.txt"]);
    }

    #[test]
    fn unknown_kinds_name_nothing() {
        assert!(names("CONFLICT (distinct types): odd").is_empty());
        assert!(names("CONFLICT (content)").is_empty());
    }
}
