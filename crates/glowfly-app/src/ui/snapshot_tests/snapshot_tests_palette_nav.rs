//! The palette's list keys and hover: PageUp/PageDown and the end keys (Cmd/Ctrl+Home/End, and
//! plain Home/End only while the query is empty) push jumps, moving the pointer onto a row
//! selects it, and a page down shows the list scrolled. Also `palette_row`, which finds a row
//! inside the open palette only (the toolbar and panels may use the same words).

use super::snapshot_tests::{harness, remove_fixture, render, with_repo};
use super::snapshot_tests_worktrees::H;
use crate::msg::{Msg, PaletteMsg};
use crate::state::palette::{Palette, shown};
use crate::state::palette_nav::Jump;
use eframe::egui::accesskit::Role;
use eframe::egui::{Key, Modifiers, pos2};
use egui_kittest::kittest::{Node, Queryable};
use glowfly_core::settings::ThemeChoice;

/// The one button labelled `label` inside the open palette.
pub(in crate::ui) fn palette_row<'h>(h: &'h H, label: &'h str) -> Node<'h> {
    let area = super::palette::area_rect(&h.ctx).expect("the palette is open");
    let inside =
        |n: &Node<'_>| n.raw_bounds().is_some_and(|b| area.contains(pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32)));
    let mut rows: Vec<Node<'h>> = h.query_all_by_role_and_label(Role::Button, label).filter(inside).collect();
    assert_eq!(rows.len(), 1, "one palette row {label:?}");
    rows.remove(0)
}

/// The jumps pushed since the last call, which clears the messages.
fn jumps(h: &mut H) -> Vec<Jump> {
    let msgs = std::mem::take(&mut h.state_mut().1);
    msgs.iter()
        .filter_map(|m| match m {
            Msg::Palette(PaletteMsg::PaletteJump(j)) => Some(*j),
            _ => None,
        })
        .collect()
}

fn hovers(h: &mut H) -> Vec<usize> {
    let msgs = std::mem::take(&mut h.state_mut().1);
    msgs.iter()
        .filter_map(|m| match m {
            Msg::Palette(PaletteMsg::PaletteHover(i)) => Some(*i),
            _ => None,
        })
        .collect()
}

fn press(h: &mut H, modifiers: Modifiers, key: Key) -> Vec<Jump> {
    h.press_key_modifiers(modifiers, key);
    h.run();
    jumps(h)
}

#[test]
fn palette_page_and_end_keys() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "palette-nav-keys");
    st.palette = Some(Palette::default());
    let mut h = harness(st, true);
    h.run();
    jumps(&mut h);
    assert_eq!(press(&mut h, Modifiers::NONE, Key::PageDown), [Jump::PageDown]);
    assert_eq!(press(&mut h, Modifiers::NONE, Key::PageUp), [Jump::PageUp]);
    assert_eq!(press(&mut h, Modifiers::NONE, Key::End), [Jump::Last], "plain End with an empty query");
    assert_eq!(press(&mut h, Modifiers::COMMAND, Key::Home), [Jump::First]);
    assert_eq!(press(&mut h, Modifiers::NONE, Key::Home), [Jump::First]);

    // With a query, plain Home and End are the field's; Cmd/Ctrl+Home and End still jump.
    h.state_mut().0.palette.as_mut().unwrap().query = "re".into();
    h.run();
    jumps(&mut h);
    assert!(press(&mut h, Modifiers::NONE, Key::Home).is_empty());
    assert!(press(&mut h, Modifiers::NONE, Key::End).is_empty());
    assert_eq!(press(&mut h, Modifiers::COMMAND, Key::End), [Jump::Last]);
    assert_eq!(press(&mut h, Modifiers::COMMAND, Key::Home), [Jump::First]);
    assert_eq!(press(&mut h, Modifiers::NONE, Key::PageDown), [Jump::PageDown]);
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn palette_free_text_step_keeps_the_keys() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "palette-nav-input");
    let mode = crate::state::palette::Mode::Input(crate::state::palette::InputKind::StashMessage);
    st.palette = Some(Palette { mode, ..Default::default() });
    let mut h = harness(st, true);
    h.run();
    jumps(&mut h);
    for (m, key) in [(Modifiers::NONE, Key::PageDown), (Modifiers::NONE, Key::Home), (Modifiers::COMMAND, Key::End)] {
        assert!(press(&mut h, m, key).is_empty(), "{key:?}");
    }
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn palette_hover_selects_on_pointer_movement() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "palette-nav-hover");
    let labels: Vec<String> = shown(&st, "").into_iter().map(|s| s.entry.label).collect();
    st.palette = Some(Palette::default());
    let mut h = harness(st, true);
    h.run();
    // The pointer first comes in over row 2, then moves to row 3.
    palette_row(&h, &labels[2]).hover();
    h.run();
    hovers(&mut h);
    palette_row(&h, &labels[3]).hover();
    h.run();
    assert_eq!(hovers(&mut h), [3], "moving onto a row selects it");
    h.run();
    assert!(hovers(&mut h).is_empty(), "a still pointer selects nothing more");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn palette_page_down() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "palette-page-down");
    st.palette = Some(Palette { selected: 12, scroll_to_selected: true, ..Default::default() });
    render("palette_page_down", st, true);
    remove_fixture(&dir);
}
