//! The tour of the main window: its five steps, the panel each one points at, and their texts
//! (`tour.*` keys).
//! The tour's place lives in `Onboarding` (this session only, never saved).

use super::layout::Panel;
use crate::keymap::{BindingId, Keymap};
use glowfly_core::i18n::Lang;

/// The tour's steps, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TourStep {
    Toolbar,
    LeftPanel,
    Graph,
    Details,
    Palette,
}

impl TourStep {
    pub const ALL: [TourStep; 5] = [TourStep::Toolbar, TourStep::LeftPanel, TourStep::Graph, TourStep::Details, TourStep::Palette];

    /// Its place, from 1.
    pub fn number(self) -> usize {
        Self::ALL.iter().position(|s| *s == self).unwrap_or(0) + 1
    }

    pub fn next(self) -> Option<TourStep> {
        Self::ALL.get(self.number()).copied()
    }

    pub fn prev(self) -> Option<TourStep> {
        self.number().checked_sub(2).map(|i| Self::ALL[i])
    }

    /// The panel it points at, for the steps on a panel that can be hidden.
    pub fn panel(self) -> Option<Panel> {
        match self {
            TourStep::Toolbar => Some(Panel::Toolbar),
            TourStep::LeftPanel => Some(Panel::Left),
            TourStep::Details => Some(Panel::Details),
            TourStep::Graph | TourStep::Palette => None,
        }
    }

    /// The card's heading.
    pub fn title(self, t: Lang) -> &'static str {
        match self {
            TourStep::Toolbar => t.tr("tour.step.toolbar.title"),
            TourStep::LeftPanel => t.tr("tour.step.left.title"),
            TourStep::Graph => t.tr("tour.step.graph.title"),
            TourStep::Details => t.tr("tour.step.details.title"),
            TourStep::Palette => t.tr("tour.step.palette.title"),
        }
    }

    /// The card's sentences; the last step names the current keys from `keys`.
    pub fn body(self, keys: &Keymap, t: Lang) -> String {
        match self {
            TourStep::Toolbar => t.tr("tour.step.toolbar.body").into(),
            TourStep::LeftPanel => t.tr("tour.step.left.body").into(),
            TourStep::Graph => t.tr("tour.step.graph.body").into(),
            TourStep::Details => t.tr("tour.step.details.body").into(),
            TourStep::Palette => {
                let open = match keys.hint(BindingId::Palette) {
                    Some(k) => t.trf("tour.step.palette.open.key", &[("key", &k)]),
                    None => t.tr("tour.step.palette.open.click").to_owned(),
                };
                let sheet = match keys.hint(BindingId::Sheet) {
                    Some(k) => t.trf("tour.step.palette.sheet.key", &[("key", &k)]),
                    None => t.tr("tour.step.palette.sheet.palette").to_owned(),
                };
                format!("{open} {sheet}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn steps_in_order() {
        assert_eq!(TourStep::Toolbar.prev(), None);
        assert_eq!(TourStep::Toolbar.next(), Some(TourStep::LeftPanel));
        assert_eq!(TourStep::Palette.next(), None);
        assert_eq!(TourStep::Palette.prev(), Some(TourStep::Details));
        assert_eq!(TourStep::ALL.map(TourStep::number), [1, 2, 3, 4, 5]);
    }

    #[test]
    fn the_last_step_names_the_current_keys() {
        let (keys, _) = Keymap::from_overrides(&BTreeMap::new());
        let body = TourStep::Palette.body(&keys, Lang::En);
        let (palette, sheet) = (keys.hint(BindingId::Palette).unwrap(), keys.hint(BindingId::Sheet).unwrap());
        assert!(body.contains(&format!("Press {palette}")) && body.contains(&format!("{sheet} lists")), "{body}");
        let unbound = BTreeMap::from([("palette".to_owned(), Vec::new()), ("shortcut-sheet".to_owned(), Vec::new())]);
        let (keys, _) = Keymap::from_overrides(&unbound);
        let body = TourStep::Palette.body(&keys, Lang::En);
        assert!(body.starts_with("Click Commands") && body.contains("Its Keyboard shortcuts entry"), "{body}");
        let german = TourStep::Palette.body(&keys, Lang::De);
        assert!(german.starts_with(Lang::De.tr("tour.step.palette.open.click")), "{german}");
    }

    #[test]
    fn every_step_follows_the_language() {
        let (keys, _) = Keymap::from_overrides(&BTreeMap::new());
        for step in TourStep::ALL {
            assert_ne!(step.title(Lang::En), step.title(Lang::De), "{step:?}");
            assert_ne!(step.body(&keys, Lang::En), step.body(&keys, Lang::Es), "{step:?}");
        }
    }
}
