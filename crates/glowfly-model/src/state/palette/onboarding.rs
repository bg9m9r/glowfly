//! The palette's onboarding entries: "Show the welcome", the tips' on or off switch (whichever
//! applies), "Show tips again" and Getting started always (even when hidden from the Help
//! menu), and "Take the tour" (or "Resume the tour") while a repository is open.

use crate::onboarding::OnboardingMsg;
use crate::state::AppState;
use crate::state::onboarding_tour::TourStep;
use crate::state::palette::commands::{Entry, PaletteCommand as C, entry, with_detail};

pub(in crate::state) fn entries(st: &AppState, out: &mut Vec<Entry>) {
    out.push(entry(
        C::Onboarding(OnboardingMsg::ShowWelcome),
        "Show the welcome",
        "getting started first run setup intro identity name email connect",
    ));
    let t = st.settings.lang();
    let tips = "hints help onboarding";
    let on = st.settings.hints;
    let toggle = if on { t.tr("palette.tips.off") } else { t.tr("palette.tips.on") };
    out.push(entry(C::Onboarding(OnboardingMsg::SetHints(!on)), toggle, tips));
    out.push(entry(C::Onboarding(OnboardingMsg::ResetHints), t.tr("palette.tips.again"), tips));
    let learn = "tips learn features discover cards help";
    out.push(entry(C::Onboarding(OnboardingMsg::OpenGettingStarted), t.tr("palette.getting_started"), learn));
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
