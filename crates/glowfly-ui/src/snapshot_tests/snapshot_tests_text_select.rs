//! Selecting text in a diff's code through the real view with pointer events: a drag across
//! three lines of a commit diff selects text and never lines, and Ctrl+C then sends exactly that
//! code; a double-click takes a word and a triple-click a line; Shift-click extends; a plain
//! click drops the text and keeps the lines; in split view the drag stays on its side; a press
//! in a text field keeps the selection, painted quieter. The goldens show text and line
//! selection together, inline in dark, light and high contrast, split, and without the
//! keyboard.

use super::super::snapshot_tests::{harness, harness_palette, remove_fixture};
use super::super::snapshot_tests_revert::with_line_diff;
use crate::msg::{DiffMsg, Msg};
use crate::state::AppState;
use crate::state::diff_text::TextSide;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::accesskit::Role;
use eframe::egui::{Color32, Event, Modifiers, PointerButton, Pos2, Shape, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::{DiffLayout, ThemeChoice};

type H = Harness<'static, (AppState, Vec<Msg>)>;

/// The left edge of the diff's rows: the sidebar's edge.
const ROWS_LEFT: f32 = 261.0;
/// The header buttons' gap to the view's right edge (`hunk_header`).
const BUTTON_GAP: f32 = 8.0;

/// Where character boundary `col` of line `line` of the hunk sits on screen (` app`, `-fix`,
/// `+fix the build`, `+on Windows`): inline, line i is i + 1 rows below the header; split,
/// `-fix` pairs with `+fix the build`, so `+on Windows` is on the row after, and the new
/// side's text starts past the middle.
struct Grid {
    header_y: f32,
    row_h: f32,
    char_w: f32,
    text_x: [f32; 2],
    split: bool,
}

impl Grid {
    fn of(h: &H, layout: DiffLayout) -> Self {
        let st = &h.state().0;
        let style = crate::ui::diff::DiffStyle::new(&st.settings);
        // The hunk's action at the row's right end, which names the lines once some are picked.
        let action = h.query_by_label("Revert hunk").or_else(|| h.query_by_label("Revert 1 line")).expect("the hunk's action");
        let b = action.bounding_box().expect("the button has bounds");
        let char_w = h.ctx.fonts(|f| f.glyph_width(&style.font, 'M'));
        let split = layout == DiffLayout::Split;
        let half = ((b.x1 as f32 + BUTTON_GAP - ROWS_LEFT) / 2.0).round();
        let text_x = if split {
            [ROWS_LEFT + style.split_cols().text, ROWS_LEFT + half + style.split_cols().text]
        } else {
            [ROWS_LEFT + style.inline_cols().text; 2]
        };
        Self { header_y: (b.y0 + b.y1) as f32 / 2.0, row_h: style.row_h, char_w, text_x, split }
    }

    /// Character boundary `col` of line `line`, on the new side in split view.
    fn at(&self, line: usize, col: usize) -> Pos2 {
        let row = if self.split && line == 3 {
            2
        } else if self.split && line == 2 {
            1
        } else {
            line
        };
        let side = usize::from(self.split);
        pos2(self.text_x[side] + (col as f32 + 0.2) * self.char_w, self.header_y + (row + 1) as f32 * self.row_h)
    }
}

fn button(h: &mut H, pos: Pos2, pressed: bool, modifiers: Modifiers) {
    h.input_mut().events.push(Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers });
}

/// Presses at `from`, drags to `to` in a few steps and releases there.
fn drag(h: &mut H, from: Pos2, to: Pos2) {
    h.input_mut().events.push(Event::PointerMoved(from));
    button(h, from, true, Modifiers::NONE);
    h.run();
    for t in [0.25, 0.5, 0.75, 1.0] {
        h.input_mut().events.push(Event::PointerMoved(from + (to - from) * t));
        h.run();
    }
    button(h, to, false, Modifiers::NONE);
    h.run();
}

