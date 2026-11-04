//! The initials tones in every built-in theme: muted and clean (not brown), the initials
//! readable on them, the discs visible on the surfaces they sit on, the eight told apart, and
//! each clear of every lane's line tone and the trunk, so the ring and the author read as two
//! marks.

use super::*;
use crate::theme::branch_colours::{line_in, swatches};
use crate::theme::builtin;
use crate::theme::contrast::brownish;
use glowfly_core::settings::BuiltinTheme;

/// OKLab distance of two colours.
fn distance(a: Color32, b: Color32) -> f64 {
    let lab = |c: Color32| {
        let (l, ch, h) = oklch(c);
        (l, ch * h.to_radians().cos(), ch * h.to_radians().sin())
    };
    let ((l1, a1, b1), (l2, a2, b2)) = (lab(a), lab(b));
    ((l1 - l2).powi(2) + (a1 - a2).powi(2) + (b1 - b2).powi(2)).sqrt()
}

fn tones(p: &Palette) -> Vec<(Color32, Color32)> {
    (0..PERSON_TONES).map(|i| tone_in(i, p)).collect()
}

#[test]
fn tones_are_muted_clean_and_their_initials_read() {
    let mut failures = Vec::new();
    for t in BuiltinTheme::ALL {
        let p = builtin(t);
        for (i, (fill, ink)) in tones(p).into_iter().enumerate() {
            let (_, chroma, _) = oklch(fill);
            let ink_ratio = ratio(ink, fill);
            if chroma > 0.06 || brownish(fill) || ink_ratio < ink_target(p) || ink != p.text {
                failures.push(format!("{t:?} tone {i}: chroma {chroma:.3}, ink {ink_ratio:.2}:1, brown {}", brownish(fill)));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn discs_show_on_their_surfaces_and_the_eight_differ() {
    let mut failures = Vec::new();
    for t in BuiltinTheme::ALL {
        let p = builtin(t);
        let all = tones(p);
        for (i, (fill, _)) in all.iter().enumerate() {
            for (what, surface) in [("bg", p.bg), ("panel", p.panel)] {
                if ratio(*fill, surface) < 1.25 {
                    failures.push(format!("{t:?} tone {i} on {what}: {:.2}:1", ratio(*fill, surface)));
                }
            }
            for (j, (other, _)) in all.iter().enumerate().skip(i + 1) {
                if distance(*fill, *other) < 0.025 {
                    failures.push(format!("{t:?} tones {i} and {j}: {:.3} apart", distance(*fill, *other)));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn tones_stay_clear_of_the_lane_colours() {
    let mut failures = Vec::new();
    for t in BuiltinTheme::ALL {
        let p = builtin(t);
        for (i, (fill, _)) in tones(p).into_iter().enumerate() {
            for s in swatches() {
                let d = distance(fill, line_in(s, p));
                if d < 0.10 {
                    failures.push(format!("{t:?} tone {i} and {s:?}'s line: {d:.3} apart"));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn a_person_keeps_their_tone_whatever_the_branch_preset() {
    let p = builtin(BuiltinTheme::Dark);
    let mut cb = *p;
    cb.branch = builtin(BuiltinTheme::HighContrastDark).branch;
    assert_eq!(tone_in(5, p), tone_in(5, &cb));
}
