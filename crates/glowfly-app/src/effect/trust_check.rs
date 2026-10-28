//! The re-check the runner makes just before a trust-gated job, shell or read starts
//! (runner/trust.rs): the repository is scanned again and compared with the decision as it was
//! when update sent the work, and the commits a checkout, merge, pull or rebase brings in have
//! their hooks that follow the working tree compared too. Update records a `Seal` beside each
//! such effect it pushes (update/trust_seal.rs); the shell pairs them before running.

use glowfly_core::settings::SavedTrust;
use glowfly_core::trust::incoming::Target;
use std::path::PathBuf;

/// What the runner checks before it starts one effect.
#[derive(Clone, Debug)]
pub struct TrustCheck {
    /// The folder the effect runs in: the repository scanned.
    pub workdir: PathBuf,
    /// Where the repository really is (`Snapshot::real_workdir`): the key its decision is saved under.
    pub real: PathBuf,
    /// The decision when the effect was sent: what the scan must still be covered by.
    pub saved: Option<SavedTrust>,
    /// The commits the effect brings into the working tree.
    pub incoming: Vec<Target>,
    /// A pull's fetch, run first to learn the commit it merges, may ask for a sign-in.
    pub prompts: bool,
    /// What it is, for holding it again when the check fails.
    pub sealed: Sealed,
}

/// What a checked effect is.
#[derive(Clone, Debug, PartialEq)]
pub enum Sealed {
    /// The active job, labelled so (`RunningJob::label`).
    Job { label: String },
    /// Something that waited for the trust prompt outside the job gate (`state::trust::WaitingFor`).
    Waiting { kind: WaitingKind, label: Option<String> },
    /// The built-in terminal's shell `id`.
    Shell { id: u64 },
}

/// Which `WaitingFor` a waiting effect goes back to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WaitingKind {
    Terminal,
    Read,
    Preview,
}

/// A check update attached to the effect it pushed at `at` in its effects, named `name`
/// (`Effect::name`): the shell pairs them before running (`update::trust_checks`).
#[derive(Debug)]
pub struct Seal {
    pub at: usize,
    pub name: &'static str,
    pub check: TrustCheck,
}
