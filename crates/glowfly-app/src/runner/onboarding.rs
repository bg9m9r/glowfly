//! The welcome's read and write of the global git identity (`Effect::Onboarding`).

use crate::msg::Msg;
use crate::onboarding::{OnboardingEffect, OnboardingMsg};
use glowfly_core::identity;

pub(super) fn run(effect: OnboardingEffect) -> Msg {
    Msg::Onboarding(match effect {
        OnboardingEffect::ReadIdentity { cwd } => OnboardingMsg::IdentityRead(identity::read_global(&cwd, &[])),
        OnboardingEffect::WriteIdentity { cwd, identity } => {
            OnboardingMsg::IdentityWritten { result: identity::write_global(&cwd, &identity, &[]), identity }
        }
    })
}
