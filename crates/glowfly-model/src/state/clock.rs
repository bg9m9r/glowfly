//! What dates are formatted with: the settings, and now and the zone from st.cues (no clock is read in a view or a pure rule). Views use it through ui's date_tip.

use super::AppState;
use glowfly_core::i18n::Lang;
use glowfly_core::settings::Settings;

/// The settings plus `now` and the UTC offset, copied from `st.cues`.
#[derive(Clone, Copy)]
pub struct Clock<'a> {
    pub settings: &'a Settings,
    pub now: i64,
    pub offset: i32,
}

impl<'a> Clock<'a> {
    pub fn of(st: &'a AppState) -> Self {
        Self { settings: &st.settings, now: st.cues.now, offset: st.cues.utc_offset }
    }

    pub fn lang(&self) -> Lang {
        self.settings.lang()
    }

    /// `secs` as a day: relative or in the user's Date format (`Settings::format_day`).
    pub fn day(&self, secs: i64) -> String {
        self.settings.format_day(secs, self.now, self.offset)
    }

    /// The full date (and the age when dates aren't relative), for a tooltip.
    pub fn tip(&self, secs: i64) -> String {
        self.settings.date_tooltip(secs, self.now, self.offset)
    }
}
