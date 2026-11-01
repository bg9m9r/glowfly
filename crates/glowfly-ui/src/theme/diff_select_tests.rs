//! The fill of a selected diff line (`contrast::diff_line_selected`): a step further from the
//! plain tint than the palette's own selected tint, with the text (4.5:1, 7:1 in high contrast)
//! and syntax (2:1, 4.5:1 in high contrast) still readable on it, in every built-in theme.

use super::color::Palette;
use super::contrast::{diff_line_selected, luminance, ratio};
use super::tests::{high_contrast, palettes, rgb};
use super::{GARDEN_DARK, GARDEN_LIGHT};

#[test]
fn selected_lines_stand_out_and_stay_readable() {
    let mut failures = Vec::new();
    for (t, p) in palettes() {
        let high = high_contrast(p);
        let (text_min, syn_min) = if high { (7.0, 4.5) } else { (4.5, 2.0) };
        for (add, plain, sel) in [(true, p.diff_add, p.diff_add_sel), (false, p.diff_del, p.diff_del_sel)] {
            let fill = diff_line_selected(p, add);
            let r = ratio(p.text, fill);
            if r < text_min {
                failures.push(format!("{t} add={add}: text on the selected fill is {r:.2}:1"));
            }
            for k in Palette::KEYS.iter().filter(|k| k.starts_with("syn_")) {
                let r = ratio(p.get(k).unwrap(), fill);
                if r < syn_min {
                    failures.push(format!("{t} add={add}: {k} on the selected fill is {r:.2}:1"));
                }
            }
            // At least as far from the plain tint as the palette's selected tint.
            let (dp, ds) = ((luminance(fill) - luminance(plain)).abs(), (luminance(sel) - luminance(plain)).abs());
            if dp < ds {
                failures.push(format!("{t} add={add}: the selected fill is weaker than diff_*_sel"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    // Pinned for the view snapshots' palettes, so a change is deliberate.
    let fills = |p| (diff_line_selected(p, true), diff_line_selected(p, false));
    assert_eq!(fills(&GARDEN_DARK), (rgb(0x316d44), rgb(0x8f373e)));
    assert_eq!(fills(&GARDEN_LIGHT), (rgb(0x8cd69b), rgb(0xef9e9e)));
}
