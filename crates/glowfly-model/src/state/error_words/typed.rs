//! The words for the typed errors that result messages carry instead of English text (#367):
//! one function per area's error type, the only place its variants become text in the user's
//! language (hosting services, credentials, signing keys, Preferences writes, updates, the
//! graph's, the diff's and the repository's reads, the dashboard's short reasons, rebase,
//! history edit and restack refusals, and working-tree, index and stack push writes). Each
//! gives the fragment that follows a headline's colon ("Couldn't save the sign-in for
//! github.com: <this>"); git's own words, when an error has them, are kept as they are. Pure.

use glowfly_core::app_update::UpdateError;
use glowfly_core::error::{ReadFailed, WriteFailed};
use glowfly_core::forge::ServiceError;
use glowfly_core::git_credential::CredentialError;
use glowfly_core::i18n::Lang;
use glowfly_core::identity::ConfigWriteFailed;
use glowfly_core::rebase::{Busy, LeftOut, RebaseError};
use glowfly_core::signing::KeyError;

/// Why saving or dropping an HTTPS sign-in with git's credential helper failed
/// (`CredentialMsg::Remembered` and `Forgotten`).
pub fn credential(t: Lang, e: &CredentialError) -> String {
    match e {
        CredentialError::NoStore { install: Some(store) } => t.trf("credential_errors.no_store.install", &[("store", store)]),
        CredentialError::NoStore { install: None } => t.tr("credential_errors.no_store").to_owned(),
        CredentialError::SetUpFailed { said: Some(line) }
        | CredentialError::ApproveFailed { said: Some(line) }
        | CredentialError::RejectFailed { said: Some(line) } => line.clone(),
        CredentialError::SetUpFailed { said: None } => t.tr("credential_errors.set_up").to_owned(),
        CredentialError::ApproveFailed { said: None } => t.tr("credential_errors.approve").to_owned(),
        CredentialError::RejectFailed { said: None } => t.tr("credential_errors.reject").to_owned(),
    }
}

/// Why signing in to a hosting service, or a call to it, failed, in the words its error has
/// always shown: the kind, then the service's own words (and an HTTP refusal's status)
/// (`ForgeMsg::Connected`, `TrelloBoardsLoaded`, `PublishMsg::PublishOwners` and `Published`).
pub fn service(t: Lang, e: &ServiceError) -> String {
    match e {
        ServiceError::Auth { said } => t.trf("service_errors.said.auth", &[("said", said)]),
        ServiceError::Forbidden { said } => t.trf("service_errors.said.forbidden", &[("said", said)]),
        ServiceError::NotFound { said } => t.trf("service_errors.said.not_found", &[("said", said)]),
        ServiceError::Http { status, message } => t.trf("service_errors.said.http", &[("status", status), ("said", message)]),
        ServiceError::Network { said } => t.trf("service_errors.said.network", &[("said", said)]),
        ServiceError::Decode { said } => t.trf("service_errors.said.decode", &[("said", said)]),
        ServiceError::Keyring { said } => t.trf("service_errors.said.keyring", &[("said", said)]),
        ServiceError::Other { said } => said.clone(),
    }
}

/// Why listing, generating or exporting a signing key failed (`PrefsMsg::SigningKeysLoaded`,
/// `GpgKeyGenerated` and `PublicKeyExported`).
pub fn signing_key(t: Lang, e: &KeyError) -> String {
    match e {
        KeyError::NoIdentity => t.tr("signing_errors.no_identity").to_owned(),
        KeyError::NotRun { program, said } => t.trf("signing_errors.not_run", &[("program", program), ("said", said)]),
        KeyError::Failed { program, said } => t.trf("signing_errors.failed", &[("program", program), ("said", said)]),
        KeyError::GenerateFailed { said } => t.trf("signing_errors.generate_failed", &[("said", said)]),
        KeyError::NewKeyNotFound => t.tr("signing_errors.new_key_not_found").to_owned(),
        KeyError::NoPublicKey { key } => t.trf("signing_errors.no_public_key", &[("key", key)]),
        KeyError::Io { said } => said.clone(),
    }
}

