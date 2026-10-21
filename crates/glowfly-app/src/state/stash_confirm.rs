//! Whether applying, popping or deleting a stash asks first: Delete always does, Apply and Pop
//! only on top of uncommitted changes (they may conflict or be refused). The one place that
//! decides it, for the STASHES menu, the palette and the toolbar's Pop. Pure.

use super::palette_kinds::StashAction;
use super::stashes::drop_confirm_body;
use crate::msg::{JobMsg, Msg};
use glowfly_core::repo::{Snapshot, StashInfo};

/// `run`, or the confirmation that sends it once accepted, for `action` on the stash named
/// `message`.
pub fn guarded(snap: &Snapshot, action: StashAction, message: &str, run: Msg) -> Msg {
    match action {
        StashAction::Drop => Msg::confirm_danger("Delete stash", drop_confirm_body(message), "Delete", run),
        StashAction::Apply | StashAction::Pop if snap.is_dirty() => {
            let button = if action == StashAction::Pop { "Pop" } else { "Apply" };
            Msg::confirm(action.job_label(), dirty_body(action, message), button, run)
        }
        StashAction::Apply | StashAction::Pop => run,
    }
}

/// The question before applying or popping the stash named `message` on a dirty tree.
pub fn dirty_body(action: StashAction, message: &str) -> String {
    let mut body = format!(
        "Apply \"{message}\" on top of your uncommitted changes? If they change the same files, git may stop with conflicts or refuse to apply."
    );
    if action == StashAction::Pop {
        body.push_str(" If it conflicts, the stash is kept.");
    }
    body
}

/// `action` on stash `s`, named by its position and commit, asking first when it should.
pub fn stash_msg(snap: &Snapshot, action: StashAction, s: &StashInfo) -> Msg {
    guarded(snap, action, &s.message, Msg::Job(JobMsg::Stash { action, index: s.index, oid: s.oid }))
}

/// `git stash pop` of the newest stash, asking first on a dirty tree; None without stashes.
pub fn pop_latest(snap: &Snapshot) -> Option<Msg> {
    let top = snap.stashes.first()?;
    Some(guarded(snap, StashAction::Pop, &top.message, Msg::git("Pop stash", &["stash", "pop"])))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> (std::path::PathBuf, Snapshot) {
        let dir = glowfly_core::paths::temp_dir().join(format!("trellis-stash-confirm-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let repo = git2::Repository::init(&dir).unwrap();
        std::fs::write(dir.join("a.txt"), "a").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("a.txt")).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let sig = git2::Signature::now("T", "t@example.com").unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "one", &tree, &[]).unwrap();
        let snap = glowfly_core::repo::load(&dir, &glowfly_core::repo::LoadOptions::all(100)).unwrap();
        (dir, snap)
    }

    fn run() -> Msg {
        Msg::git("Pop stash", &["stash", "pop"])
    }

    #[test]
    fn asks_only_on_a_dirty_tree() {
        let (dir, clean) = fixture("guard");
        assert!(matches!(guarded(&clean, StashAction::Apply, "wip", run()), Msg::Job(JobMsg::Git { .. })));
        assert!(matches!(guarded(&clean, StashAction::Pop, "wip", run()), Msg::Job(JobMsg::Git { .. })));
        assert!(
            matches!(guarded(&clean, StashAction::Drop, "wip", run()), Msg::ConfirmDanger { ref title, .. } if title == "Delete stash")
        );
        std::fs::write(dir.join("a.txt"), "b").unwrap();
        let dirty = glowfly_core::repo::load(&dir, &glowfly_core::repo::LoadOptions::all(100)).unwrap();
        assert!(dirty.is_dirty());
        let m = guarded(&dirty, StashAction::Pop, "wip", run());
        assert!(matches!(&m, Msg::Confirm { title, button, then, .. }
            if title == "Pop stash" && button == "Pop" && matches!(**then, Msg::Job(JobMsg::Git { .. }))));
        let m = guarded(&dirty, StashAction::Apply, "wip", run());
        assert!(matches!(&m, Msg::Confirm { title, button, .. } if title == "Apply stash" && button == "Apply"));
        assert!(pop_latest(&dirty).is_none(), "no stashes");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dirty_body_wording() {
        let apply = dirty_body(StashAction::Apply, "wip on main");
        assert_eq!(
            apply,
            "Apply \"wip on main\" on top of your uncommitted changes? If they change the same files, git may stop with conflicts or refuse to apply."
        );
        assert_eq!(dirty_body(StashAction::Pop, "wip on main"), format!("{apply} If it conflicts, the stash is kept."));
    }
}
