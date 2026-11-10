//! The repositories a session starts from, built from its seed with the fixture helpers and
//! the fixed git dates, so the same seed is the same repositories on every run.
//!
//! `work/app` has the remote `remote/app.git`, a `main` that moved on, and two branches (`topic`
//! and `side`) that change lines of `shared.txt` that `main` changed too, so merging or
//! rebasing either conflicts. `work/lib` is a plain repository with a branch `dev`.

use super::super::fixture::{Fixture, commit_files, git};
use super::rng::Rng;
use std::path::PathBuf;

pub(super) struct Scene {
    #[expect(dead_code, reason = "never read: it keeps the folders until the scene is dropped")]
    pub fx: Fixture,
    pub app: PathBuf,
    pub lib: PathBuf,
    pub bare: PathBuf,
}

/// The repository-relative paths the outside actions may touch.
pub(super) const PATHS: [&str; 7] = ["a.txt", "shared.txt", "crlf.txt", "src/b.txt", "src/c.txt", "notes.txt", "new/d.txt"];

fn shared(edits: &[(usize, &str)]) -> String {
    (1..=10)
        .map(|n| match edits.iter().find(|(at, _)| *at == n) {
            Some((_, text)) => format!("{text} {n}\n"),
            None => format!("line {n}\n"),
        })
        .collect()
}

/// Commits `count` commits on the branch `name` (already created from the first commit): the
/// first changes `shared.txt` at `line`, the others add files of their own.
fn branch_work(app: &std::path::Path, name: &str, line: usize, count: usize) {
    git(app, &["checkout", "-q", name]);
    for n in 1..=count {
        if n == 1 {
            commit_files(app, &[("shared.txt", &shared(&[(line, name)]))], &format!("{name} 1"));
        } else {
            commit_files(app, &[(&format!("{name}-{n}.txt"), &format!("{name} {n}\n"))], &format!("{name} {n}"));
        }
    }
}

/// A scene for the session `seed`; `tag` keeps the folders of sessions running at once apart.
pub(super) fn build(tag: &str, seed: u64) -> Scene {
    let fx = Fixture::new(&format!("explore-scene-{tag}-{seed}"));
    fx.pin_dates();
    let mut rng = Rng::new(seed);

    let files = [
        ("a.txt", "alpha\n".to_owned()),
        ("shared.txt", shared(&[])),
        ("crlf.txt", "one\r\ntwo\r\nthree\r\n".to_owned()),
        ("src/b.txt", "bravo\n".to_owned()),
    ];
    let files: Vec<(&str, &str)> = files.iter().map(|(n, t)| (*n, t.as_str())).collect();
    let (bare, _seed_repo) = fx.seeded_remote("app", &files);
    let app = fx.work().join("app");
    fx.clone_cli(&bare, &app);

    git(&app, &["branch", "topic"]);
    git(&app, &["branch", "side"]);
    branch_work(&app, "topic", 5, rng.below(3) + 1);
    branch_work(&app, "side", 6, rng.below(3) + 1);
    git(&app, &["checkout", "-q", "main"]);
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::ffi::OsStrExt;
        let name = std::ffi::OsStr::from_bytes(b"caf\xe9.txt");
        std::fs::write(app.join(name), "not utf-8\n").unwrap();
        git(&app, &["add", "-A"]);
        git(&app, &["commit", "-q", "-m", "A file whose name is not UTF-8"]);
    }
    commit_files(&app, &[("shared.txt", &shared(&[(5, "main"), (6, "main")]))], "main moves on");

    let lib = fx.work().join("lib");
    fx.init(&lib, &[("lib.txt", "library\n")]);
    git(&lib, &["branch", "dev"]);

    Scene { fx, app, lib, bare }
}

#[cfg(test)]
mod tests {
    use super::super::super::fixture::{branch_oid, git_try};
    use super::*;

    #[test]
    fn same_seed_same_scene() {
        let (a, b) = (build("t1", 7), build("t2", 7));
        for branch in ["main", "topic", "side"] {
            let (x, y) = (branch_oid(&a.app, branch), branch_oid(&b.app, branch));
            assert!(x.is_some(), "{branch}");
            assert_eq!(x, y, "{branch}");
        }
    }

    #[test]
    fn different_seeds_can_differ() {
        let ids: Vec<_> = (0..8).map(|s| branch_oid(&build("t3", s).app, "topic")).collect();
        assert!(ids.iter().any(|id| *id != ids[0]), "{ids:?}");
    }

    #[test]
    fn branches_conflict() {
        let s = build("t4", 3);
        for branch in ["topic", "side"] {
            assert!(!git_try(&s.app, &["merge-tree", "--write-tree", "main", branch]), "{branch} merges cleanly");
        }
    }

    #[test]
    fn the_blob_holds_crlf_and_the_paths_exist() {
        let s = build("t5", 3);
        let repository = git2::Repository::open(&s.app).unwrap();
        let blob = repository.revparse_single("main:crlf.txt").unwrap();
        assert_eq!(blob.as_blob().unwrap().content(), b"one\r\ntwo\r\nthree\r\n");
        assert!(s.lib.join("lib.txt").exists());
        assert!(branch_oid(&s.lib, "dev").is_some());
        assert!(s.bare.exists());
        for p in ["a.txt", "shared.txt", "crlf.txt", "src/b.txt"] {
            assert!(PATHS.contains(&p) && s.app.join(p).exists(), "{p}");
        }
    }
}
