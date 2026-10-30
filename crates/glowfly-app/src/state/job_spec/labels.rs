//! A job's label in the UI language, for the headlines built from it ("<label> failed: …",
//! "<label> succeeded", "<label> cancelled", Try again's refusals). A job's own label stays
//! English: it names the job to the code (routine runs, retries, Git Flow's pending finish, the
//! activity file), so it is translated only where it is shown. Labels with a name in them are
//! matched by their fixed start; one this file doesn't know is shown as it is. Pure.

use glowfly_core::i18n::Lang;
use std::borrow::Cow;

/// `label` in `t`'s language.
pub fn shown(t: Lang, label: &str) -> Cow<'_, str> {
    if let Some(text) = fixed(t, label) {
        return Cow::Borrowed(text);
    }
    named(t, label).map_or(Cow::Borrowed(label), Cow::Owned)
}

/// The labels that never change.
fn fixed(t: Lang, label: &str) -> Option<&'static str> {
    Some(match label {
        "Abort" => t.tr("job.label.abort"),
        "Abort rebase" => t.tr("job.label.abort_rebase"),
        "Amend" => t.tr("job.label.amend"),
        "Apply patch" => t.tr("job.label.apply_patch"),
        "Apply patches" => t.tr("job.label.apply_patches"),
        "Checkout" => t.tr("job.label.checkout"),
        "Cherry-pick" => t.tr("job.label.cherry_pick"),
        "Clone" => t.tr("job.label.clone"),
        "Commit" => t.tr("job.label.commit"),
        "Continue" => t.tr("job.label.continue"),
        "Create branch" => t.tr("job.label.create_branch"),
        "Create worktree" => t.tr("job.label.create_worktree"),
        "Delete branch" => t.tr("job.label.delete_branch"),
        "Delete local and remote branch" => t.tr("job.label.delete_local_and_remote_branch"),
        "Delete remote branch" => t.tr("job.label.delete_remote_branch"),
        "Delete tag" => t.tr("job.label.delete_tag"),
        "Discard all changes" => t.tr("job.label.discard_all"),
        "Download file contents" => t.tr("job.label.download_contents"),
        "Drop commits" => t.tr("job.label.drop_commits"),
        "Fast-forward" => t.tr("job.label.fast_forward"),
        "Fetch" => t.tr("job.label.fetch"),
        "Fetch all" => t.tr("job.label.fetch_all"),
        "Fetch and push" => t.tr("job.label.fetch_and_push"),
        "Force push" => t.tr("job.label.force_push"),
        "Initialize Git Flow" => t.tr("job.label.init_git_flow"),
        "Initialize Git LFS" => t.tr("job.label.init_git_lfs"),
        "Initialize LFS" => t.tr("job.label.init_lfs"),
        "Initialize submodules" => t.tr("job.label.init_submodules"),
        "Interactive rebase" => t.tr("job.label.interactive_rebase"),
        "Lock worktree" => t.tr("job.label.lock_worktree"),
        "Merge" => t.tr("job.label.merge"),
        "Move worktree" => t.tr("job.label.move_worktree"),
        "Prune worktrees" => t.tr("job.label.prune_worktrees"),
        "Publish to GitHub" => t.tr("job.label.publish_github"),
        "Pull" => t.tr("job.label.pull"),
        "Push" => t.tr("job.label.push"),
        "Push to all remotes" => t.tr("job.label.push_all"),
        "Put back stashed changes" => t.tr("job.label.put_back"),
        "Rebase" => t.tr("job.label.rebase"),
        "Remove worktree" => t.tr("job.label.remove_worktree"),
        "Rename branch" => t.tr("job.label.rename_branch"),
        "Rename local and remote branch" => t.tr("job.label.rename_local_and_remote_branch"),
        "Rename stash" => t.tr("job.label.rename_stash"),
        "Reset (hard)" => t.tr("job.label.reset_hard"),
        "Reset (mixed)" => t.tr("job.label.reset_mixed"),
        "Reset (soft)" => t.tr("job.label.reset_soft"),
        "Revert" => t.tr("job.label.revert"),
        "Reword commit" => t.tr("job.label.reword"),
        "Save Git Flow settings" => t.tr("job.label.save_git_flow"),
        "Set upstream" => t.tr("job.label.set_upstream"),
        "Squash commits" => t.tr("job.label.squash"),
        "Stage all and commit" => t.tr("job.label.stage_all_and_commit"),
        "Stash" => t.tr("job.label.stash"),
        "Unlock worktree" => t.tr("job.label.unlock_worktree"),
        "Unset upstream" => t.tr("job.label.unset_upstream"),
        "Update submodules" => t.tr("job.label.update_submodules"),
        _ => return None,
    })
}

/// A named label's text for its name.
type Named = fn(Lang, &str) -> String;

/// The labels made of a fixed start and a name (a remote, tag, pattern or path).
fn named(t: Lang, label: &str) -> Option<String> {
    let starts: [(&str, Named); 9] = [
        ("Add remote ", |t, n| t.trf("job.label.add_remote", &[("name", &n)])),
        ("Add submodule ", |t, n| t.trf("job.label.add_submodule", &[("path", &n)])),
        ("Annotate tag ", |t, n| t.trf("job.label.annotate_tag", &[("tag", &n)])),
        ("Checkout tag ", |t, n| t.trf("job.label.checkout_tag", &[("tag", &n)])),
        ("Edit submodule ", |t, n| t.trf("job.label.edit_submodule", &[("path", &n)])),
        ("Fetch ", |t, n| t.trf("job.label.fetch_remote", &[("name", &n)])),
        ("Remove remote ", |t, n| t.trf("job.label.remove_remote", &[("name", &n)])),
        ("Remove submodule ", |t, n| t.trf("job.label.remove_submodule", &[("path", &n)])),
        ("Track ", |t, n| t.trf("job.label.track", &[("pattern", &n)])),
    ];
    // "Fetch all" and "Fetch and push" are fixed; "Track x in LFS" is not "Track <pattern>".
    if label.ends_with(" in LFS") || label.starts_with("Fetch ") && label.contains(" and ") {
        return None;
    }
    starts.iter().find_map(|(start, text)| label.strip_prefix(start).filter(|n| !n.is_empty() && !n.contains(' ')).map(|n| text(t, n)))
}

#[cfg(test)]
mod tests {
    use super::shown;
    use glowfly_core::i18n::Lang;

    #[test]
    fn english_labels_read_as_they_are() {
        for label in ["Pull", "Fetch all", "Reset (hard)", "Fetch origin", "Track *.bin", "Add submodule libs/x", "Cherry-pick 3 commits"] {
            assert_eq!(shown(Lang::En, label), label);
        }
    }

    #[test]
    fn other_languages_translate_fixed_and_named_labels() {
        assert_eq!(shown(Lang::De, "Pull"), "Pullen");
        assert_eq!(shown(Lang::Es, "Fetch all"), "Obtener todo");
        assert_eq!(shown(Lang::De, "Fetch origin"), "origin abrufen");
        assert_eq!(shown(Lang::Es, "Remove submodule libs/x"), "Quitar el submódulo libs/x");
        // Unknown, or a name with a space in it: as it is.
        assert_eq!(shown(Lang::De, "Cherry-pick 3 commits"), "Cherry-pick 3 commits");
        assert_eq!(shown(Lang::De, "Delete feature and fetch origin"), "Delete feature and fetch origin");
        assert_eq!(shown(Lang::De, "Track a b in LFS"), "Track a b in LFS");
    }
}
