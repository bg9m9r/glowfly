//! The palette's onboarding entries: "Show the welcome" always, and "Take the tour" (or "Resume
//! the tour") while a repository is open.

use super::AppState;
use super::onboarding_tour::TourStep;
use super::palette_commands::{Entry, PaletteCommand as C, entry, with_detail};
use crate::onboarding::OnboardingMsg;

pub(super) fn entries(st: &AppState, out: &mut Vec<Entry>) {
    out.push(entry(
        C::Onboarding(OnboardingMsg::ShowWelcome),
        "Show the welcome",
        "getting started first run setup intro identity name email connect",
    ));
    if st.snap.is_none() {
        return;
    }
    // Few letters after an a, so "tag" and "tab" still find only the tag and tab commands.
    let keywords = "tutorial help intro show around explain";
    let command = C::Onboarding(OnboardingMsg::StartTour);
    out.push(match st.onboarding.tour_resume {
        Some(step) => {
            with_detail(entry(command, "Resume the tour", keywords), Some(format!("step {} of {}", step.number(), TourStep::ALL.len())))
        }
        None => entry(command, "Take the tour", keywords),
    });
}
