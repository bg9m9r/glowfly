//! Headless tests for the pane keys and the graph's keys: a pane key asks the view to focus its
//! pane, showing the left or details panel first, the diff only while one is open, the commit
//! message as Go to the commit message does; Enter in the graph shows the details panel's file
//! list; F6 goes round the panes that are drawn; Ctrl+C copies the selected commits' full IDs
//! newest first; and the Cherry-pick and Revert command rows run the palette's entries for the
//! selected commits, which cherry-pick them oldest first or revert them newest first.

use super::selection_tests::{fixture, up};
use crate::effect::{Effect, UiEffect};
use crate::keymap::{BindingId, KeyAction};
use crate::msg::{LayoutMsg, Msg, PaletteMsg, RepoMsg, SelectionMsg};
use crate::state::layout::{Pane, Panel};
use crate::state::palette::{PaletteCommand as C, entries};
use crate::state::palette_bindings::command_for;
use crate::state::{AppState, Selection};

fn key(st: &mut AppState, action: KeyAction) -> Vec<Effect> {
    up(st, Msg::Shortcut(action))
}

fn copied(fx: &[Effect]) -> Option<&str> {
    fx.iter().find_map(|e| match e {
        Effect::Ui(UiEffect::CopyText(t)) => Some(t.as_str()),
        _ => None,
    })
}

