//! The selection of a pane without the keyboard focus (`tokens::selected_inactive`): a neutral
//! (OKLCH chroma at most 0.02) on which `weak` text reads at 4.5:1, in every built-in theme.

use super::builtin;
use super::contrast::{brownish, oklch, ratio};
use super::tokens::selected_inactive;
use glowfly_core::settings::BuiltinTheme;

/// The most OKLCH chroma the inactive selection may have (the design spec's neutral).
const INACTIVE_CHROMA: f64 = 0.02;

#[test]
fn inactive_selection_is_neutral_and_keeps_weak_text_readable() {
    let mut failures = Vec::new();
    for t in BuiltinTheme::ALL {
        let p = builtin(t);
        let fill = selected_inactive(p);
        let (l, c, _) = oklch(fill);
        let weak = ratio(p.weak, fill);
        eprintln!(
            "{t:?}: #{:02x}{:02x}{:02x} (L {l:.3}, C {c:.3}), weak {weak:.2}:1, strong {:.2}:1",
            fill.r(),
            fill.g(),
            fill.b(),
            ratio(p.strong, fill)
        );
        if c > INACTIVE_CHROMA || weak < 4.5 || brownish(fill) || fill.a() != 255 {
            failures.push(format!("{t:?}: chroma {c:.3}, weak {weak:.2}:1"));
        }
        assert_ne!(fill, p.selected, "{t:?}: the inactive selection differs from the focused one");
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