/// `n` quick clicks at `at`, with `modifiers` held. The harness steps a quarter second a
/// frame, longer than a double-click takes, so each event's frame is given a clock that moves
/// on a hundredth of a second, starting ten seconds after the frames before (so the clicks
/// start a new count).
fn clicks(h: &mut H, at: Pos2, n: usize, modifiers: Modifiers) {
    let mut now = h.ctx.input(|i| i.time) + 10.0;
    let mut send = |h: &mut H, e: Event| {
        now += 0.01;
        h.input_mut().time = Some(now);
        h.input_mut().modifiers = modifiers;
        h.input_mut().events.push(e);
        h.step();
    };
    send(h, Event::PointerMoved(at));
    for _ in 0..n {
        send(h, Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: true, modifiers });
        send(h, Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: false, modifiers });
    }
    h.input_mut().modifiers = Modifiers::NONE;
    h.run();
}

/// What the copy key sends now: the text of each `Msg::Copy`.
fn copied(h: &mut H) -> Vec<String> {
    h.state_mut().1.clear();
    h.input_mut().events.push(Event::Copy);
    h.run();
    h.state().1.iter().filter_map(|m| if let Msg::Copy(t) = m { Some(t.clone()) } else { None }).collect()
}

/// Whether the view asked for any line to be picked.
fn picked_lines(h: &H) -> bool {
    h.state().1.iter().any(|m| matches!(m, Msg::Diff(DiffMsg::StartLineDrag { .. } | DiffMsg::SelectDiffLine { .. })))
}

fn open(
    name: &str,
    theme: ThemeChoice,
    layout: DiffLayout,
    selected: &[usize],
    palette: Option<crate::ui::theme::color::Palette>,
) -> (H, std::path::PathBuf) {
    let dark = theme != ThemeChoice::Light;
    let (st, dir) = with_line_diff(name, theme, layout, selected);
    let mut h = match palette {
        Some(p) => harness_palette(st, p),
        None => harness(st, dark),
    };
    h.run();
    (h, dir)
}

