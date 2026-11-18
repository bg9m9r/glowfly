//! The old path of a renamed or copied file as text: the weak note after a file row's path, the
//! whole "old to new" sentence for its hover and accessible name, and what the note shows in a
//! tree. Core fills `FileEntry::renamed_from`; a copy is listed as status 'A' with it, so an old
//! path on an 'A' row is a copy and on any other row a rename. Pure: no drawing.

use glowfly_core::i18n::Lang;
use glowfly_core::repo::FileEntry;
use std::borrow::Cow;

/// A file's old path and whether the file is a copy of it rather than a rename.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OldPath<'a> {
    pub old: &'a str,
    pub copied: bool,
}

impl<'a> OldPath<'a> {
    /// The old path of `f`'s row, when it has one.
    pub fn of(f: &'a FileEntry) -> Option<OldPath<'a>> {
        f.renamed_from.as_deref().map(|old| OldPath { old, copied: f.status == 'A' })
    }

    /// The old path `old` of a file whose status letter is `status` (the diff header's).
    pub fn new(old: &'a str, status: Option<char>) -> OldPath<'a> {
        OldPath { old, copied: status == Some('A') }
    }

    /// The row's note: "from a.txt", or "copy of a.txt". `shown` is the old path as the row
    /// shows it (the whole path, or in a tree maybe only its name).
    pub fn note(&self, t: Lang, shown: &str) -> String {
        if self.copied {
            t.trf("files.old_path.copy_note", &[("old", &shown)])
        } else {
            t.trf("files.old_path.rename_note", &[("old", &shown)])
        }
    }

    /// The whole text: "a.txt to b.txt", or "b.txt, copied from a.txt".
    pub fn whole(&self, t: Lang, new: &str) -> String {
        if self.copied {
            t.trf("files.old_path.copy_whole", &[("old", &self.old), ("new", &new)])
        } else {
            t.trf("files.old_path.rename_whole", &[("old", &self.old), ("new", &new)])
        }
    }
}

/// The name of `f`'s row for hover and accessibility: the whole "old to new" text for a row with
/// an old path, else its bare path.
pub fn row_name<'a>(t: Lang, f: &'a FileEntry) -> Cow<'a, str> {
    match OldPath::of(f) {
        Some(o) => Cow::Owned(o.whole(t, &f.path)),
        None => Cow::Borrowed(&f.path),
    }
}

/// What a tree row's note shows of `old` for a file now at `new`: the old file name when both
/// are in the same folder, else the whole old path.
pub fn shown_in_tree<'a>(old: &'a str, new: &str) -> &'a str {
    let folder = |p: &str| p.rfind('/').map(|i| p[..i].to_owned());
    if folder(old) == folder(new) { super::file_tree::file_name(old) } else { old }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, status: char, from: Option<&str>) -> FileEntry {
        FileEntry { renamed_from: from.map(str::to_owned), ..FileEntry::new(path, status) }
    }

    #[test]
    fn a_row_has_an_old_path_only_when_core_filled_one() {
        assert_eq!(OldPath::of(&entry("b.txt", 'M', None)), None);
        assert_eq!(OldPath::of(&entry("b.txt", 'R', Some("a.txt"))), Some(OldPath { old: "a.txt", copied: false }));
        assert_eq!(OldPath::of(&entry("b.txt", 'A', Some("a.txt"))), Some(OldPath { old: "a.txt", copied: true }));
        assert!(OldPath::new("a.txt", Some('A')).copied);
        assert!(!OldPath::new("a.txt", Some('R')).copied);
        assert!(!OldPath::new("a.txt", None).copied);
    }

    #[test]
    fn the_note_and_the_whole_text_say_rename_or_copy() {
        let rename = OldPath::new("a.txt", Some('R'));
        let copy = OldPath::new("a.txt", Some('A'));
        assert_eq!(rename.note(Lang::En, "a.txt"), "from a.txt");
        assert_eq!(rename.whole(Lang::En, "b.txt"), "a.txt to b.txt");
        assert_eq!(copy.note(Lang::En, "a.txt"), "copy of a.txt");
        assert_eq!(copy.whole(Lang::En, "b.txt"), "b.txt, copied from a.txt");
        assert_eq!(rename.note(Lang::De, "a.txt"), "vorher a.txt");
        assert_eq!(rename.whole(Lang::De, "b.txt"), "a.txt zu b.txt");
        assert_eq!(copy.note(Lang::De, "a.txt"), "Kopie von a.txt");
        assert_eq!(rename.note(Lang::Es, "a.txt"), "antes a.txt");
        assert_eq!(copy.whole(Lang::Es, "b.txt"), "b.txt, copiado de a.txt");
        assert_eq!(rename.note(Lang::En, "src/a.txt"), "from src/a.txt", "the note says what it is given");
    }

    #[test]
    fn a_plain_row_keeps_its_bare_path() {
        let plain = entry("b.txt", 'M', None);
        assert!(matches!(row_name(Lang::En, &plain), Cow::Borrowed("b.txt")));
        assert_eq!(row_name(Lang::En, &entry("b.txt", 'R', Some("a.txt"))), "a.txt to b.txt");
        assert_eq!(row_name(Lang::En, &entry("b.txt", 'A', Some("a.txt"))), "b.txt, copied from a.txt");
    }

    #[test]
    fn a_tree_row_shows_the_old_name_only_in_the_same_folder() {
        assert_eq!(shown_in_tree("src/a.rs", "src/b.rs"), "a.rs");
        assert_eq!(shown_in_tree("lib/a.rs", "src/b.rs"), "lib/a.rs");
        assert_eq!(shown_in_tree("a.rs", "src/b.rs"), "a.rs");
        assert_eq!(shown_in_tree("src/a.rs", "b.rs"), "src/a.rs");
        assert_eq!(shown_in_tree("a.rs", "b.rs"), "a.rs");
        assert_eq!(shown_in_tree("src/ui/a.rs", "src/b.rs"), "src/ui/a.rs");
    }
}
