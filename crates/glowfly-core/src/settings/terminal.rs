//! Preferences > Terminal (the shell, text size, colours, cursor and dimming, copy on select,
//! middle-click paste, and whether Open in terminal uses the panel), and the panel's remembered
//! state.

use serde::{Deserialize, Serialize};
use std::ops::RangeInclusive;

/// Which shell new terminals start.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShellChoice {
    /// PowerShell 7, else Windows PowerShell, else COMSPEC on Windows; $SHELL, else /bin/sh,
    /// elsewhere.
    #[default]
    Auto,
    Pwsh,
    /// Windows only; elsewhere it acts as Auto.
    WindowsPowerShell,
    /// Windows only; elsewhere it acts as Auto.
    Cmd,
    /// Git Bash on Windows, bash from PATH elsewhere.
    Bash,
    /// `ShellSetting::program` with `ShellSetting::args`.
    Custom,
}

impl ShellChoice {
    /// The choices Preferences offers on this kind of system, in order.
    pub fn offered(windows: bool) -> &'static [ShellChoice] {
        use ShellChoice::*;
        if windows { &[Auto, Pwsh, WindowsPowerShell, Cmd, Bash, Custom] } else { &[Auto, Pwsh, Bash, Custom] }
    }
}

/// The shell choice, with the program and arguments of a custom one.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShellSetting {
    pub choice: ShellChoice,
    /// A program name on PATH or a path (Custom only).
    pub program: String,
    /// Arguments, split like the External Tools templates (Custom only).
    pub args: String,
}

/// How the terminal draws its cursor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CursorStyle {
    #[default]
    Block,
    Underline,
    Bar,
}

/// The colours the built-in terminal draws in.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalColours {
    /// The app theme's own terminal colours (a theme file's `terminal_*` keys, or its base's).
    #[default]
    MatchTheme,
    /// A built-in theme's terminal colours, by its id (`BuiltinTheme::id`); an unknown id
    /// matches the app theme.
    Theme(String),
    /// An imported scheme in the themes folder's `terminal` folder, by its file stem; one that
    /// isn't there matches the app theme.
    Scheme(String),
}

/// The terminal's text size when nothing else is set.
pub const DEFAULT_TERMINAL_FONT_SIZE: f32 = 12.5;
/// The terminal text sizes Preferences offers.
pub const TERMINAL_FONT_SIZES: RangeInclusive<f32> = 8.0..=24.0;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TerminalPrefs {
    /// Applies to terminals started after it changes.
    pub shell: ShellSetting,
    /// Text size in points; read through `font_px()`.
    pub font_size: f32,
    pub cursor: CursorStyle,
    /// Dim the screen while it doesn't have the keyboard.
    pub dim_unfocused: bool,
    /// Releasing a mouse selection copies it, and the copy key then goes to the shell.
    pub copy_on_select: bool,
    /// A middle click on the screen pastes the clipboard; read through `middle_click_pastes()`,
    /// since it acts only on Linux.
    pub middle_click_paste: bool,
    /// Open in terminal (palette, toolbar right-click) uses the panel.
    pub open_in_panel: bool,
    pub colours: TerminalColours,
    /// Window state, not on the page: whether the panel was open, and its height in points.
    pub panel_open: bool,
    pub panel_height: Option<f32>,
}

impl Default for TerminalPrefs {
    fn default() -> Self {
        Self {
            shell: ShellSetting::default(),
            font_size: DEFAULT_TERMINAL_FONT_SIZE,
            cursor: CursorStyle::Block,
            dim_unfocused: false,
            copy_on_select: false,
            middle_click_paste: true,
            open_in_panel: false,
            colours: TerminalColours::MatchTheme,
            panel_open: false,
            panel_height: None,
        }
    }
}

impl TerminalPrefs {
    /// The text size, kept within `TERMINAL_FONT_SIZES` (the default for a non-number from a
    /// hand-edited file).
    pub fn font_px(&self) -> f32 {
        if !self.font_size.is_finite() {
            return DEFAULT_TERMINAL_FONT_SIZE;
        }
        self.font_size.clamp(*TERMINAL_FONT_SIZES.start(), *TERMINAL_FONT_SIZES.end())
    }

    /// Whether a middle click pastes on a system where `linux` says whether it is Linux.
    pub fn middle_click_pastes_for(&self, linux: bool) -> bool {
        linux && self.middle_click_paste
    }

