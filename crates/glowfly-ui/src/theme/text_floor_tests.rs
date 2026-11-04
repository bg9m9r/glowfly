//! Text that must read at 4.5:1 with no allowance in every built-in theme (the high-contrast
//! ones included): a field's placeholder (`faint`) on the field fill, the status bar's `faint`
//! on `toolbar`, `weak` on a pane's unfocused selection, and a search-dimmed row's message and
//! metadata on the background and on every lane band. Also the find fill behind a search's
//! matched text, on every surface a graph row can have, and the live-updates marks on `toolbar`.

use super::branch_colours::{line_in, swatches};
use super::contrast::ratio;
use super::tokens::selected_inactive;
use super::*;
use crate::graph::dim_inks;
use crate::theme::surfaces::{band_color, highlight_color};
use eframe::egui::Color32;
use glowfly_core::settings::BuiltinTheme;

const FLOOR: f64 = 4.5;

#[test]
fn quiet_text_reads_at_four_and_a_half() {
    let mut failures = Vec::new();
    for t in BuiltinTheme::ALL {
        let p = builtin(t);
        let (message, meta) = dim_inks(p);
        let mut pairs: Vec<(String, Color32, Color32)> = vec![
            ("placeholder on field fill".into(), p.faint, p.field_fill()),
            ("faint on toolbar".into(), p.faint, p.toolbar),
            ("weak on selected_inactive".into(), p.weak, selected_inactive(p)),
            ("dimmed message on bg".into(), message, p.bg),
            ("dimmed metadata on bg".into(), meta, p.bg),
        ];
        for lane in swatches() {
            let band = band_color(line_in(lane, p), p);
            pairs.push((format!("dimmed message on the {lane:?} band"), message, band));
            pairs.push((format!("dimmed metadata on the {lane:?} band"), meta, band));
        }
        for (what, fg, bg) in pairs {
            let r = ratio(fg, bg);
            if fg.a() != 255 || r < FLOOR {
                failures.push(format!("{t:?}: {what} is {r:.2}:1 (alpha {})", fg.a()));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn matched_text_reads_on_the_find_fill_and_the_fill_shows() {
    let mut failures = Vec::new();
    for t in BuiltinTheme::ALL {
        let p = builtin(t);
        let min = if p.high_contrast { 7.0 } else { FLOOR };
        let mut unders = vec![("bg".to_owned(), p.bg), ("hover".into(), p.hover), ("selected".into(), p.selected)];
        unders.push(("selected_inactive".into(), selected_inactive(p)));
        for lane in swatches() {
            let line = line_in(lane, p);
            let band = band_color(line, p);
            unders.push((format!("the {lane:?} band"), band));
            unders.push((format!("the {lane:?} highlight"), highlight_color(band, line)));
        }
        for (name, under) in unders {
            let fill = p.find_fill(under);
            let (text, strong, shows) = (ratio(p.text, fill), ratio(p.strong, fill), ratio(fill, under));
            if fill.a() != 255 || text < min || strong < min || shows < 1.25 {
                failures.push(format!("{t:?} on {name}: text {text:.2}, strong {strong:.2}, against it {shows:.2}"));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{}",
        failures.join(
            "
"
        )
    );
}

#[test]
fn live_update_marks_show_on_the_toolbar() {
    let mut failures = Vec::new();
    for t in BuiltinTheme::ALL {
        let p = builtin(t);
        for (name, c) in [("success", p.status.added), ("warn", p.attention.warn), ("weak", p.weak)] {
            let r = ratio(c, p.toolbar);
            if r < 3.0 {
                failures.push(format!("{t:?}: {name} on toolbar is {r:.2}:1"));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{}",
        failures.join(
            "
"
        )
    );
}
