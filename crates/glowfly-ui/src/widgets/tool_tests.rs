//! The tool slot's count badge (its text, its place inside the slot with captions shown or
//! hidden) and the contrast of the badge, at rest and lit, and of the disabled look in every
//! built-in theme.

use super::*;
use crate::theme::builtin;
use crate::theme::contrast::ratio;
use crate::theme::surfaces::over;
use glowfly_core::settings::BuiltinTheme;

#[test]
fn a_badge_caps_its_count() {
    assert_eq!(badge_text(3), "3");
    assert_eq!(badge_text(9), "9");
    assert_eq!(badge_text(10), "9+");
    assert_eq!(badge_text(120), "9+");
}

#[test]
fn a_badge_stays_inside_its_slot() {
    let h = ChipSize::Badge.metrics().h;
    for badge_w in [18.0, 22.0, 26.0] {
        // Slots as `badged_tool_width` sizes them: at least what the widest badge needs.
        for slot_w in [TOOL_W.max(badged_min(badge_w)), TOOL_W_BADGED_BARE.max(badged_min(badge_w)), 90.0] {
            let slot = Rect::from_min_size(pos2(100.0, 4.0), vec2(slot_w, TOOL_H));
            let r = badge_rect(slot, badge_w);
            assert!(slot.contains_rect(r.expand(BADGE_RING)), "slot {slot_w}, badge {badge_w}: {r:?} in {slot:?}");
            assert_eq!((r.top(), r.height()), (slot.top() + BADGE_TOP, h));
            assert_eq!(r.left(), slot.center().x + BADGE_X, "it never slides onto the icon");
            assert!(r.right() <= slot.right() - BADGE_EDGE);
        }
    }
}

#[test]
fn badges_and_disabled_slots_read_in_every_theme() {
    let mut failures = Vec::new();
    for t in BuiltinTheme::ALL {
        let p = builtin(t);
        let badge = ratio(p.text, p.raised);
        if badge < 4.5 {
            failures.push(format!("{t:?}: badge text on raised {badge:.2}:1"));
        }
        // Disabled controls are exempt from the text floors, but the icon must still read as a mark.
        let icon = ratio(over(p.text.gamma_multiply(DISABLED_ALPHA), p.panel), p.panel);
        if icon < 3.0 {
            failures.push(format!("{t:?}: disabled icon on panel {icon:.2}:1"));
        }
        let enabled = ratio(p.text, p.panel);
        if icon >= enabled {
            failures.push(format!("{t:?}: the disabled icon is no weaker than the enabled one"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn a_badge_stands_out_at_rest_and_lit() {
    let mut failures = Vec::new();
    for t in BuiltinTheme::ALL {
        let p = builtin(t);
        let ground = p.panel;
        for press in [Press::Rest, Press::Hover, Press::Pressed, Press::Latched] {
            let look = badge_look(p, ground, press);
            // What the badge's fill sits against: the ring, in the slot's colour in this state.
            let under = match press {
                Press::Rest => ground,
                Press::Hover => p.hover_on(ground),
                Press::Pressed => p.pressed_on(ground),
                Press::Latched => p.latched_on(ground),
            };
            let shape = ratio(look.fill, under).max(look.edge.map_or(0.0, |e| ratio(e, under)));
            if shape < 1.2 {
                failures.push(format!("{t:?} {press:?}: the badge on its ring {shape:.2}:1"));
            }
            let text = ratio(look.ink, look.fill);
            if text < 4.5 {
                failures.push(format!("{t:?} {press:?}: badge text {text:.2}:1"));
            }
        }
        let edged = p.high_contrast || !p.dark;
        assert_eq!(badge_look(p, ground, Press::Rest).edge.is_some(), edged, "{t:?}: an edge in light and high contrast only");
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