/// Why a read for the graph (a comparison, stacked lists, path search, history index), the
/// diff (hunk patch, unchanged lines, binary sides, conflict, conflict markers, file history) or
/// the repository (a load, the status, a commit's detail, a file diff, a signature check, line
/// counts, blame, the unmerged count, an operation preview, the sparse status, a stash's files)
/// failed; libgit2's and the system's words, the paths and effect names kept as they are.
pub fn read(t: Lang, e: &ReadFailed) -> String {
    use ReadFailed as R;
    match e {
        R::Said { said } | R::Other { said } => said.clone(),
        R::NotARepository { path } => t.trf("read_errors.not_a_repository", &[("path", path)]),
        R::NotOwned { root } => t.trf("read_errors.not_owned", &[("root", root)]),
        R::OpenFailed { path, why } => t.trf("read_errors.open_failed", &[("path", path), ("why", &read(t, why))]),
        R::Panicked { effect, panic } => t.trf("read_errors.panicked", &[("effect", effect), ("panic", panic)]),
        R::MissingContent { path, remote } => t.trf("read_errors.missing_content", &[("path", path), ("remote", remote)]),
        R::DiffChanged => t.tr("read_errors.diff_changed").to_owned(),
        R::FileChanged => t.tr("read_errors.file_changed").to_owned(),
        R::OldFileUnreadable => t.tr("read_errors.old_file_unreadable").to_owned(),
        R::NotConflicted { path } => t.trf("read_errors.not_conflicted", &[("path", path)]),
        R::NoFolder => t.tr("read_errors.no_folder").to_owned(),
        R::NoThread { said } => t.trf("read_errors.no_thread", &[("said", said)]),
        R::Internal { effect } => t.trf("read_errors.internal", &[("effect", effect)]),
        R::Stopped => t.tr("read_errors.stopped").to_owned(),
    }
}

/// Why a folder the dashboard lists couldn't be read, short, for a line that already names the
/// folder (`DashboardMsg::Scanned`).
pub fn unreadable(t: Lang, e: &ReadFailed) -> String {
    match e {
        ReadFailed::NotARepository { .. } => t.tr("dashboard.unreadable.not_a_repository").to_owned(),
        ReadFailed::NotOwned { .. } => t.tr("dashboard.unreadable.not_owned").to_owned(),
        e => read(t, e),
    }
}

/// Why a rebase, a graph history edit, a remaining-steps edit or a restack was refused or
/// failed (`RebaseMsg`'s results); git's and the system's words, short ids, branch names, paths
/// and config names kept as they are.
pub fn rebase(t: Lang, e: &RebaseError) -> String {
    use RebaseError as E;
    let words = match e {
        E::Said { said } | E::Other { said } => return said.clone(),
        E::Panicked { effect, panic } => return t.trf("read_errors.panicked", &[("effect", effect), ("panic", panic)]),
        E::InProgress { op } => return t.tr(busy_key(*op)).to_owned(),
        E::CommitNotOnBranch { commit } => return t.trf("rebase_errors.commit_not_on_branch", &[("commit", commit)]),
        E::NothingToRebase { branch, onto } => return t.trf("rebase_errors.nothing_to_rebase", &[("branch", branch), ("onto", onto)]),
        E::BranchNotFound { name } => return t.trf("rebase_errors.branch_not_found", &[("name", name)]),
        E::CommitNotFound { commit } => return t.trf("rebase_errors.commit_not_found", &[("commit", commit)]),
        E::Held { alias, target, elsewhere } => {
            let args: [(&str, &dyn std::fmt::Display); 2] = [("alias", alias), ("target", target)];
            return if *elsewhere { t.trf("rebase_errors.held.elsewhere", &args) } else { t.trf("rebase_errors.held.not_branch", &args) };
        }
        E::NotMoved { branches } => return not_moved(t, branches),
        E::Stale => t.tr("rebase_errors.stale"),
        E::Changed => t.tr("rebase_errors.changed"),
        E::Detached => t.tr("rebase_errors.detached"),
        E::FirstCombines => t.tr("rebase_errors.first_combines"),
        E::EmptyReword => t.tr("rebase_errors.empty_reword"),
        E::NotOnBranch => t.tr("rebase_errors.not_on_branch"),
        E::SomeNotOnBranch => t.tr("rebase_errors.some_not_on_branch"),
        E::MergeSelected => t.tr("rebase_errors.merge_selected"),
        E::SomeMergeSelected => t.tr("rebase_errors.some_merge_selected"),
        E::SideLine => t.tr("rebase_errors.side_line"),
        E::SomeSideLine => t.tr("rebase_errors.some_side_line"),
        E::BranchTip => t.tr("rebase_errors.branch_tip"),
        E::SomeBranchTips => t.tr("rebase_errors.some_branch_tips"),
        E::SplitByBranch => t.tr("rebase_errors.split_by_branch"),
        E::CombineAfterMerge => t.tr("rebase_errors.combine_after_merge"),
        E::LostToReset => t.tr("rebase_errors.lost_to_reset"),
        E::RootSquash => t.tr("rebase_errors.root_squash"),
        E::ParentMerge => t.tr("rebase_errors.parent_merge"),
        E::NotConsecutive => t.tr("rebase_errors.not_consecutive"),
        E::TooFewToSquash => t.tr("rebase_errors.too_few_to_squash"),
        E::DropsEverything => t.tr("rebase_errors.drops_everything"),
        E::NotRebasing => t.tr("rebase_errors.not_rebasing"),
        E::EmptyCombined => t.tr("rebase_errors.empty_combined"),
        E::EmptyExec => t.tr("rebase_errors.empty_exec"),
        E::ExecOneLine => t.tr("rebase_errors.exec_one_line"),
        E::CombineAfterStep => t.tr("rebase_errors.combine_after_step"),
        E::NotInteractive => t.tr("rebase_errors.not_interactive"),
        E::UnsupportedSteps => t.tr("rebase_errors.unsupported_steps"),
        E::UpdateRefInGroup => t.tr("rebase_errors.update_ref_in_group"),
        E::RefsChanged => t.tr("rebase_errors.refs_changed"),
        E::TodoMoved => t.tr("rebase_errors.todo_moved"),
        E::NotUtf8Branch => t.tr("rebase_errors.not_utf8"),
        E::StackChanged => t.tr("rebase_errors.stack_changed"),
        E::StillRebasing => t.tr("rebase_errors.still_rebasing"),
        E::NoCommitsYet => t.tr("rebase_errors.no_commits_yet"),
        E::EditTodoFailed => t.tr("rebase_errors.edit_todo_failed"),
    };
    words.to_owned()
}

