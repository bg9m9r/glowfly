//! The "N commits behind" alert after a background fetch: what to remember when the fetch ends
//! and what to say once the reload after it shows the branch further behind (update/notify.rs).

use super::branches::commits;
use std::path::PathBuf;
use trellis_core::repo::{BranchInfo, Snapshot};

/// How far behind its upstream the checked-out branch was when an auto-fetch ended.
#[derive(Clone, Debug, PartialEq)]
pub struct BehindWatch {
    pub workdir: PathBuf,
    pub branch: String,
    pub upstream: String,
    pub behind: usize,
    /// st.load_gen when the fetch ended: only a load started after it shows the fetch's result.
    pub after_gen: u64,
}

/// The checked-out branch's position before the reload, or None when HEAD is detached or the
/// branch has no upstream ref.
pub fn baseline(snap: &Snapshot, after_gen: u64) -> Option<BehindWatch> {
    let branch = snap.head_branch.as_ref()?;
    let head = snap.head_info().filter(|h| &h.name == branch)?;
    Some(BehindWatch {
        workdir: snap.workdir.clone(),
        branch: head.name.clone(),
        upstream: head.upstream.clone()?,
        behind: head.behind,
        after_gen,
    })
}

/// The alert for the reloaded checked-out branch `head`, when it is the watched branch with the
/// same upstream and has fallen further behind it.
pub fn alert_text(w: &BehindWatch, head: Option<&BranchInfo>) -> Option<String> {
    let head = head?;
    if head.name != w.branch || head.upstream.as_deref() != Some(w.upstream.as_str()) || head.behind <= w.behind {
        return None;
    }
    let mut text = format!("{} is {} behind {}", head.name, commits(head.behind), w.upstream);
    if head.ahead > 0 {
        text.push_str(&format!(" and {} ahead", commits(head.ahead)));
    }
    Some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn watch(behind: usize) -> BehindWatch {
        BehindWatch { workdir: "w".into(), branch: "main".into(), upstream: "origin/main".into(), behind, after_gen: 1 }
    }

    fn head(name: &str, upstream: &str, ahead: usize, behind: usize) -> BranchInfo {
        BranchInfo {
            name: name.into(),
            oid: None,
            is_head: true,
            upstream: Some(upstream.into()),
            ahead,
            behind,
            tracking: None,
            upstream_ref: Some(format!("refs/remotes/{upstream}")),
            merged: true,
        }
    }

    #[test]
    fn alert_names_the_branch_and_counts() {
        let h = head("main", "origin/main", 0, 1);
        assert_eq!(alert_text(&watch(0), Some(&h)).as_deref(), Some("main is 1 commit behind origin/main"));
        let h = head("main", "origin/main", 1, 3);
        assert_eq!(alert_text(&watch(1), Some(&h)).as_deref(), Some("main is 3 commits behind origin/main and 1 commit ahead"));
        let h = head("main", "origin/main", 3, 2);
        assert_eq!(alert_text(&watch(0), Some(&h)).as_deref(), Some("main is 2 commits behind origin/main and 3 commits ahead"));
    }

    #[test]
    fn no_alert_unless_further_behind_on_the_same_branch() {
        assert_eq!(alert_text(&watch(2), Some(&head("main", "origin/main", 0, 2))), None, "unchanged");
        assert_eq!(alert_text(&watch(3), Some(&head("main", "origin/main", 0, 1))), None, "lower");
        assert_eq!(alert_text(&watch(0), Some(&head("main", "origin/main", 0, 0))), None, "up to date");
        assert_eq!(alert_text(&watch(0), Some(&head("dev", "origin/main", 0, 1))), None, "another branch");
        assert_eq!(alert_text(&watch(0), Some(&head("main", "upstream/main", 0, 1))), None, "another upstream");
        assert_eq!(alert_text(&watch(0), None), None, "no head branch");
    }

    #[test]
    fn no_baseline_when_detached_or_without_upstream() {
        let dir = trellis_core::paths::temp_dir().join(format!("trellis-notify-baseline-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let repo = git2::Repository::init(&dir).unwrap();
        std::fs::write(dir.join("a.txt"), "a").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("a.txt")).unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let sig = git2::Signature::now("T", "t@example.com").unwrap();
        let oid = repo.commit(Some("HEAD"), &sig, &sig, "one", &tree, &[]).unwrap();
        let opts = trellis_core::repo::LoadOptions::all(100);
        let snap = trellis_core::repo::load(&dir, &opts).unwrap();
        assert!(snap.head_branch.is_some());
        assert_eq!(baseline(&snap, 0), None, "no upstream");
        repo.set_head_detached(oid).unwrap();
        let snap = trellis_core::repo::load(&dir, &opts).unwrap();
        assert_eq!(snap.head_branch, None);
        assert_eq!(baseline(&snap, 0), None, "detached");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
