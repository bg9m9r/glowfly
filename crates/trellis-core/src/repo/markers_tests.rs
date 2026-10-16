use super::markers::scan_with_limit;
use super::*;
use crate::testutil::*;

#[test]
fn marker_regions_counts_complete_regions() {
    let merge = "a\n<<<<<<< ours\nmine\n=======\ntheirs\n>>>>>>> theirs\nb\n";
    let diff3 = "<<<<<<< HEAD\nmine\n||||||| base\nold\n=======\ntheirs\n>>>>>>> other\n";
    let table: [(&str, String, usize); 9] = [
        ("plain text", "just\nsome lines\n".into(), 0),
        ("a merge-style region", merge.into(), 1),
        ("a diff3-style region", diff3.into(), 1),
        ("two regions", format!("{merge}{diff3}"), 2),
        ("marker size 9", "<<<<<<<<< a\nx\n=========\ny\n>>>>>>>>> b\n".into(), 1),
        ("unclosed", "<<<<<<< a\nx\n=======\ny\n".into(), 0),
        ("a lone separator", "Title\n=======\ntext\n".into(), 0),
        ("CR LF endings", merge.replace('\n', "\r\n"), 1),
        ("mismatched sizes", "<<<<<<< a\nx\n=======\ny\n>>>>>>>>> b\n".into(), 0),
    ];
    for (what, text, want) in table {
        assert_eq!(marker_regions(text.as_bytes()), want, "{what}");
    }
    // A start inside a region starts it again; the end has no newline after it.
    assert_eq!(marker_regions(b"<<<<<<< a\nx\n<<<<<<< b\ny\n=======\nz\n>>>>>>> c"), 1);
    // Marker-like lines that aren't markers: no space after the run, or too short.
    assert_eq!(marker_regions(b"<<<<<<<x\n=======\n>>>>>>>\n"), 0);
    assert_eq!(marker_regions(b"<<<<<< a\n======\n>>>>>> b\n"), 0);
    // A separator with trailing whitespace still counts.
    assert_eq!(marker_regions(b"<<<<<<< a\nx\n=======  \ny\n>>>>>>> b\n"), 1);
}

#[test]
fn conflict_markers_reads_regular_text_files_only() {
    let dir = scratch_repo("markers-scan");
    write(&dir, "m.txt", "<<<<<<< a\nx\n=======\ny\n>>>>>>> b\n");
    write(&dir, "bin.dat", "<<<<<<< a\n\0\n=======\ny\n>>>>>>> b\n");
    std::fs::create_dir_all(dir.join("sub dir")).unwrap();
    write(&dir, "sub dir/clean.txt", "clean\n");
    let paths: Vec<String> = ["m.txt", "missing.txt", "bin.dat", "sub dir/clean.txt", "sub dir"].map(String::from).into();
    let found = conflict_markers(&dir, &paths);
    let counts: Vec<Option<usize>> = found.iter().map(|(_, n)| *n).collect();
    assert_eq!(counts, [Some(1), None, None, Some(0), None]);
    assert_eq!(found[3].0, "sub dir/clean.txt", "paths come back as given");
    // Over the size limit (a tiny one here rather than a 16 MB file).
    assert_eq!(scan_with_limit(&dir, "m.txt", 10), None);
    assert_eq!(scan_with_limit(&dir, "m.txt", 1000), Some(1));
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn conflict_markers_does_not_follow_links() {
    let dir = scratch_repo("markers-link");
    write(&dir, "m.txt", "<<<<<<< a\nx\n=======\ny\n>>>>>>> b\n");
    std::os::unix::fs::symlink("m.txt", dir.join("link.txt")).unwrap();
    let found = conflict_markers(&dir, &["link.txt".to_owned()]);
    assert_eq!(found, [("link.txt".to_owned(), None)]);
    let _ = std::fs::remove_dir_all(&dir);
}
