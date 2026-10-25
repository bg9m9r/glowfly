//! Messages and effects of the first-run welcome and the tour of the main window
//! (state/onboarding.rs, state/onboarding_tour.rs, update/onboarding.rs, runner/onboarding.rs,
//! ui/welcome.rs, ui/welcome_steps.rs, ui/tour.rs).

use crate::effect::EffectMeta;
#[cfg(test)]
use crate::effect::SandboxRule;
use crate::msg::{CloneMsg, Inactive, Msg, RepoMsg};
use glowfly_core::identity::Identity;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub enum OnboardingMsg {
    /// Sent once by the shell at start: opens the welcome on a first run.
    Startup,
    /// The palette's "Show the welcome": opens it again at the first step.
    ShowWelcome,
    /// Continue to the next step (on the identity step, the same as `SaveIdentity`).
    Next,
    /// Back to the previous step.
    Back,
    /// The identity step leaves git config alone, the service step goes on without a sign-in.
    Skip,
    /// The identity step's Continue: write the name and email when they changed.
    SaveIdentity,
    /// The last step's Open, Clone or Start buttons (Some) or Done (None).
    Finish(Option<StartChoice>),
    /// The close icon or Esc: ends the welcome early, which still counts as seen.
    CloseWelcome,
    /// Start the tour (or resume it at the step it was closed on).
    StartTour,
    TourNext,
    TourBack,
    CloseTour,
    // ---- from the runner
    /// The global git identity, read when the welcome opens.
    IdentityRead(Result<Identity, String>),
    /// The identity step's write finished.
    IdentityWritten {
        identity: Identity,
        result: Result<(), String>,
    },
}

/// What the welcome's last step starts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StartChoice {
    Open,
    Clone,
    Init,
}

impl StartChoice {
    /// The message the welcome screen's button of the same name sends.
    pub fn msg(self) -> Msg {
        match self {
            StartChoice::Open => Msg::Repo(RepoMsg::PickRepo),
            StartChoice::Clone => Msg::Clone(CloneMsg::Dialog),
            StartChoice::Init => Msg::Repo(RepoMsg::InitRepo),
        }
    }
}

impl OnboardingMsg {
    /// The welcome's effects are unscoped (the identity is in the global git config), so
    /// nothing arrives for an inactive tab.
    pub fn on_inactive(self) -> Option<Inactive> {
        match self {
            Self::Startup
            | Self::ShowWelcome
            | Self::Next
            | Self::Back
            | Self::Skip
            | Self::SaveIdentity
            | Self::Finish(_)
            | Self::CloseWelcome
            | Self::StartTour
            | Self::TourNext
            | Self::TourBack
            | Self::CloseTour
            | Self::IdentityRead(_)
            | Self::IdentityWritten { .. } => None,
        }
    }
}

/// Run by runner/onboarding.rs.
#[derive(Debug)]
pub enum OnboardingEffect {
    /// Read user.name, user.email and user.signingkey from the global config.
    ReadIdentity { cwd: PathBuf },
    /// Write the name and email to the global config (the signing key is left alone).
    WriteIdentity { cwd: PathBuf, identity: Identity },
}

impl EffectMeta for OnboardingEffect {
    /// A short name for the worker thread and the debug log.
    fn name(&self) -> &'static str {
        match self {
            OnboardingEffect::ReadIdentity { .. } => "read-identity",
            OnboardingEffect::WriteIdentity { .. } => "write-identity",
        }
    }

    fn scope(&self) -> Option<&PathBuf> {
        None
    }

    fn timed(&self) -> bool {
        true
    }

    fn queue(&self) -> Option<&Path> {
        None
    }

    #[cfg(test)]
    fn sandbox(&self) -> SandboxRule {
        SandboxRule::Drop
    }
}
