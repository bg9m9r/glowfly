//! The window's zoom and which of its panels are drawn: the left panel of refs, the commit
//! details panel and the toolbar. All four live in `Settings`; the shell reads them each frame.

use super::AppState;
use super::palette_commands::{Entry, PaletteCommand, entry, with_detail};
use crate::keymap::BindingId;
use crate::msg::LayoutMsg;
use trellis_core::settings::{DEFAULT_ZOOM_PERCENT, Settings, ZOOM_STEPS};

/// A panel that can be hidden.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Panel {
    /// The left panel of branches, remotes and other refs.
    Left,
    /// The right panel with WIP, commit and multi-commit details.
    Details,
    /// The toolbar at the top of the window.
    Toolbar,
}

impl Panel {
    pub const ALL: [Panel; 3] = [Panel::Left, Panel::Details, Panel::Toolbar];

    /// Its name in labels ("Hide <name>").
    pub fn name(self) -> &'static str {
        match self {
            Panel::Left => "left panel",
            Panel::Details => "commit details panel",
            Panel::Toolbar => "toolbar",
        }
    }

    /// The key that shows or hides it.
    pub fn binding(self) -> BindingId {
        match self {
            Panel::Left => BindingId::ToggleLeftPanel,
            Panel::Details => BindingId::ToggleDetailsPanel,
            Panel::Toolbar => BindingId::ToggleToolbar,
        }
    }

    fn keywords(self) -> &'static str {
        match self {
            Panel::Left => "sidebar branches refs left panel toggle collapse",
            Panel::Details => "side panel details commit box wip staging toggle",
            Panel::Toolbar => "toolbar top bar buttons toggle",
        }
    }
}

/// Whether `p` is drawn.
pub fn shown(s: &Settings, p: Panel) -> bool {
    match p {
        Panel::Left => s.show_left_panel,
        Panel::Details => s.show_details_panel,
        Panel::Toolbar => s.show_toolbar,
    }
}

/// Shows or hides `p`; returns whether that changed anything.
pub fn set_shown(s: &mut Settings, p: Panel, on: bool) -> bool {
    let field = match p {
        Panel::Left => &mut s.show_left_panel,
        Panel::Details => &mut s.show_details_panel,
        Panel::Toolbar => &mut s.show_toolbar,
    };
    let changed = *field != on;
    *field = on;
    changed
}

/// The toast after a zoom step, e.g. "Zoom 125%", noting when a limit is reached.
pub fn zoom_toast_text(percent: u16) -> String {
    let limit = if percent >= ZOOM_STEPS[ZOOM_STEPS.len() - 1] {
        " (the largest)"
    } else if percent <= ZOOM_STEPS[0] {
        " (the smallest)"
    } else {
        ""
    };
    format!("{}{limit}", zoom_chip_text(percent))
}

/// The status bar's reset button while zoomed, e.g. "Zoom 125%".
pub fn zoom_chip_text(percent: u16) -> String {
    format!("Zoom {percent}%")
}

/// Zoom in, Zoom out and Reset zoom (each while it would change something; the detail is the
/// level now), and Hide or Show for each panel.
pub(super) fn palette_entries(st: &AppState, out: &mut Vec<Entry>) {
    let lvl = st.settings.zoom_level();
    let now = || Some(format!("now {lvl}%"));
    let zooms = [
        (LayoutMsg::ZoomIn, "Zoom in", "zoom larger bigger magnify scale size increase", lvl < ZOOM_STEPS[ZOOM_STEPS.len() - 1]),
        (LayoutMsg::ZoomOut, "Zoom out", "zoom smaller shrink scale size decrease", lvl > ZOOM_STEPS[0]),
        (LayoutMsg::ZoomReset, "Reset zoom to 100%", "zoom actual size default 100 scale", lvl != DEFAULT_ZOOM_PERCENT),
    ];
    for (msg, label, keywords, offered) in zooms {
        if offered {
            out.push(with_detail(entry(PaletteCommand::Layout(msg), label, keywords), now()));
        }
    }
    for panel in Panel::ALL {
        let on = shown(&st.settings, panel);
        let label = format!("{} {}", if on { "Hide" } else { "Show" }, panel.name());
        out.push(entry(PaletteCommand::Layout(LayoutMsg::SetPanel { panel, shown: !on }), label, panel.keywords()));
    }
}
