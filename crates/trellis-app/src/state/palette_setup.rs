//! The palette's entries that set up or configure a repository feature: Git LFS and sparse
//! checkout (Git Flow has its own file).

use super::{Entry, PaletteCommand as C, entry};

/// Initialize and Configure Git LFS, then Configure sparse checkout.
pub(super) fn setup_entries(out: &mut Vec<Entry>) {
    out.push(entry(C::InitializeLfs, "Initialize Git LFS", "lfs large file storage setup install enable binary assets"));
    out.push(entry(
        C::ConfigureLfs,
        "Configure Git LFS…",
        "lfs large file storage track patterns gitattributes fetch pull push prune preferences settings",
    ));
    out.push(entry(
        C::ConfigureSparse,
        "Configure sparse checkout…",
        "sparse checkout cone partial subset folders paths patterns monorepo skip-worktree reapply enable disable preferences settings",
    ));
}
