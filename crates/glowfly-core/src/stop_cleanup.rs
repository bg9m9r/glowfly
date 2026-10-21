//! What a stopped fetch leaves behind. git removes its lock files when it gets SIGTERM, but a
//! hard kill (SIGKILL, or terminating its job on Windows) skips that, and a leftover lock makes
//! the next command fail with "Unable to create '….lock': File exists". Before the fetch runs,
//! [`scan`] notes which candidate files exist; after a stop, [`clean`] removes the ones that
//! appeared since: the locks a fetch takes in the common git dir (`shallow.lock`,
//! `packed-refs.lock`, `FETCH_HEAD.lock`, `config.lock`, `refs/**/*.lock`) and its temporary
//! object files (`objects/pack/tmp_*`, `objects/??/tmp_obj_*`, and a `pack-*.keep` that
//! fetch-pack wrote). `index.lock` and work-tree files are never touched. Refs and `shallow`
//! themselves are left as git left them: old or updated, one ref at a time.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// The most directory entries one scan reads.
const MAX_ENTRIES: usize = 100_000;
/// How much earlier than the scan a new file's time may be (coarse file-system clocks).
const SLACK: Duration = Duration::from_secs(2);
/// The locks a fetch may take at the top of the common git dir.
const TOP_LOCKS: [&str; 4] = ["shallow.lock", "packed-refs.lock", "FETCH_HEAD.lock", "config.lock"];

/// The candidates present before a command started.
pub struct Before {
    started: SystemTime,
    common: PathBuf,
    existing: HashSet<PathBuf>,
}

/// Notes the candidates in the repository at `workdir` (its common git dir, read with libgit2).
/// None when it isn't a repository.
pub fn scan(workdir: &Path) -> Option<Before> {
    let repo = git2::Repository::open(workdir).ok()?;
    let common = repo.commondir().to_path_buf();
    let started = SystemTime::now();
    Some(Before { started, existing: candidates(&common).into_iter().collect(), common })
}

/// Removes the candidates that weren't there at `b`'s scan and aren't older than it, and returns
/// them.
pub fn clean(b: &Before) -> Vec<PathBuf> {
    let floor = b.started.checked_sub(SLACK).unwrap_or(b.started);
    let new = |p: &PathBuf| !b.existing.contains(p) && std::fs::symlink_metadata(p).and_then(|m| m.modified()).is_ok_and(|t| t >= floor);
    candidates(&b.common).into_iter().filter(new).filter(|p| crate::persist::remove_file(p).is_ok()).collect()
}

/// The lock and temporary files in `common` that a stopped fetch may leave.
fn candidates(common: &Path) -> Vec<PathBuf> {
    let mut budget = MAX_ENTRIES;
    let mut out: Vec<PathBuf> = TOP_LOCKS.iter().map(|n| common.join(n)).filter(|p| is_file(p)).collect();
    walk_ref_locks(&common.join("refs"), &mut budget, &mut out);
    let pack = common.join("objects").join("pack");
    for (path, name) in files(&pack, &mut budget) {
        if name.starts_with("tmp_") || (name.ends_with(".keep") && written_by_fetch(&path)) {
            out.push(path);
        }
    }
    for (dir, name) in dirs(&common.join("objects"), &mut budget) {
        if name.len() == 2 && name.bytes().all(|b| b.is_ascii_hexdigit()) {
            out.extend(files(&dir, &mut budget).into_iter().filter(|(_, n)| n.starts_with("tmp_obj_")).map(|(p, _)| p));
        }
    }
    out
}

fn is_file(p: &Path) -> bool {
    std::fs::symlink_metadata(p).is_ok_and(|m| m.is_file())
}

/// A pack's `.keep` that fetch-pack wrote while it updates the refs.
fn written_by_fetch(path: &Path) -> bool {
    use std::io::Read;
    let mut head = [0u8; 10];
    std::fs::File::open(path).and_then(|mut f| f.read_exact(&mut head)).is_ok() && head == *b"fetch-pack"
}

/// The entries of `dir` of one kind (files, or folders), never following links.
fn entries(dir: &Path, budget: &mut usize, want_dir: bool) -> Vec<(PathBuf, String)> {
    let Ok(read) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out = Vec::new();
    for e in read.flatten() {
        if *budget == 0 {
            break;
        }
        *budget -= 1;
        let Ok(kind) = e.file_type() else { continue };
        if (want_dir && kind.is_dir()) || (!want_dir && kind.is_file()) {
            out.push((e.path(), e.file_name().to_string_lossy().into_owned()));
        }
    }
    out
}

fn files(dir: &Path, budget: &mut usize) -> Vec<(PathBuf, String)> {
    entries(dir, budget, false)
}

fn dirs(dir: &Path, budget: &mut usize) -> Vec<(PathBuf, String)> {
    entries(dir, budget, true)
}

/// Every `*.lock` file under `dir`, depth first.
fn walk_ref_locks(dir: &Path, budget: &mut usize, out: &mut Vec<PathBuf>) {
    for (path, name) in files(dir, budget) {
        if name.ends_with(".lock") {
            out.push(path);
        }
    }
    for (sub, _) in dirs(dir, budget) {
        walk_ref_locks(&sub, budget, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{git, scratch_repo, write};

    #[test]
    fn removes_only_the_files_made_after_the_scan() {
        let dir = scratch_repo("stop-cleanup");
        write(&dir, "a.txt", "one\n");
        git(&dir, &["add", "."]);
        git(&dir, &["commit", "-q", "-m", "first"]);
        let gd = dir.join(".git");
        std::fs::write(gd.join("packed-refs.lock"), "").unwrap();
        let b = scan(&dir).expect("a repository");
        std::fs::create_dir_all(gd.join("refs/remotes/origin")).unwrap();
        std::fs::create_dir_all(gd.join("objects/pack")).unwrap();
        let made = [
            gd.join("shallow.lock"),
            gd.join("refs/remotes/origin/main.lock"),
            gd.join("objects/pack/tmp_pack_x"),
            gd.join("objects/pack/pack-1234.keep"),
        ];
        for p in &made[..3] {
            std::fs::write(p, "").unwrap();
        }
        std::fs::write(&made[3], "fetch-pack 123 on host\n").unwrap();
        let kept =
            [gd.join("packed-refs.lock"), gd.join("index.lock"), gd.join("refs/remotes/origin/main"), gd.join("objects/pack/pack-9.keep")];
        std::fs::write(&kept[1], "").unwrap();
        std::fs::write(&kept[2], "0000000000000000000000000000000000000000\n").unwrap();
        std::fs::write(&kept[3], "kept by hand\n").unwrap();
        let names = |ps: &[PathBuf]| {
            let mut n: Vec<String> = ps.iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
            n.sort();
            n
        };
        assert_eq!(names(&clean(&b)), names(&made));
        for p in &made {
            assert!(!p.exists(), "{} removed", p.display());
        }
        for p in &kept {
            assert!(p.exists(), "{} kept", p.display());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
