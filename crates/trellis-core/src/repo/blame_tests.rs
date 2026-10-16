use super::*;
use crate::testutil::*;
use std::path::Path;

fn head(dir: &Path) -> Oid {
    Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

fn commit(dir: &Path, file: &str, text: &str, message: &str) -> Oid {
    if let Some(parent) = Path::new(file).parent() {
        std::fs::create_dir_all(dir.join(parent)).unwrap();
    }
    write(dir, file, text);
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", message]);
    head(dir)
}

/// (commit oid, start, len) of each run.
fn runs(b: &Blame) -> Vec<(Oid, usize, usize)> {
    b.runs.iter().map(|r| (b.commits[r.commit].oid, r.start, r.len)).collect()
}

#[test]
fn blame_groups_lines_into_runs_by_commit() {
    let dir = scratch_repo("blame-runs");
    let c1 = commit(&dir, "a.txt", "1\n2\n3\n4\n5\n6\n", "add six");
    let c2 = commit(&dir, "a.txt", "1\ntwo\nthree\n4\n5\n6\n", "edit two and three");
    let c3 = commit(&dir, "a.txt", "1\ntwo\nthree\n4\n5\nsix\n", "edit six");
    let b = blame_file(&dir, "a.txt", None).unwrap();
    assert_eq!(b.at, c3);
    assert_eq!(runs(&b), [(c1, 0, 1), (c2, 1, 2), (c1, 3, 2), (c3, 5, 1)]);
    assert_eq!(b.lines, ["1", "two", "three", "4", "5", "six"]);
    let repo = Repository::open(&dir).unwrap();
    for c in &b.commits {
        let real = repo.find_commit(c.oid).unwrap();
        assert_eq!((c.author.as_str(), c.email.as_str()), ("Test", "test@example.com"));
        assert_eq!(c.summary, real.summary().unwrap());
        assert_eq!(c.time, real.author().when().seconds());
    }
    assert_eq!(b.commits.iter().map(|c| c.oid).collect::<Vec<_>>(), [c1, c2, c3], "in order of first appearance");
    for (line, want) in [(0, c1), (1, c2), (2, c2), (3, c1), (4, c1), (5, c3)] {
        assert_eq!(b.commit_of_line(line).map(|c| c.oid), Some(want), "line {line}");
    }
    assert_eq!(b.run_of(2), Some(1));
    assert_eq!(b.run_of(6), None);
    assert!(b.commit_of_line(6).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn blame_at_a_commit_sees_that_version() {
    let dir = scratch_repo("blame-at");
    commit(&dir, "a.txt", "one\ntwo\n", "first");
    let c2 = commit(&dir, "a.txt", "one\nTWO\n", "second");
    let c3 = commit(&dir, "a.txt", "one\nTWO\nthree\n", "third");
    let b = blame_file(&dir, "a.txt", Some(c2)).unwrap();
    assert_eq!(b.at, c2);
    assert_eq!(b.lines, ["one", "TWO"]);
    assert!(b.commits.iter().all(|c| c.oid != c3));
    assert_eq!(blame_file(&dir, "a.txt", None).unwrap().at, c3, "None resolves HEAD");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn blame_honours_mailmap() {
    let dir = scratch_repo("blame-mailmap");
    commit(&dir, ".mailmap", "Proper Name <proper@example.com> Test <test@example.com>\n", "mailmap");
    commit(&dir, "a.txt", "one\n", "add a");
    let b = blame_file(&dir, "a.txt", None).unwrap();
    assert_eq!((b.commits[0].author.as_str(), b.commits[0].email.as_str()), ("Proper Name", "proper@example.com"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn blame_skips_revs_in_ignore_revs_file() {
    let dir = scratch_repo("blame-ignore");
    let c1 = commit(&dir, "a.txt", "alpha\nbeta\ngamma\n", "add");
    let c2 = commit(&dir, "a.txt", "alpha\nbeta;\ngamma\n", "reformat");
    write(&dir, ".git-blame-ignore-revs", &format!("{c2}\n"));
    assert_eq!(blame_file(&dir, "a.txt", None).unwrap().commit_of_line(1).map(|c| c.oid), Some(c2));
    set_config(&dir, &[("blame.ignoreRevsFile", ".git-blame-ignore-revs")]);
    let b = blame_file(&dir, "a.txt", None).unwrap();
    assert_eq!(b.commit_of_line(1).map(|c| c.oid), Some(c1));
    assert_eq!(b.lines[1], "beta;");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_missing_ignore_revs_file_does_not_break_blame() {
    let dir = scratch_repo("blame-ignore-missing");
    let c1 = commit(&dir, "a.txt", "alpha\nbeta\ngamma\n", "add");
    let c2 = commit(&dir, "a.txt", "alpha\nbeta;\ngamma\n", "reformat");
    let repo = Repository::open(&dir).unwrap();
    assert!(ignore_revs_args(&repo, &dir).is_empty(), "nothing listed");
    set_config(&dir, &[("blame.ignoreRevsFile", "does-not-exist")]);
    let b = blame_file(&dir, "a.txt", None).unwrap();
    assert_eq!(b.commit_of_line(1).map(|c| c.oid), Some(c2));

    // A missing file next to one that exists: the existing one still applies.
    write(&dir, ".git-blame-ignore-revs", &format!("{c2}\n"));
    git(&dir, &["config", "--add", "blame.ignoreRevsFile", ".git-blame-ignore-revs"]);
    let args = ignore_revs_args(&Repository::open(&dir).unwrap(), &dir);
    assert_eq!(args.first().map(String::as_str), Some("--no-ignore-revs-file"), "{args:?}");
    assert!(args.iter().any(|a| a == "--ignore-revs-file=.git-blame-ignore-revs"), "{args:?}");
    assert!(!args.iter().any(|a| a.contains("does-not-exist")), "{args:?}");
    let b = blame_file(&dir, "a.txt", None).unwrap();
    assert_eq!(b.commit_of_line(1).map(|c| c.oid), Some(c1));

    // All present: git reads its config itself.
    git(&dir, &["config", "--unset-all", "blame.ignoreRevsFile", "does-not-exist"]);
    assert!(ignore_revs_args(&Repository::open(&dir).unwrap(), &dir).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn blame_of_binary_folder_submodule_missing() {
    let dir = scratch_repo("blame-refusals");
    std::fs::create_dir_all(dir.join("dir")).unwrap();
    std::fs::write(dir.join("dir/data.bin"), b"\x00\x01\x02binary\x00").unwrap();
    commit(&dir, "a.txt", "one\n", "add");
    let err = |path: &str| blame_file(&dir, path, None).unwrap_err();
    assert!(matches!(err("dir/data.bin"), Error::Unsupported(m) if m.contains("binary")));
    assert!(matches!(err("dir"), Error::Unsupported(m) if m == "dir is a folder"));
    let missing = err("nope.txt");
    assert!(missing.is_not_found(), "{missing}");
    assert!(missing.to_string().contains("nope.txt is not in commit"), "{missing}");

    let empty = scratch_repo("blame-unborn");
    assert!(matches!(blame_file(&empty, "a.txt", None), Err(Error::Other(m)) if m.contains("no commits")));

    let (parent, _sub) = with_submodule("blame-submodule");
    assert!(matches!(blame_file(&parent, "libs/lib", None), Err(Error::Unsupported(m)) if m.contains("submodule")));
    for d in [&dir, &empty, &parent] {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn blame_of_a_too_large_file() {
    let dir = scratch_repo("blame-too-large");
    // Text just past the limit: refused before git runs, with the size rounded up to whole MB.
    let big = "x\n".repeat(BLAME_MAX_BYTES / 2 + 1);
    assert!(big.len() > BLAME_MAX_BYTES);
    commit(&dir, "big.txt", &big, "big");
    let err = blame_file(&dir, "big.txt", None).unwrap_err();
    assert!(matches!(&err, Error::Unsupported(m) if m == "big.txt is too large to blame (9 MB)"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn blame_keeps_tabs_and_strips_cr() {
    let dir = scratch_repo("blame-crlf");
    commit(&dir, "a.txt", "fn main() {\r\n\tlet x = 1;\r\n}\r\n", "crlf");
    let b = blame_file(&dir, "a.txt", None).unwrap();
    assert_eq!(b.lines, ["fn main() {", "\tlet x = 1;", "}"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn blame_of_a_literal_path() {
    let dir = scratch_repo("blame-literal");
    let c = commit(&dir, "dir/[id] a.txt", "x\ny\n", "odd name");
    commit(&dir, "dir/i a.txt", "other\n", "a name the pattern would match");
    let b = blame_file(&dir, "dir/[id] a.txt", None).unwrap();
    assert_eq!(b.lines, ["x", "y"]);
    assert_eq!(runs(&b), [(c, 0, 2)]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn blame_of_an_empty_file() {
    let dir = scratch_repo("blame-empty");
    commit(&dir, "empty.txt", "", "empty");
    let b = blame_file(&dir, "empty.txt", None).unwrap();
    assert!(b.lines.is_empty() && b.runs.is_empty() && b.commits.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}
