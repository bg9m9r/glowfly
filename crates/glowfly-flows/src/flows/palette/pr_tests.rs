//! The palette's Show pull request entry for the checked-out branch: listed only while the branch
//! has an open pull request on a code host, and choosing it opens the panel.

use crate::flows::push_pr_state_tests::pr_for;
use crate::flows::push_pr_tests::{cleanup, with_host};
use crate::flows::tests::{drive, git};
use crate::flows::update;
use glowfly_model::msg::{Msg, PaletteMsg, RepoMsg};
use glowfly_model::state::AppState;
use glowfly_model::state::palette::{PaletteCommand, entries, shown};
use std::time::Instant;

fn shows(st: &AppState) -> Vec<PaletteCommand> {
    entries(st).into_iter().map(|e| e.command).filter(|c| matches!(c, PaletteCommand::ShowPr(_))).collect()
}

#[test]
fn show_pr_entry_only_while_the_branch_has_one() {
    let (mut st, work, bare) = with_host("palette-show-pr");
    git(&work, &["checkout", "-q", "topic"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(shows(&st).is_empty(), "no pull request yet");

    st.session.forge.as_mut().unwrap().prs = vec![pr_for("topic")];
    assert_eq!(shows(&st), [PaletteCommand::ShowPr(7)]);
    let found = shown(&st, "");
    let entry = found.iter().find(|s| s.entry.command == PaletteCommand::ShowPr(7)).expect("listed");
    assert_eq!(entry.entry.label, "Show pull request #7 for topic");
    assert!(shown(&st, "mr").iter().any(|s| s.entry.command == PaletteCommand::ShowPr(7)), "found by its keywords");
    let creates = entries(&st).into_iter().any(|e| matches!(e.command, PaletteCommand::CreatePr(_) | PaletteCommand::PushAndCreatePr(_)));
    assert!(!creates, "no create entries for a branch with an open pull request");

    update(&mut st, Msg::Palette(PaletteMsg::TogglePalette), Instant::now());
    assert!(st.palette.is_some());
    update(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::ShowPr(7))), Instant::now());
    assert!(st.palette.is_none(), "the palette closes");
    assert_eq!(st.session.pull_request.view.as_ref().map(|v| v.pr.id), Some(7));
    assert_eq!(st.palette_recent.keys(), ["show-pr"]);

    st.session.forge.as_mut().unwrap().prs.clear();
    assert!(shows(&st).is_empty());
    st.session.forge.as_mut().unwrap().prs = vec![pr_for("topic")];
    st.session.forge = None;
    assert!(shows(&st).is_empty(), "no code host");
    cleanup(&work, &bare);
}
