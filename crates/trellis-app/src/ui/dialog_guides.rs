//! The side panes of the dialogs for less familiar operations (`dialog_frame::Guide`): each
//! explains its operation in a few steps, from git's own documentation, under "How it works".
//! The keys are written out in full so the key check (`tests/i18n_keys.rs`) sees them.

use super::dialog_frame::Guide;
use trellis_core::i18n::Lang;

/// Add submodule.
pub fn submodule_add(t: Lang) -> Guide {
    Guide {
        steps: vec![
            t.tr("dialog.guide.submodule_add.step1"),
            t.tr("dialog.guide.submodule_add.step2"),
            t.tr("dialog.guide.submodule_add.step3"),
            t.tr("dialog.guide.submodule_add.step4"),
        ],
    }
}

/// Edit submodule.
pub fn submodule_edit(t: Lang) -> Guide {
    Guide {
        steps: vec![
            t.tr("dialog.guide.submodule_edit.step1"),
            t.tr("dialog.guide.submodule_edit.step2"),
            t.tr("dialog.guide.submodule_edit.step3"),
            t.tr("dialog.guide.submodule_edit.step4"),
        ],
    }
}

/// Remove submodule.
pub fn submodule_remove(t: Lang) -> Guide {
    Guide {
        steps: vec![
            t.tr("dialog.guide.submodule_remove.step1"),
            t.tr("dialog.guide.submodule_remove.step2"),
            t.tr("dialog.guide.submodule_remove.step3"),
            t.tr("dialog.guide.submodule_remove.step4"),
        ],
    }
}

/// Create worktree.
pub fn worktree_add(t: Lang) -> Guide {
    Guide {
        steps: vec![
            t.tr("dialog.guide.worktree_add.step1"),
            t.tr("dialog.guide.worktree_add.step2"),
            t.tr("dialog.guide.worktree_add.step3"),
            t.tr("dialog.guide.worktree_add.step4"),
        ],
    }
}

/// Remove worktree.
pub fn worktree_remove(t: Lang) -> Guide {
    Guide {
        steps: vec![
            t.tr("dialog.guide.worktree_remove.step1"),
            t.tr("dialog.guide.worktree_remove.step2"),
            t.tr("dialog.guide.worktree_remove.step3"),
        ],
    }
}

/// Lock worktree.
pub fn worktree_lock(t: Lang) -> Guide {
    Guide {
        steps: vec![
            t.tr("dialog.guide.worktree_lock.step1"),
            t.tr("dialog.guide.worktree_lock.step2"),
            t.tr("dialog.guide.worktree_lock.step3"),
        ],
    }
}

/// Move worktree.
pub fn worktree_move(t: Lang) -> Guide {
    Guide {
        steps: vec![
            t.tr("dialog.guide.worktree_move.step1"),
            t.tr("dialog.guide.worktree_move.step2"),
            t.tr("dialog.guide.worktree_move.step3"),
        ],
    }
}

/// Git Flow Start.
pub fn gitflow_start(t: Lang) -> Guide {
    Guide {
        steps: vec![
            t.tr("dialog.guide.gitflow_start.step1"),
            t.tr("dialog.guide.gitflow_start.step2"),
            t.tr("dialog.guide.gitflow_start.step3"),
            t.tr("dialog.guide.gitflow_start.step4"),
        ],
    }
}

/// Git Flow Finish.
pub fn gitflow_finish(t: Lang) -> Guide {
    Guide {
        steps: vec![
            t.tr("dialog.guide.gitflow_finish.step1"),
            t.tr("dialog.guide.gitflow_finish.step2"),
            t.tr("dialog.guide.gitflow_finish.step3"),
            t.tr("dialog.guide.gitflow_finish.step4"),
        ],
    }
}

/// Apply patch.
pub fn patch_apply(t: Lang) -> Guide {
    Guide {
        steps: vec![
            t.tr("dialog.guide.patch_apply.step1"),
            t.tr("dialog.guide.patch_apply.step2"),
            t.tr("dialog.guide.patch_apply.step3"),
            t.tr("dialog.guide.patch_apply.step4"),
        ],
    }
}

/// Create patch.
pub fn patch_save(t: Lang) -> Guide {
    Guide {
        steps: vec![
            t.tr("dialog.guide.patch_save.step1"),
            t.tr("dialog.guide.patch_save.step2"),
            t.tr("dialog.guide.patch_save.step3"),
            t.tr("dialog.guide.patch_save.step4"),
        ],
    }
}

/// Choose the mainline parent.
pub fn mainline(t: Lang) -> Guide {
    Guide {
        steps: vec![
            t.tr("dialog.guide.mainline.step1"),
            t.tr("dialog.guide.mainline.step2"),
            t.tr("dialog.guide.mainline.step3"),
        ],
    }
}
