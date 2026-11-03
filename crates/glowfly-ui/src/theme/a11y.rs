//! What Preferences > Accessibility sets in egui's style, every frame and in both egui themes:
//! the animation time (none with Reduce motion), the tooltip timing (the Tooltip delay of
//! stillness; at once within `tooltip_motion::GRACE` of another tooltip) and whether the text
//! cursor in text fields blinks; and the focus rings' style (`tokens::set_focus_style`). A value is written only when it differs; the blink only when
//! the setting changed since the last frame (egui's own default is to blink, and the view
//! tests' harness turns it off, which a setting left on keeps).

use crate::ui::theme::tokens::{self, motion};
use crate::ui::widgets::tooltip_motion;
use eframe::egui;
use glowfly_core::settings::Settings;

/// Sets the animation time, the tooltip timing, the text cursor's blink and the focus style
/// from `s`.
pub fn configure(ctx: &egui::Context, s: &Settings) {
    if tokens::focus_style(ctx) != s.a11y.focus_style {
        tokens::set_focus_style(ctx, s.a11y.focus_style);
    }
    let animation = motion::animation_time(s.reduce_motion);
    let delay = s.a11y.tooltip_delay.seconds();
    let key = egui::Id::new("glowfly-cursor-blink");
    let blink = s.a11y.cursor_blink;
    let blink_changed = ctx.data(|d| d.get_temp::<bool>(key)).unwrap_or(true) != blink;
    ctx.data_mut(|d| d.insert_temp(key, blink));
    for theme in [egui::Theme::Dark, egui::Theme::Light] {
        let st = ctx.style_of(theme);
        let i = &st.interaction;
        let same = i.tooltip_delay == delay
            && i.tooltip_grace_time == tooltip_motion::GRACE
            && i.show_tooltips_only_when_still
            && st.animation_time == animation
            && !blink_changed;
        if !same {
            ctx.style_mut_of(theme, |st| {
                st.interaction.tooltip_delay = delay;
                st.interaction.tooltip_grace_time = tooltip_motion::GRACE;
                st.interaction.show_tooltips_only_when_still = true;
                st.animation_time = animation;
                if blink_changed {
                    st.visuals.text_cursor.blink = blink;
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glowfly_core::settings::{FocusStyle, TooltipDelay};

    #[test]
    fn configure_sets_the_timing_and_the_cursor_in_both_themes() {
        let ctx = egui::Context::default();
        let mut s = Settings::default();
        configure(&ctx, &s);
        for theme in [egui::Theme::Dark, egui::Theme::Light] {
            let st = ctx.style_of(theme);
            assert_eq!((st.interaction.tooltip_delay, st.interaction.tooltip_grace_time, st.animation_time), (0.4, 0.3, motion::STANDARD));
            assert!(st.interaction.show_tooltips_only_when_still && st.visuals.text_cursor.blink, "egui's default");
        }
        s.reduce_motion = true;
        s.a11y.cursor_blink = false;
        s.a11y.tooltip_delay = TooltipDelay::Slow;
        configure(&ctx, &s);
        for theme in [egui::Theme::Dark, egui::Theme::Light] {
            let st = ctx.style_of(theme);
            assert_eq!((st.animation_time, st.interaction.tooltip_delay), (0.0, 1.0), "reduced motion, slow tooltips");
            assert!(!st.visuals.text_cursor.blink);
        }
        s.a11y.cursor_blink = true;
        configure(&ctx, &s);
        assert!(ctx.style_of(egui::Theme::Light).visuals.text_cursor.blink, "on again");
        s.a11y.tooltip_delay = TooltipDelay::Instant;
        configure(&ctx, &s);
        assert_eq!(ctx.style_of(egui::Theme::Dark).interaction.tooltip_delay, 0.0);
    }

    #[test]
    fn configure_sets_the_focus_style() {
        let ctx = egui::Context::default();
        let mut s = Settings::default();
        configure(&ctx, &s);
        assert_eq!(tokens::focus_style(&ctx), FocusStyle::Standard);
        s.a11y.focus_style = FocusStyle::Strong;
        configure(&ctx, &s);
        assert_eq!(tokens::focus_style(&ctx), FocusStyle::Strong);
        s.a11y.focus_style = FocusStyle::Standard;
        configure(&ctx, &s);
        assert_eq!(tokens::focus_style(&ctx), FocusStyle::Standard, "and back");
    }
}