    /// Whether a middle click pastes on this system.
    pub fn middle_click_pastes(&self) -> bool {
        self.middle_click_pastes_for(cfg!(target_os = "linux"))
    }

    /// Everything on the Preferences page back to its default; the panel's state stays.
    pub fn reset_page(&mut self) {
        *self = Self { panel_open: self.panel_open, panel_height: self.panel_height, ..Self::default() };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;

    #[test]
    fn old_files_load_the_defaults() {
        let s: Settings = serde_json::from_str(r#"{"auto_fetch_minutes": 5}"#).unwrap();
        assert_eq!(s.terminal, TerminalPrefs::default());
        assert_eq!(s.terminal.font_size, 12.5);
        assert_eq!(s.terminal.shell.choice, ShellChoice::Auto);
        assert!(!s.terminal.copy_on_select);
        assert!(s.terminal.middle_click_paste);
        assert_eq!(s.terminal.colours, TerminalColours::MatchTheme);
        let p: TerminalPrefs = serde_json::from_str(r#"{"font_size":14.0,"dim_unfocused":true}"#).unwrap();
        assert!(!p.copy_on_select);
        assert!(p.middle_click_paste);
    }

    #[test]
    fn middle_click_pastes_only_on_linux() {
        let on = TerminalPrefs::default();
        let off = TerminalPrefs { middle_click_paste: false, ..Default::default() };
        assert!(on.middle_click_pastes_for(true));
        assert!(!off.middle_click_pastes_for(true));
        assert!(!on.middle_click_pastes_for(false));
        assert!(!off.middle_click_pastes_for(false));
        assert_eq!(on.middle_click_pastes(), cfg!(target_os = "linux"));
    }

    #[test]
    fn round_trip_and_partial_files() {
        let t = TerminalPrefs {
            shell: ShellSetting { choice: ShellChoice::Custom, program: "nu".into(), args: "-l \"a b\"".into() },
            font_size: 15.0,
            cursor: CursorStyle::Underline,
            dim_unfocused: true,
            copy_on_select: true,
            middle_click_paste: false,
            open_in_panel: true,
            colours: TerminalColours::Scheme("campbell".into()),
            panel_open: true,
            panel_height: Some(301.0),
        };
        let s = Settings { terminal: t.clone(), ..Settings::default() };
        let back: Settings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back.terminal, t);

        let p: TerminalPrefs = serde_json::from_str(r#"{"cursor":"Bar"}"#).unwrap();
        assert_eq!(p, TerminalPrefs { cursor: CursorStyle::Bar, ..TerminalPrefs::default() });
        let p: TerminalPrefs = serde_json::from_str(r#"{"colours":{"Theme":"sand"}}"#).unwrap();
        assert_eq!(p.colours, TerminalColours::Theme("sand".into()));
        let p: TerminalPrefs = serde_json::from_str(r#"{"shell":{"choice":"Bash"}}"#).unwrap();
        assert_eq!(p.shell, ShellSetting { choice: ShellChoice::Bash, ..Default::default() });
    }

    #[test]
    fn font_px_clamps() {
        let px = |font_size| TerminalPrefs { font_size, ..Default::default() }.font_px();
        assert_eq!(px(4.0), 8.0);
        assert_eq!(px(99.0), 24.0);
        assert_eq!(px(f32::NAN), 12.5);
        assert_eq!(px(14.0), 14.0);
    }

    #[test]
    fn reset_page_keeps_the_panel_state() {
        let mut t = TerminalPrefs {
            font_size: 20.0,
            dim_unfocused: true,
            copy_on_select: true,
            middle_click_paste: false,
            open_in_panel: true,
            colours: TerminalColours::Theme("sand".into()),
            panel_open: true,
            panel_height: Some(250.0),
            ..Default::default()
        };
        t.reset_page();
        assert_eq!(t, TerminalPrefs { panel_open: true, panel_height: Some(250.0), ..Default::default() });
    }

    #[test]
    fn offered_choices_per_platform() {
        assert_eq!(ShellChoice::offered(true).len(), 6);
        assert_eq!(ShellChoice::offered(false), [ShellChoice::Auto, ShellChoice::Pwsh, ShellChoice::Bash, ShellChoice::Custom]);
    }
}
