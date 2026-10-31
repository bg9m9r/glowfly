//! The check a job can carry (`Effect::RunGit { expect }`): a revision named by position, such
//! as `stash@{2}`, must still be the commit the user picked, or nothing runs.

use glowfly_core::gitcli::{self, args};
use std::path::Path;

/// Ok when `rev` resolves to `oid` in `cwd`; otherwise the message the job fails with.
pub(super) fn check(cwd: &Path, rev: &str, oid: git2::Oid) -> Result<(), String> {
    let out = gitcli::read(cwd, &args(&["rev-parse", "--verify", "--quiet", &format!("{rev}^{{commit}}")]));
    let now = out.stdout.trim();
    if out.ok && now == oid.to_string() {
        return Ok(());
    }
    let what = if rev.starts_with("stash@") { "stash list" } else { "repository" };
    let short = &oid.to_string()[..7];
    let found = if out.ok && !now.is_empty() { format!("{rev} is now {}", &now[..now.len().min(7)]) } else { format!("{rev} is gone") };
    Err(format!("The {what} changed since it was shown ({found}, not {short}), so nothing was done. Refresh and pick it again."))
}

#[cfg(test)]
mod tests {
    use super::*;
    use glowfly_core::gitcli::run;

    #[test]
    fn checks_the_revision_still_names_the_commit() {
        let dir = glowfly_core::paths::temp_dir().join(format!("glowfly-app-test-expect-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let git = |a: &[&str]| {
            let (ok, out) = run(&dir, &args(a));
            assert!(ok, "{a:?}: {out}");
            out
        };
        git(&["init", "-q", "-b", "main"]);
        git(&["-c", "user.name=T", "-c", "user.email=t@e", "commit", "-q", "--allow-empty", "-m", "one"]);
        let head = git2::Oid::from_str(git(&["rev-parse", "HEAD"]).trim()).unwrap();
        assert_eq!(check(&dir, "HEAD", head), Ok(()));
        git(&["-c", "user.name=T", "-c", "user.email=t@e", "commit", "-q", "--allow-empty", "-m", "two"]);
        let e = check(&dir, "HEAD", head).unwrap_err();
        assert!(e.contains("HEAD is now") && e.contains("nothing was done"), "{e}");
        let e = check(&dir, "stash@{0}", head).unwrap_err();
        assert!(e.contains("stash list changed") && e.contains("stash@{0} is gone"), "{e}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
