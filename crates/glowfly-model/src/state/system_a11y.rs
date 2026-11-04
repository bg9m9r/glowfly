//! The system's accessibility settings as the shell last read them (`platform/a11y_system.rs` in
//! the app: at start, when the system reports a settings change and when the window regains the
//! focus), and the values in effect once Preferences > Accessibility's own choices are added. A
//! system setting only ever adds to a choice made in Preferences: Reduce motion or a
//! high-contrast theme chosen there stays whatever the system says, and the chosen theme is never
//! rewritten (the shell shows a high-contrast one in its place while the system asks for it).

use crate::state::AppState;
use glowfly_core::settings::snap_zoom;

/// What the system asks for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SystemA11y {
    /// Less motion (Windows: animations off; macOS: Reduce motion; GNOME: animations off).
    pub reduce_motion: bool,
    /// A high-contrast mode (Windows: a contrast theme; macOS: Increase contrast; GNOME: High
    /// contrast).
    pub high_contrast: bool,
    /// The text size as a factor, 1.0 where the system has none or leaves it alone.
    pub text_scale: f32,
    /// The system has a text size to follow (not macOS): Preferences > Accessibility shows
    /// "Also follow the system's text size" only then.
    pub text_size_followable: bool,
}

impl Default for SystemA11y {
    fn default() -> Self {
        Self { reduce_motion: false, high_contrast: false, text_scale: 1.0, text_size_followable: true }
    }
}

impl AppState {
    /// Whether motion is reduced: chosen in Preferences, or asked for by the system while
    /// Preferences follows it.
    pub fn motion_reduced(&self) -> bool {
        self.settings.reduce_motion || (self.settings.a11y.follow_system_motion && self.cues.system_a11y.reduce_motion)
    }

    /// Whether a high-contrast theme shows in place of a chosen theme that isn't one, because
    /// the system uses high contrast and Preferences follows it.
    pub fn system_contrast(&self) -> bool {
        self.settings.a11y.follow_system_contrast && self.cues.system_a11y.high_contrast && !self.settings.theme.is_high_contrast()
    }

    /// The zoom in effect as a factor: the interface size, times the system's text size while
    /// Preferences follows it, snapped to a zoom step.
    pub fn zoom_factor(&self) -> f32 {
        let scale = self.cues.system_a11y.text_scale;
        if !self.settings.a11y.follow_system_text_size || !scale.is_finite() || scale <= 0.0 || scale == 1.0 {
            return self.settings.zoom_factor();
        }
        let scaled = (f32::from(self.settings.zoom_level()) * scale).round().clamp(0.0, f32::from(u16::MAX));
        f32::from(snap_zoom(scaled as u16)) / 100.0
    }
}