#[test]
fn a_drag_across_code_selects_text_and_ctrl_c_copies_exactly_it() {
    let (mut h, dir) = open("text-drag", ThemeChoice::Dark, DiffLayout::Inline, &[], None);
    let g = Grid::of(&h, DiffLayout::Inline);
    drag(&mut h, g.at(0, 1), g.at(3, 2));
    assert!(!picked_lines(&h), "a drag across code picks no lines");
    assert!(h.state().0.view.diff_lines.is_empty());
    let sel = h.state().0.view.diff_text.clone().expect("the drag selected text");
    assert_eq!(sel.side, TextSide::Inline);
    // No line numbers, no signs; the `-` and `+` lines in the order shown.
    assert_eq!(copied(&mut h), ["pp\nfix\nfix the build\non"]);
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn double_and_triple_clicks_take_a_word_and_a_line() {
    let (mut h, dir) = open("text-clicks", ThemeChoice::Dark, DiffLayout::Inline, &[], None);
    let g = Grid::of(&h, DiffLayout::Inline);
    clicks(&mut h, g.at(2, 10), 2, Modifiers::NONE);
    assert_eq!(copied(&mut h), ["build"]);
    clicks(&mut h, g.at(3, 1), 3, Modifiers::NONE);
    assert_eq!(copied(&mut h), ["on Windows"]);
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn shift_click_extends_and_a_plain_click_drops_the_text_only() {
    // `-fix` is picked in the gutter first: the code's clicks never touch it.
    let (mut h, dir) = open("text-shift", ThemeChoice::Dark, DiffLayout::Inline, &[1], None);
    let g = Grid::of(&h, DiffLayout::Inline);
    drag(&mut h, g.at(2, 0), g.at(2, 3));
    clicks(&mut h, g.at(3, 2), 1, Modifiers::SHIFT);
    assert_eq!(copied(&mut h), ["fix the build\non"]);
    clicks(&mut h, g.at(0, 1), 1, Modifiers::NONE);
    assert!(h.state().0.view.diff_text.is_none(), "a plain click drops the text");
    assert_eq!(copied(&mut h), Vec::<String>::new());
    assert!(!picked_lines(&h));
    assert_eq!(h.state().0.view.diff_lines.lines.iter().copied().collect::<Vec<_>>(), [1], "and keeps the lines");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn a_split_drag_stays_on_its_side_and_copies_that_side() {
    let (mut h, dir) = open("text-split", ThemeChoice::Dark, DiffLayout::Split, &[], None);
    let g = Grid::of(&h, DiffLayout::Split);
    drag(&mut h, g.at(0, 0), g.at(3, 2));
    let sel = h.state().0.view.diff_text.clone().expect("the drag selected text");
    assert_eq!(sel.side, TextSide::New);
    assert!(!picked_lines(&h));
    // The new side only: `-fix` on the old side isn't in it.
    assert_eq!(copied(&mut h), ["app\nfix the build\non"]);
    drop(h);
    remove_fixture(&dir);
}

/// Presses in the sidebar's filter field, a text field that takes the keyboard (the commit
/// summary isn't on screen beside a commit's diff).
fn press_in_a_text_field(h: &mut H) {
    let field = h.query_all_by_role(Role::TextInput).filter_map(|n| n.bounding_box()).find(|b| (b.x1 as f32) < ROWS_LEFT);
    let b = field.expect("the sidebar's filter field");
    let at = pos2((b.x0 + b.x1) as f32 / 2.0, (b.y0 + b.y1) as f32 / 2.0);
    h.input_mut().events.push(Event::PointerMoved(at));
    button(h, at, true, Modifiers::NONE);
    button(h, at, false, Modifiers::NONE);
    h.run();
}

/// Whether a fill of `c` was painted this frame.
fn painted(h: &H, c: Color32) -> bool {
    h.output().shapes.iter().any(|s| matches!(&s.shape, Shape::Rect(r) if r.fill == c))
}

#[test]
fn a_press_in_a_text_field_keeps_the_selection_painted_quieter() {
    let (mut h, dir) = open("text-unfocused", ThemeChoice::Dark, DiffLayout::Inline, &[], None);
    let g = Grid::of(&h, DiffLayout::Inline);
    drag(&mut h, g.at(0, 1), g.at(3, 2));
    // The harness's dark palette.
    let p = crate::ui::theme::GARDEN_DARK;
    assert!(painted(&h, p.code_sel) && !painted(&h, p.code_sel_dim), "the diff has the keyboard");
    let before = h.state().0.view.diff_text.clone();
    press_in_a_text_field(&mut h);
    assert!(before.is_some());
    assert_eq!(h.state().0.view.diff_text, before, "the selection stays");
    assert!(painted(&h, p.code_sel_dim) && !painted(&h, p.code_sel), "painted with code_sel_dim");
    // The copy key belongs to the field now.
    assert_eq!(copied(&mut h), Vec::<String>::new());
    drop(h);
    remove_fixture(&dir);
}

/// A drag across the code from `app` to `on Windows`, with `-fix` picked in the gutter: the
/// text selection's wash and nubs beside the line selection's tint, bar and check.
/// With `unfocused`, a press in a text field then takes the keyboard from the diff.
fn golden(name: &str, theme: ThemeChoice, layout: DiffLayout, palette: Option<crate::ui::theme::color::Palette>, unfocused: bool) {
    let (mut h, dir) = open(name, theme, layout, &[1], palette);
    let g = Grid::of(&h, layout);
    drag(&mut h, g.at(0, 1), g.at(3, 2));
    assert!(h.state().0.view.diff_text.is_some());
    if unfocused {
        press_in_a_text_field(&mut h);
    }
    h.check_snapshot(name);
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn diff_text_selection_dark() {
    golden("diff_text_selection_dark", ThemeChoice::Dark, DiffLayout::Inline, None, false);
}

#[test]
fn diff_text_selection_light() {
    golden("diff_text_selection_light", ThemeChoice::Light, DiffLayout::Inline, None, false);
}

#[test]
fn diff_text_selection_hc_dark() {
    golden(
        "diff_text_selection_hc_dark",
        ThemeChoice::HighContrastDark,
        DiffLayout::Inline,
        Some(crate::ui::theme::HIGH_CONTRAST_DARK),
        false,
    );
}

#[test]
fn diff_split_text_selection() {
    golden("diff_split_text_selection", ThemeChoice::Dark, DiffLayout::Split, None, false);
}

#[test]
fn diff_text_selection_unfocused_dark() {
    golden("diff_text_selection_unfocused_dark", ThemeChoice::Dark, DiffLayout::Inline, None, true);
}