/// The sentence refusing an interactive rebase while `op` is in progress (the commit menu's
/// own words for it).
fn busy_key(op: Busy) -> &'static str {
    match op {
        Busy::Merge => "commits.blocked.in_progress.merge",
        Busy::Rebase => "commits.blocked.in_progress.rebase",
        Busy::CherryPick => "commits.blocked.in_progress.cherry_pick",
        Busy::Revert => "commits.blocked.in_progress.revert",
        Busy::PatchApplication => "commits.blocked.in_progress.am",
        Busy::Bisect => "commits.blocked.in_progress.bisect",
    }
}

/// Why git won't move `branches` with the rebase: each named, with where git says it is
/// checked out (the path as git wrote it).
fn not_moved(t: Lang, branches: &[LeftOut]) -> String {
    let named: Vec<String> = branches
        .iter()
        .map(|b| match &b.checked_out_at {
            Some(path) => t.trf("rebase_errors.checked_out_at", &[("branch", &b.name), ("path", path)]),
            None => b.name.clone(),
        })
        .collect();
    let elsewhere = branches.iter().any(|b| b.checked_out_at.is_some());
    let branches = named.join(", ");
    if elsewhere {
        t.trf("rebase_errors.not_moved.elsewhere", &[("branches", &branches)])
    } else {
        t.trf("rebase_errors.not_moved", &[("branches", &branches)])
    }
}

/// Why a write to the working tree or index, a revert, a restore from a commit, a working-tree
/// file operation or a stack push failed (`StageMsg::LocalOpDone`, `Reverted`,
/// `FilesMsg::FileRestored`, `WorkFileMsg::WorkFileDone`, `StackMsg::Pushed`); git's, libgit2's
/// and the system's words kept as they are.
pub fn write(t: Lang, e: &WriteFailed) -> String {
    match e {
        WriteFailed::Said { said } | WriteFailed::Other { said } => said.clone(),
        WriteFailed::MissingContent { path, remote } => t.trf("read_errors.missing_content", &[("path", path), ("remote", remote)]),
        WriteFailed::Panicked { effect, panic } => t.trf("read_errors.panicked", &[("effect", effect), ("panic", panic)]),
    }
}

/// Why a Preferences page's git config write failed (`PrefsMsg::ConfigSaved`).
pub fn config_write(t: Lang, e: &ConfigWriteFailed) -> String {
    t.trf("config_errors.write", &[("key", &e.key), ("said", &e.said)])
}

