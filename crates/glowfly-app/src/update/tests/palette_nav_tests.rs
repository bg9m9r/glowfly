//! Headless tests for the palette's page and end keys (clamped, never wrapping, in the command
//! list and in a sub-list; nothing in the free-text step) and for selecting a row by hovering
//! it, which never scrolls.

use super::palette_stash_tests::two_stashes;
use super::tests::{drive, git, state};
use super::update;
use crate::msg::{Msg, PaletteMsg, RepoMsg};
use crate::state::AppState;
use crate::state::palette::{InputKind, Mode, Palette, PaletteCommand as C, StashAction, row_count};
use crate::state::palette_nav::{Jump, PAGE_ROWS};
use std::time::Instant;

/// Sends `j` and returns the new selection, checking it asks the list to scroll there.
fn jump(st: &mut AppState, j: Jump) -> usize {
    st.palette.as_mut().unwrap().scroll_to_selected = false;
    update(st, Msg::Palette(PaletteMsg::PaletteJump(j)), Instant::now());
    let p = st.palette.as_ref().unwrap();
    assert!(p.scroll_to_selected, "{j:?} scrolls to the selection");
    p.selected
}

#[test]
fn page_and_end_keys_in_the_command_list() {
    let mut st = state();
    st.palette = Some(Palette::default());
    let len = row_count(&mut st);
    assert!(len > 2 * PAGE_ROWS + 1, "enough commands for three pages: {len}");
    assert_eq!(jump(&mut st, Jump::PageDown), 12);
    assert_eq!(jump(&mut st, Jump::PageDown), 24);
    assert_eq!(jump(&mut st, Jump::PageDown), (24 + PAGE_ROWS).min(len - 1));
    assert_eq!(jump(&mut st, Jump::Last), len - 1);
    assert_eq!(jump(&mut st, Jump::PageDown), len - 1, "no wrap at the end");
    assert_eq!(jump(&mut st, Jump::First), 0);
    st.palette.as_mut().unwrap().selected = 5;
    assert_eq!(jump(&mut st, Jump::PageUp), 0);
    assert_eq!(jump(&mut st, Jump::PageUp), 0, "no wrap at the start");
}

#[test]
fn free_text_step_has_no_rows() {
    let mut st = state();
    st.palette = Some(Palette { mode: Mode::Input(InputKind::StashMessage), ..Default::default() });
    assert_eq!(row_count(&mut st), 0);
    for j in [Jump::PageDown, Jump::Last] {
        update(&mut st, Msg::Palette(PaletteMsg::PaletteJump(j)), Instant::now());
        assert_eq!(st.palette.as_ref().unwrap().selected, 0);
    }
}

#[test]
fn end_keys_in_a_sub_list() {
    let (mut st, dir) = two_stashes("palette-nav-pick");
    std::fs::write(dir.join("d.txt"), "d\n").unwrap();
    git(&dir, &["stash", "push", "-u", "-m", "third"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    update(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::ChooseStash(StashAction::Apply))), Instant::now());
    assert_eq!(row_count(&mut st), 3);
    assert_eq!(jump(&mut st, Jump::Last), 2);
    assert_eq!(jump(&mut st, Jump::PageUp), 0);
    assert_eq!(jump(&mut st, Jump::PageDown), 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn hover_selects_without_scrolling() {
    let mut st = state();
    st.palette = Some(Palette::default());
    update(&mut st, Msg::Palette(PaletteMsg::PaletteHover(4)), Instant::now());
    let p = st.palette.as_ref().unwrap();
    assert_eq!(p.selected, 4);
    assert!(!p.scroll_to_selected);
    st.palette = None;
    update(&mut st, Msg::Palette(PaletteMsg::PaletteHover(2)), Instant::now());
    assert!(st.palette.is_none(), "nothing happens with the palette closed");
}
