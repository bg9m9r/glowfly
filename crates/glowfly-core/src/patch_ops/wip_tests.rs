//! Uncommitted changes saved as a patch: the command lines, splitting long path lists, and
//! round trips through `git apply` for each scope with binary, untracked and awkward names.

use super::*;
use crate::patch_ops::{PatchSource, SaveReport, build, save};
use crate::testutil::{git, scratch_repo, write};

const BASE: [&str; 9] = [
    "--literal-pathspecs",
    "diff",
    "--binary",
    "--no-color",
    "--no-ext-diff",
    "--no-textconv",
    "--submodule=short",
    "--src-prefix=a/",
    "--dst-prefix=b/",
];

fn shape(extra: &[&str]) -> Vec<String> {
    BASE.iter().chain(extra).map(|s| (*s).to_owned()).collect()
}

fn owned(p: &[&str]) -> Vec<String> {
    p.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn diff_cmd_shape() {
    assert_eq!(diff_cmd(WipScope::All, &[], false), shape(&["HEAD"]));
    assert_eq!(diff_cmd(WipScope::All, &[], true), shape(&[EMPTY_TREE]));
    assert_eq!(diff_cmd(WipScope::Staged, &[], false), shape(&["--cached"]));
    assert_eq!(diff_cmd(WipScope::Staged, &[], true), shape(&["--cached"]));
    assert_eq!(diff_cmd(WipScope::Unstaged, &[], false), shape(&[]));
    assert_eq!(diff_cmd(WipScope::Unstaged, &owned(&["-a", "b c"]), false), shape(&["--", "-a", "b c"]));
    assert_eq!(diff_cmd(WipScope::All, &owned(&["x"]), false), shape(&["HEAD", "--", "x"]));
}

#[test]
fn untracked_cmd_shape() {
    let want = [
        "diff",
        "--no-index",
        "--binary",
        "--no-color",
        "--no-ext-diff",
        "--no-textconv",
        "--src-prefix=a/",
        "--dst-prefix=b/",
        "--",
        "/dev/null",
        "-new.txt",
    ];
    assert_eq!(untracked_cmd("-new.txt"), want);
}

#[test]
fn chunks_split_on_the_budget_in_order() {
    let paths = owned(&["aaaa", "bbbb", "cccc", "dddddddddddd", "e"]);
    // Each path costs its length plus one.
    let runs = chunks(&paths, 10);
    assert_eq!(runs, [&paths[0..2], &paths[2..3], &paths[3..4], &paths[4..5]]);
    assert_eq!(chunks(&paths, 1000), [&paths[..]]);
    assert!(chunks(&[], 10).is_empty());
}

fn req(scope: WipScope, paths: &[&str], untracked: &[&str]) -> ChangesRequest {
    ChangesRequest { scope, paths: owned(paths), picked: !paths.is_empty(), untracked: owned(untracked), unborn: false }
}

/// A repository with tracked a.txt, `[id].txt`, `-dash.txt`, `with space.txt` and bin.dat.
fn fixture(name: &str) -> std::path::PathBuf {
    let dir = scratch_repo(name);
    for f in ["a.txt", "[id].txt", "-dash.txt", "with space.txt"] {
        write(&dir, f, "one\n");
    }
    std::fs::write(dir.join("bin.dat"), b"\x00\x01binary").unwrap();
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    dir
}

/// Saves `r` to a file, puts the repository back to HEAD with no untracked files, applies the
/// file and returns the files' bytes after.
fn round_trip(dir: &Path, r: &ChangesRequest, files: &[&str]) -> Vec<Vec<u8>> {
    let dest = dir.with_file_name(format!("{}-wip.patch", dir.file_name().unwrap().to_string_lossy()));
    let report = save(dir, &PatchSource::Changes(r.clone()), &dest).unwrap();
    assert!(matches!(report, SaveReport::Changes { files } if files > 0));
    git(dir, &["reset", "-q", "--hard"]);
    git(dir, &["clean", "-q", "-fd"]);
    git(dir, &["apply", &dest.display().to_string()]);
    let _ = std::fs::remove_file(dest);
    files.iter().map(|f| std::fs::read(dir.join(f)).unwrap_or_default()).collect()
}

#[test]
fn unstaged_round_trip_with_awkward_names() {
    let dir = fixture("wip-unstaged");
    for f in ["[id].txt", "-dash.txt", "with space.txt"] {
        write(&dir, f, "one\ntwo\n");
    }
    write(&dir, "id.txt", "a file the glob would match\n");
    git(&dir, &["add", "id.txt"]);
    std::fs::write(dir.join("bin.dat"), b"\x00\x02changed\xff").unwrap();
    write(&dir, "new file.txt", "new\n");
    let files = ["[id].txt", "-dash.txt", "with space.txt", "bin.dat", "new file.txt"];
    let want: Vec<Vec<u8>> = files.iter().map(|f| std::fs::read(dir.join(f)).unwrap()).collect();
    let r = req(WipScope::Unstaged, &["[id].txt", "-dash.txt", "with space.txt", "bin.dat"], &["new file.txt"]);
    assert_eq!(round_trip(&dir, &r, &files), want);
    // `[id].txt` was taken literally: the staged id.txt isn't in the patch.
    assert!(!dir.join("id.txt").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn staged_and_all_round_trips() {
    let dir = fixture("wip-staged");
    write(&dir, "a.txt", "one\nstaged\n");
    git(&dir, &["add", "a.txt"]);
    write(&dir, "with space.txt", "one\nunstaged\n");
    // Only the staged change.
    let staged = round_trip(&dir, &req(WipScope::Staged, &[], &[]), &["a.txt", "with space.txt"]);
    assert_eq!(staged, [b"one\nstaged\n".to_vec(), b"one\n".to_vec()]);

    write(&dir, "a.txt", "one\nstaged\n");
    git(&dir, &["add", "a.txt"]);
    write(&dir, "a.txt", "one\nstaged\nand more\n");
    write(&dir, "with space.txt", "one\nunstaged\n");
    write(&dir, "untracked.txt", "u\n");
    let all = round_trip(&dir, &req(WipScope::All, &[], &["untracked.txt"]), &["a.txt", "with space.txt", "untracked.txt"]);
    assert_eq!(all, [b"one\nstaged\nand more\n".to_vec(), b"one\nunstaged\n".to_vec(), b"u\n".to_vec()]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn nothing_changed_and_picked_untracked_only() {
    let dir = fixture("wip-nothing");
    let err = build(&dir, &PatchSource::Changes(req(WipScope::All, &[], &[]))).unwrap_err();
    assert_eq!(err.to_string(), NO_CHANGES);
    // Picking only an untracked file leaves the tracked changes out.
    write(&dir, "a.txt", "one\nchanged\n");
    write(&dir, "u.txt", "u\n");
    let r = ChangesRequest { picked: true, ..req(WipScope::All, &[], &["u.txt"]) };
    let (bytes, report) = build(&dir, &PatchSource::Changes(r)).unwrap();
    assert_eq!(report, SaveReport::Changes { files: 1 });
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.contains("b/u.txt") && !text.contains("a.txt"), "{text}");
    let many: Vec<String> = (0..=MAX_UNTRACKED).map(|i| format!("{i}.txt")).collect();
    let r = ChangesRequest { untracked: many, ..req(WipScope::All, &[], &[]) };
    let err = build(&dir, &PatchSource::Changes(r)).unwrap_err().to_string();
    assert!(err.starts_with("Too many untracked files to include (1001)"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unborn_branch() {
    let dir = scratch_repo("wip-unborn");
    write(&dir, "a.txt", "one\n");
    git(&dir, &["add", "a.txt"]);
    let staged = ChangesRequest { unborn: true, ..req(WipScope::Staged, &[], &[]) };
    let (bytes, _) = build(&dir, &PatchSource::Changes(staged)).unwrap();
    assert!(String::from_utf8(bytes).unwrap().contains("new file mode"));
    write(&dir, "a.txt", "one\ntwo\n");
    let all = ChangesRequest { unborn: true, ..req(WipScope::All, &[], &[]) };
    let (bytes, report) = build(&dir, &PatchSource::Changes(all)).unwrap();
    assert_eq!(report, SaveReport::Changes { files: 1 });
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.contains("new file mode") && text.contains("+two"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn untracked_nested_repository_is_skipped() {
    let dir = fixture("wip-nested");
    let inner = dir.join("vendor").join("inner");
    std::fs::create_dir_all(&inner).unwrap();
    git(&inner, &["init", "-q"]);
    write(&inner, "lib.txt", "inner\n");
    write(&dir, "a.txt", "one\nchanged\n");
    write(&dir, "u.txt", "u\n");
    // Status lists the nested repository as one directory row; with or without the slash it
    // is left out and the rest is saved.
    for nested in ["vendor/inner/", "vendor/inner"] {
        let r = req(WipScope::All, &[], &[nested, "u.txt"]);
        let (bytes, report) = build(&dir, &PatchSource::Changes(r)).unwrap();
        assert_eq!(report, SaveReport::Changes { files: 2 });
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.contains("b/a.txt") && text.contains("b/u.txt") && !text.contains("inner"), "{text}");
    }
    // Only the nested repository picked: nothing to save.
    let r = ChangesRequest { picked: true, ..req(WipScope::All, &[], &["vendor/inner/"]) };
    assert_eq!(build(&dir, &PatchSource::Changes(r)).unwrap_err().to_string(), NO_CHANGES);
    let _ = std::fs::remove_dir_all(&dir);
}