/// Why a check for updates, a download or Install and restart failed, in the words it has
/// always shown, with the system's and the hosting service's own words and the paths as they
/// are shown (`UpdateMsg::Checked`, `Downloaded` and `RestartReady`).
pub fn update(t: Lang, e: &UpdateError) -> String {
    use UpdateError as E;
    match e {
        E::FeedNotAbsolute { value } => t.trf("update_errors.feed_not_absolute", &[("value", value)]),
        E::FeedFolderMissing { dir } => t.trf("update_errors.feed_folder_missing", &[("dir", dir)]),
        E::Service(e) => service(t, e),
        E::ReadFailed { path, said } => t.trf("update_errors.read_failed", &[("path", path), ("said", said)]),
        E::ReadTooLong { path, name } => t.trf("update_errors.read_too_long", &[("path", path), ("name", name)]),
        E::NoOwnProgram { said } => t.trf("update_errors.no_own_program", &[("said", said)]),
        E::NoDownload => t.tr("update_errors.no_download").to_owned(),
        E::NoDownloadsFolder => t.tr("update_errors.no_downloads_folder").to_owned(),
        E::NoChecksum { name } => t.trf("update_errors.no_checksum", &[("name", name)]),
        E::Checksums(why) => t.trf("update_errors.checksums", &[("why", &update(t, why))]),
        E::Unsigned => t.tr("update_errors.unsigned").to_owned(),
        E::SignatureUnreadable(why) => t.trf("update_errors.signature_unreadable", &[("why", &update(t, why))]),
        E::BadSignature => t.tr("update_errors.bad_signature").to_owned(),
        E::SignedForAnotherRelease { tag, signed: Some(signed) } => {
            t.trf("update_errors.signed_for_other", &[("signed", signed), ("tag", tag)])
        }
        E::SignedForAnotherRelease { tag, signed: None } => t.trf("update_errors.not_signed_for", &[("tag", tag)]),
        E::TooLarge { name, mb } => t.trf("update_errors.too_large", &[("name", name), ("mb", mb)]),
        E::CreateFailed { path, said } => t.trf("update_errors.create_failed", &[("path", path), ("said", said)]),
        E::DownloadFailed { name, why } => t.trf("update_errors.download_failed", &[("name", name), ("why", &update(t, why))]),
        E::LargerThanExpected { name } => t.trf("update_errors.larger_than_expected", &[("name", name)]),
        E::BadFileName { name } => t.trf("update_errors.bad_file_name", &[("name", name)]),
        E::ChecksumMismatch => t.tr("update_errors.checksum_mismatch").to_owned(),
        E::SaveFailed { path, said } => t.trf("update_errors.save_failed", &[("path", path), ("said", said)]),
        E::NameTaken { name, dir } => t.trf("update_errors.name_taken", &[("name", name), ("dir", dir)]),
        E::WrongSize { name, got, want } => t.trf("update_errors.wrong_size", &[("name", name), ("got", got), ("want", want)]),
        E::Stopped => t.tr("update_errors.stopped").to_owned(),
        E::ChangedSinceChecked => t.tr("update_errors.changed").to_owned(),
        E::OpenFailed { path, said, install_with } => {
            let open = t.trf("update_errors.open_failed", &[("path", path), ("said", said)]);
            match install_with {
                Some(tool) => format!("{open}\n{}", t.trf("update_errors.install_with", &[("tool", tool), ("path", path)])),
                None => open,
            }
        }
        E::NotInPlace => t.tr("update_errors.not_in_place").to_owned(),
        E::NotRunningFromAppImage => t.tr("update_errors.not_running_from_appimage").to_owned(),
        E::NotAnAppImage => t.tr("update_errors.not_an_appimage").to_owned(),
        E::ArchiveUnreadable { file, said } => t.trf("update_errors.archive_unreadable", &[("file", file), ("said", said)]),
        E::NotAnArchive { file } => t.trf("update_errors.not_an_archive", &[("file", file)]),
        E::ArchiveTooLarge { mb } => t.trf("update_errors.archive_too_large", &[("mb", mb)]),
        E::TwoCopies { name } => t.trf("update_errors.two_copies", &[("name", name)]),
        E::NotRegular { name } => t.trf("update_errors.not_regular", &[("name", name)]),
        E::NoProgram { program } => t.trf("update_errors.no_program", &[("program", program)]),
        E::WriteFailed { path, said } => t.trf("update_errors.write_failed", &[("path", path), ("said", said)]),
        E::RemoveFailed { path, said } => t.trf("update_errors.remove_failed", &[("path", path), ("said", said)]),
        E::NoHelperPlace { tried } => t.trf("update_errors.no_helper_place", &[("tried", &UpdateError::reasons(tried))]),
        E::ReplaceFailed { folder, said, left } => {
            let mut text = t.trf("update_errors.replace_failed", &[("folder", folder), ("said", said)]);
            for path in left {
                text.push(' ');
                text.push_str(&t.trf("update_errors.old_file_left", &[("path", path)]));
            }
            text
        }
        E::RollbackFailed { failed } => UpdateError::reasons(failed),
        E::NotUndone { error, undo } => format!("{} {}", update(t, error), update(t, undo)),
        E::HelperNotStarted { said } => t.trf("update_errors.helper_not_started", &[("said", said)]),
        E::HelperNoInput => t.tr("update_errors.helper_no_input").to_owned(),
        E::Internal { effect, panic } => t.trf("update_errors.internal", &[("effect", effect), ("panic", panic)]),
        E::Said { said } => said.clone(),
    }
}
