//! The re-check the runner makes just before trust-gated work starts (runner/trust.rs): the
//! repository is scanned again and compared with the decision as it was when update sent the
//! work, and the commits a checkout, merge, pull, rebase or pick brings in have their hooks that
//! follow the working tree compared too. The check travels inside the effect
//! (`Effect::Checked`): update wraps each effect it sealed before returning it
//! (update/trust_seal.rs), so nothing pairs them up later.

#[cfg(test)]
use super::SandboxRule;
use super::{Effect, EffectMeta};
use glowfly_core::settings::SavedTrust;
use std::path::{Path, PathBuf};

/// What the runner checks before it starts one effect.
#[derive(Clone, Debug)]
pub struct TrustCheck {
    /// The folder the effect runs in: the repository scanned.
    pub workdir: PathBuf,
    /// Where the repository really is (`Snapshot::real_workdir`): the key its decision is saved under.
    pub real: PathBuf,
    /// The decision when the effect was sent: what the scan must still be covered by.
    pub saved: Option<SavedTrust>,
    /// What it is, for holding it again when the check fails.
    pub sealed: Sealed,
    /// The user chose Trust and run on a prompt that said what it brings in couldn't be
    /// compared: that reason, at that place, passes; anything else is still compared.
    pub accepted: Option<Accepted>,
    /// The language of the texts the runner writes for it (a job stopped part way).
    pub lang: glowfly_core::i18n::Lang,
}

/// What couldn't be compared that the user chose to run anyway: the reason, and where. `at`
/// is the index of the job's git command it was found before (`None`: the effect itself, a
/// pick or a rebase step).
#[derive(Clone, Debug, PartialEq)]
pub struct Accepted {
    pub at: Option<usize>,
    pub reason: glowfly_core::trust::incoming::Unchecked,
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
    /// A read that lets git run in the repository (a status read by git, a signature check, a
    /// diff through its filters): when the check fails it reads without git where it can, else
    /// it fails as its sender expects.
    Read,
}

/// Which `WaitingFor` a waiting effect goes back to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WaitingKind {
    Terminal,
    Read,
    Preview,
}

/// A check update recorded for the effect it pushed at `at` in its effects, named `name`
/// (`Effect::name`); update wraps them before it returns (`update::update_sealed`).
#[derive(Debug)]
pub struct Seal {
    pub at: usize,
    pub name: &'static str,
    pub check: TrustCheck,
}

/// An effect with the re-check the runner makes before it starts it (`Effect::Checked`).
#[derive(Debug)]
pub struct Checked {
    pub effect: Effect,
    pub check: TrustCheck,
}

impl EffectMeta for Checked {
    fn name(&self) -> &'static str {
        self.effect.name()
    }

    /// None: the runner takes the effect out before running it (`Runner::run`), and
    /// `execute_with` scopes the effect's own result.
    fn scope(&self) -> Option<&PathBuf> {
        None
    }

    fn timed(&self) -> bool {
        self.effect.timed()
    }

    fn queue(&self) -> Option<&Path> {
        self.effect.queue()
    }

    fn holds(&self) -> Option<&Path> {
        self.effect.holds()
    }

    fn stop_key(&self) -> Option<super::partial::StopKey> {
        self.effect.stop_key()
    }

    #[cfg(test)]
    fn sandbox(&self) -> SandboxRule {
        crate::runner::sandbox_rule(&self.effect)
    }
}

impl Effect {
    /// The effect itself, without the check it carries (`Effect::Checked`).
    pub fn unsealed(self) -> Effect {
        match self {
            Effect::Checked(c) => c.effect,
            e => e,
        }
    }

    /// The check this effect carries.
    pub fn trust_check(&self) -> Option<&TrustCheck> {
        match self {
            Effect::Checked(c) => Some(&c.check),
            _ => None,
        }
    }
}
