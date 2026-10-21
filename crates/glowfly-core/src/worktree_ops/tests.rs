use super::*;
use git2::Oid;

fn oid() -> Oid {
    Oid::from_str("abc1234def5678901234567890123456789abcde").unwrap()
}

/// An absolute path on this platform.
fn abs(rest: &str) -> PathBuf {
    if cfg!(windows) { PathBuf::from(format!("C:/{rest}")) } else { PathBuf::from(format!("/{rest}")) }
}

#[test]
fn add_cmd_forms() {
    let dir = abs("work/app-worktrees/side");
    let d = dir.display().to_string();
    let branch = WorktreeBase::Branch("side".into());
    let commit = WorktreeBase::Commit(oid());
    let sha = oid().to_string();
    assert_eq!(add_cmd(&dir, &branch, None), ["worktree", "add", &d, "side"]);
    assert_eq!(add_cmd(&dir, &branch, Some("topic")), ["worktree", "add", "-b", "topic", &d, "side"]);
    assert_eq!(add_cmd(&dir, &commit, None), ["worktree", "add", "--detach", &d, &sha]);
    assert_eq!(add_cmd(&dir, &commit, Some("topic")), ["worktree", "add", "-b", "topic", &d, &sha]);
    let remote = WorktreeBase::Remote { remote: "origin".into(), branch: "feature".into() };
    let tracked = "refs/remotes/origin/feature";
    assert_eq!(add_cmd(&dir, &remote, None), ["worktree", "add", "--track", "-b", "feature", &d, tracked]);
    assert_eq!(add_cmd(&dir, &remote, Some("mine")), ["worktree", "add", "--track", "-b", "mine", &d, tracked]);
}

#[test]
fn remote_base_creates_a_tracking_branch() {
    use crate::testutil::{git, with_origin};
    let (work, bare) = with_origin("wt-add-remote");
    git(&work, &["branch", "feature"]);
    git(&work, &["push", "-q", "origin", "feature"]);
    git(&work, &["branch", "-D", "feature"]);
    let folder = work.with_file_name(format!("{}-feature", work.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&folder);
    let base = WorktreeBase::Remote { remote: "origin".into(), branch: "feature".into() };
    let (ok, out) = crate::gitcli::run(&work, &add_cmd(&folder, &base, None));
    assert!(ok, "{out}");
    let listed = crate::repo::worktrees_at(&work).unwrap();
    let wt = listed.iter().find(|w| crate::paths::same_folder(&w.path, &folder)).expect("the new worktree");
    assert_eq!(wt.branch.as_deref(), Some("feature"));
    let repo = git2::Repository::open(&work).unwrap();
    let config = repo.config().unwrap().snapshot().unwrap();
    assert_eq!(config.get_str("branch.feature.remote").unwrap(), "origin");
    assert_eq!(config.get_str("branch.feature.merge").unwrap(), "refs/heads/feature");
    for d in [&folder, &bare, &work] {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn base_labels() {
    assert_eq!(WorktreeBase::Branch("feature/x".into()).label(), "feature/x");
    assert_eq!(WorktreeBase::Commit(oid()).label(), "abc1234");
    assert_eq!(WorktreeBase::Remote { remote: "origin".into(), branch: "feature".into() }.label(), "origin/feature");
}

#[test]
fn slugs() {
    assert_eq!(folder_slug("feature/x"), "feature-x");
    assert_eq!(folder_slug("a b:c"), "a-b-c");
    assert_eq!(folder_slug("/.x/"), "x");
    assert_eq!(folder_slug("..-y-."), "y");
    assert_eq!(folder_slug("a\\b*c?d\"e<f>g|h\ti"), "a-b-c-d-e-f-g-h-i");
    assert_eq!(folder_slug("aux"), "aux-wt");
    assert_eq!(folder_slug("Con.d"), "Con-wt.d");
    assert_eq!(folder_slug("bell\u{7}"), "bell");
    assert_eq!(folder_slug("COM\u{b9}"), "COM\u{b9}-wt");
    for name in ["aux", "nul.x", "feature/lpt9", "x. ", "fix/conin$"] {
        assert_eq!(crate::names::win_name_problem(&folder_slug(name)), None, "{name:?}");
    }
}

#[test]
fn parse_folder_rules() {
    assert_eq!(parse_folder(""), Err(FolderProblem::Empty));
    assert_eq!(parse_folder("   "), Err(FolderProblem::Empty));
    assert_eq!(parse_folder("rel/dir"), Err(FolderProblem::NotAbsolute));
    let tmp = crate::paths::temp_dir().join("glowfly-wt-parse");
    assert_eq!(parse_folder(&format!("  {}  ", tmp.display())), Ok(tmp));
}

#[test]
fn check_folder_on_disk() {
    let root = crate::paths::temp_dir().join(format!("glowfly-wt-check-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    assert_eq!(check_folder(&root.join("missing")), Ok(()));
    let empty = root.join("empty");
    std::fs::create_dir(&empty).unwrap();
    assert_eq!(check_folder(&empty), Ok(()));
    let full = root.join("full");
    std::fs::create_dir(&full).unwrap();
    std::fs::write(full.join("a.txt"), "x").unwrap();
    assert_eq!(check_folder(&full), Err(FolderProblem::NotEmpty));
    assert_eq!(check_folder(&full.join("a.txt")), Err(FolderProblem::IsFile));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn messages() {
    assert_eq!(FolderProblem::Empty.message(""), "Choose a folder for the new worktree.");
    assert_eq!(FolderProblem::NotAbsolute.message("x"), "Enter the full path of the folder.");
    assert!(FolderProblem::NotEmpty.message("C:/x").contains("C:/x"));
    assert!(FolderProblem::IsFile.message("C:/x").starts_with("C:/x is a file"));
}

#[test]
fn inside_and_same() {
    use crate::paths::{is_within, same_spelling};
    let app = abs("a/app");
    assert!(is_within(&app, &app));
    assert!(same_spelling(&app, &app));
    assert!(is_within(&abs("a/app/sub/dir"), &app));
    assert!(!is_within(&abs("a/app-wt"), &app), "a sibling with a shared prefix");
    assert!(!is_within(&abs("a"), &app));
    assert!(!same_spelling(&abs("a/app/sub"), &app));
    assert!(same_spelling(&abs("a/app/"), &app), "a trailing separator");
    if cfg!(windows) {
        assert!(same_spelling(Path::new(r"c:\a\app"), Path::new("C:/a/app")));
        // Case is the volume's business: the spelling never folds it.
        assert!(!same_spelling(Path::new(r"c:\A\App"), Path::new("C:/a/app")));
        assert!(!is_within(Path::new(r"C:\a\APP\x"), Path::new("c:/a/app")));
    }
}
