//! The sizes the window's drawers were last dragged to. `None` is "never dragged": the app
//! then sizes the drawer from the window. The app clamps every value to the drawer's limits and
//! to the window when it uses it, so a hand-edited or outgrown value is repaired, not trusted.

use serde::{Deserialize, Serialize};

/// A drawer whose dragged size is remembered here (the terminal's height is in `TerminalPrefs`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum SizedPanel {
    /// The left panel of refs (width).
    Sidebar,
    /// The right panel of commit details and staging (width).
    Details,
    /// The logs panel at the bottom (height).
    Activity,
}

/// Dragged drawer sizes in px.
#[derive(Clone, Copy, PartialEq, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PanelSizes {
    pub sidebar: Option<f32>,
    pub details: Option<f32>,
    pub activity: Option<f32>,
}

impl PanelSizes {
    /// The remembered size, `None` when never dragged or not a number.
    pub fn get(&self, p: SizedPanel) -> Option<f32> {
        let v = match p {
            SizedPanel::Sidebar => self.sidebar,
            SizedPanel::Details => self.details,
            SizedPanel::Activity => self.activity,
        };
        v.filter(|v| v.is_finite())
    }

    /// Stores `v` (already within the drawer's limits); returns whether it changed.
    pub fn set(&mut self, p: SizedPanel, v: f32) -> bool {
        let slot = match p {
            SizedPanel::Sidebar => &mut self.sidebar,
            SizedPanel::Details => &mut self.details,
            SizedPanel::Activity => &mut self.activity,
        };
        let changed = *slot != Some(v);
        *slot = Some(v);
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_files_and_partial_tables_load() {
        let s: PanelSizes = serde_json::from_str("{}").unwrap();
        assert_eq!(s, PanelSizes::default());
        let s: PanelSizes = serde_json::from_str(r#"{"details": 420.0}"#).unwrap();
        assert_eq!((s.get(SizedPanel::Details), s.get(SizedPanel::Sidebar)), (Some(420.0), None));
    }

    #[test]
    fn set_reports_a_change_once() {
        let mut s = PanelSizes::default();
        assert!(s.set(SizedPanel::Activity, 180.0));
        assert!(!s.set(SizedPanel::Activity, 180.0));
        assert_eq!(s.get(SizedPanel::Activity), Some(180.0));
    }

    #[test]
    fn sizes_survive_a_save_and_load() {
        let mut s = crate::settings::Settings::default();
        s.panel_sizes.set(SizedPanel::Sidebar, 312.0);
        s.panel_sizes.set(SizedPanel::Details, 444.0);
        let dir = crate::paths::temp_dir().join(format!("trellis-settings-panels-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("settings.json");
        s.save_to(&file).unwrap();
        let (back, problem) = crate::settings::Settings::load_from(&file);
        assert!(problem.is_none(), "{problem:?}");
        let sizes = back.panel_sizes;
        assert_eq!(sizes.get(SizedPanel::Sidebar), Some(312.0));
        assert_eq!(sizes.get(SizedPanel::Details), Some(444.0));
        assert_eq!(sizes.get(SizedPanel::Activity), None, "never dragged");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
