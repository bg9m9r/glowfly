//! The hand-kept list of core I/O functions the layering check used before the rule was derived
//! from core's sources (#365), kept as a regression test: every entry must still be denied to
//! views and the model, by the derivation or by `DENIED`, unless `NO_LONGER_IO` says why not.

use super::rules::DENIED;
use super::{core, violations};

/// Old entries that are allowed now, each with the reason. Only shrinks.
const NO_LONGER_IO: &[(&str, &str)] = &[];

/// The old list, as (module, or the type for an associated function; names).
const OLD_IO_FUNCTIONS: &[(&str, &[&str])] = &[
    (
        "repo",
        &[
            "load",
            "status_at",
            "messages_between",
            "commit_stats",
            "commit_detail",
            "commit_line_counts",
            "wip_line_counts",
            "compare",
            "load_diff",
            "load_diff_with",
            "load_diff_for_view",
            "load_listed_diff",
            "binary_sides",
            "apply_hunk",
            "apply_lines",
            "stage_file",
            "stage_all",
            "unstage_paths",
            "discard_file",
            "discard_untracked",
            "load_conflict",
            "file_history",
            "file_at",
            "file_at_with",
            "file_bytes_at",
            "write_resolution",
            "resolve_with_side",
            "restore_file",
            "create_file",
            "rename_file",
            "delete_file",
            "delete_staged_new_file",
            "unstage_gone_new_file",
            "os_trash",
            "submodules_at",
            "worktrees_at",
            "line_maps",
            "pr_line_maps",
        ],
    ),
    ("gitcli", &["run", "run_with_input", "version", "observe_runs"]),
    ("dashboard", &["read_repo", "fetch_target", "run_bounded"]),
    ("git_program", &["set", "probe", "discover", "precheck"]),
    ("logs", &["read_tail", "bundle_text", "write_bundle"]),
    ("i18n", &["detect_system_language"]),
    ("persist", &["append", "open_append"]),
    ("ActivityFile", &["open"]),
    ("settings", &["log_dir", "downloads_dir"]),
    ("app_update", &["download", "install_kind", "appimage_file", "current_platform"]),
    ("conflict_check", &["predict"]),
    ("stash_ops", &["changed_paths"]),
    (
        "forge",
        &[
            "verify",
            "pull_requests",
            "default_branch",
            "create_pull_request",
            "my_issues",
            "issues",
            "resolve_auth",
            "save_token",
            "delete_token",
            "from_credential_manager",
            "forget_credential_manager",
            "service",
            "repo_owners",
            "create_repo",
            "list_forks",
            "find_my_fork",
            "create_fork",
            "latest_release",
            "fetch_public",
        ],
    ),
    ("avatars", &["fetch"]),
    ("signature_check", &["verify_commit", "signature_format"]),
    ("signing", &["list_gpg_keys", "list_ssh_keys", "generate_gpg_key", "export_public_key"]),
    ("ssh_keys", &["find_keygen", "fingerprint", "list_key_pairs", "list", "generate"]),
    ("undo", &["before", "after", "plan", "redone", "discard_saving", "restore_saving", "write_saved"]),
    ("rebase", &["plan", "start", "stopped_at", "install_todo", "resume", "progress", "prepare_edit"]),
    ("askpass", &["start", "start_with", "ask", "ask_from_env", "ask_gpg", "ask_gpg_from_env"]),
    ("gpg_prompt", &["run_wrapper"]),
    ("program", &["user_program"]),
    ("real", &["passthrough"]),
    ("server", &["start", "start_with"]),
    ("pty", &["spawn"]),
    ("shell", &["resolve"]),
    ("lfs", &["status", "installed", "hook_state"]),
    ("gitflow", &["read_config"]),
    ("worktree_ops", &["check_folder", "remove", "dirty_count"]),
    ("paths", &["resolve", "temp_dir", "same_folder"]),
    ("FolderId", &["of"]),
    ("CommonDir", &["at"]),
    (
        "platform",
        &[
            "load_recent",
            "save_recent",
            "check_folders",
            "load_tabs",
            "save_tabs",
            "open_terminal",
            "open_in_file_manager",
            "reveal_in_file_manager",
        ],
    ),
];

#[test]
fn the_old_io_functions_are_still_denied() {
    let core = core();
    let mut allowed = Vec::new();
    for (module, names) in OLD_IO_FUNCTIONS {
        for name in *names {
            let entry = format!("{module}::{name}");
            if NO_LONGER_IO.iter().any(|(e, _)| *e == entry) {
                continue;
            }
            let denied = if module.starts_with(|c: char| c.is_ascii_uppercase()) {
                DENIED.iter().any(|(p, _)| *p == entry)
            } else {
                // A submodule the old list named by its last segment (`server`, `pty`).
                let path = core.module_named(module).unwrap_or_else(|| (*module).to_owned());
                !violations(&format!("use glowfly_core::{path};\n{module}::{name}(x);\n"), core).is_empty()
            };
            if !denied {
                allowed.push(entry);
            }
        }
    }
    assert!(allowed.is_empty(), "the old IO_FUNCTIONS entries no longer denied (add a reason to NO_LONGER_IO if right): {allowed:?}");
    for (entry, _) in NO_LONGER_IO {
        assert!(OLD_IO_FUNCTIONS.iter().any(|(m, ns)| ns.iter().any(|n| format!("{m}::{n}") == *entry)), "{entry}: not an old entry");
    }
}