#[test]
fn pane_keys_ask_for_their_pane_and_show_its_panel() {
    let (mut st, f) = fixture("pane-keys-ask");
    up(&mut st, Msg::Repo(RepoMsg::SelectCommit(f.c(3))));
    for p in [Panel::Left, Panel::Details] {
        up(&mut st, Msg::Layout(LayoutMsg::SetPanel { panel: p, shown: false }));
    }
    key(&mut st, KeyAction::FocusPane(Pane::Sidebar));
    assert!(st.settings.show_left_panel, "the left panel is shown");
    assert_eq!(st.view.pane_focus, Some(Pane::Sidebar));
    key(&mut st, KeyAction::FocusPane(Pane::Files));
    assert!(st.settings.show_details_panel, "the details panel is shown");
    assert_eq!(st.view.pane_focus, Some(Pane::Files));
    key(&mut st, KeyAction::FocusPane(Pane::Central));
    assert_eq!(st.view.pane_focus, Some(Pane::Central));

    // The diff only while one is open.
    st.view.pane_focus = None;
    key(&mut st, KeyAction::FocusPane(Pane::Diff));
    assert_eq!(st.view.pane_focus, None, "no diff is open");

    // The commit message as Go to the commit message: the WIP panel, its summary focused.
    key(&mut st, KeyAction::FocusPane(Pane::Message));
    assert_eq!(st.session.selection, Selection::Wip);
    assert!(st.view.focus_message);
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[test]
fn enter_in_the_graph_goes_to_the_details_file_list() {
    let (mut st, f) = fixture("pane-keys-enter");
    st.session.selection = Selection::None;
    key(&mut st, KeyAction::OpenDetails);
    assert_eq!(st.view.pane_focus, None, "nothing selected, no file list");
    up(&mut st, Msg::Repo(RepoMsg::SelectCommit(f.c(2))));
    up(&mut st, Msg::Layout(LayoutMsg::SetPanel { panel: Panel::Details, shown: false }));
    key(&mut st, KeyAction::OpenDetails);
    assert!(st.settings.show_details_panel);
    assert_eq!(st.view.pane_focus, Some(Pane::Files));
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[test]
fn f6_goes_round_the_drawn_panes() {
    let all = |_| true;
    assert_eq!(Pane::cycle(None, 1, all), Pane::Central, "from no pane, the central view");
    assert_eq!(Pane::cycle(Some(Pane::Sidebar), 1, all), Pane::Central);
    assert_eq!(Pane::cycle(Some(Pane::Central), 1, all), Pane::Files);
    assert_eq!(Pane::cycle(Some(Pane::Files), 1, all), Pane::Message);
    assert_eq!(Pane::cycle(Some(Pane::Message), 1, all), Pane::Sidebar);
    assert_eq!(Pane::cycle(Some(Pane::Sidebar), -1, all), Pane::Message);
    assert_eq!(Pane::cycle(Some(Pane::Diff), 1, all), Pane::Central, "the diff is the central view");
    // Panes that aren't drawn are passed over.
    let no_message = |p| p != Pane::Message;
    assert_eq!(Pane::cycle(Some(Pane::Files), 1, no_message), Pane::Sidebar);
    assert_eq!(Pane::cycle(Some(Pane::Sidebar), 1, |_| false), Pane::Central, "the central view is always drawn");
}

#[test]
fn copy_commit_ids_newest_first() {
    let (mut st, f) = fixture("pane-keys-copy");
    st.session.selection = Selection::None;
    assert!(copied(&up(&mut st, Msg::Selection(SelectionMsg::CopyIds))).is_none(), "nothing selected");

    up(&mut st, Msg::Repo(RepoMsg::SelectCommit(f.c(2))));
    let fx = up(&mut st, Msg::Selection(SelectionMsg::CopyIds));
    assert_eq!(copied(&fx), Some(f.c(2).to_string().as_str()));
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Copied 1 commit ID"));

    // Toggled in oldest first, copied newest first.
    for n in [4, 5] {
        up(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(f.c(n))));
    }
    let fx = up(&mut st, Msg::Selection(SelectionMsg::CopyIds));
    let want = [f.c(5), f.c(4), f.c(2)].map(|o| o.to_string()).join("\n");
    assert_eq!(copied(&fx), Some(want.as_str()));
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Copied 3 commit IDs"));
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[test]
fn cherry_pick_and_revert_rows_run_the_palette_entries() {
    assert_eq!(command_for(BindingId::CherryPick), Some(C::PickSelected { revert: false }));
    assert_eq!(command_for(BindingId::RevertCommit), Some(C::PickSelected { revert: true }));
    assert_eq!(C::PickSelected { revert: false }.binding(), Some(BindingId::CherryPick));
    let (mut st, f) = fixture("pane-keys-pick");
    let listed = |st: &AppState| -> Vec<String> {
        entries(st).into_iter().filter(|e| matches!(e.command, C::PickSelected { .. })).map(|e| e.label).collect()
    };
    st.session.selection = Selection::Wip;
    assert!(listed(&st).is_empty(), "no commit selected");
    up(&mut st, Msg::Repo(RepoMsg::SelectCommit(f.side)));
    assert_eq!(listed(&st), ["Cherry-pick the selected commit", "Revert the selected commit"]);
    // HEAD's own commit can't be cherry-picked onto itself; it can be reverted.
    up(&mut st, Msg::Repo(RepoMsg::SelectCommit(f.c(5))));
    assert_eq!(listed(&st), ["Revert the selected commit"]);
    up(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(f.c(4))));
    assert_eq!(listed(&st), ["Cherry-pick the 2 selected commits", "Revert the 2 selected commits"]);
    let _ = std::fs::remove_dir_all(&f.dir);
}

/// The git command of the cherry-pick or revert job `fx` starts, if it starts one.
fn pick_cmd(fx: &[Effect]) -> Option<Vec<String>> {
    fx.iter().find_map(|e| match e {
        Effect::Pick(j) => j.cmds.first().cloned(),
        _ => None,
    })
}

#[test]
fn running_the_cherry_pick_and_revert_rows_sends_their_message() {
    let (mut st, f) = fixture("pane-keys-pick-run");
    // The side branch's commit, onto main: a cherry-pick.
    up(&mut st, Msg::Repo(RepoMsg::SelectCommit(f.side)));
    let fx = up(&mut st, Msg::Palette(PaletteMsg::RunBoundCommand(C::PickSelected { revert: false })));
    assert_eq!(pick_cmd(&fx), Some(vec!["cherry-pick".to_owned(), f.side.to_string()]));
    st.job = None;
    // Two of main's commits, newest first: a revert.
    up(&mut st, Msg::Repo(RepoMsg::SelectCommit(f.c(2))));
    up(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(f.c(4))));
    let fx = up(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::PickSelected { revert: true })));
    let cmd = pick_cmd(&fx).expect("a revert job");
    assert_eq!(cmd[0], "revert");
    assert!(cmd.ends_with(&[f.c(4).to_string(), f.c(2).to_string()]), "{cmd:?}");
    let _ = std::fs::remove_dir_all(&f.dir);
}
