//! The window's zoom and which of its panels are drawn: the left panel of refs, the commit
//! details panel, the toolbar and the menu bar. All live in `Settings`; the shell reads them
//! each frame. Full screen is the window's own state (`view.window`), never saved. The tool view open in the central area decides whether the commit details panel
//! is drawn in full or as a narrow rail (`details_shape`), which is view state, never saved.

use super::AppState;
use super::palette_commands::{Entry, PaletteCommand, entry, with_detail};
use crate::keymap::BindingId;
use crate::msg::LayoutMsg;
use glowfly_core::repo::Snapshot;
use glowfly_core::settings::{DEFAULT_ZOOM_PERCENT, Settings, ZOOM_STEPS};
use std::collections::HashSet;

/// A panel that can be hidden.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Panel {
    /// The left panel of branches, remotes and other refs.
    Left,
    /// The right panel with WIP, commit and multi-commit details.
    Details,
    /// The toolbar at the top of the window.
    Toolbar,
    /// The menu bar (File, Edit, View, Window, Help) above the tabs.
    MenuBar,
}

impl Panel {
    pub const ALL: [Panel; 4] = [Panel::Left, Panel::Details, Panel::Toolbar, Panel::MenuBar];

    /// Its name in labels ("Hide <name>").
    pub fn name(self) -> &'static str {
        match self {
            Panel::Left => "left panel",
            Panel::Details => "commit details panel",
            Panel::Toolbar => "toolbar",
            Panel::MenuBar => "menu bar",
        }
    }

    /// The key that shows or hides it; the menu bar has none.
    pub fn binding(self) -> Option<BindingId> {
        match self {
            Panel::Left => Some(BindingId::ToggleLeftPanel),
            Panel::Details => Some(BindingId::ToggleDetailsPanel),
            Panel::Toolbar => Some(BindingId::ToggleToolbar),
            Panel::MenuBar => None,
        }
    }

    fn keywords(self) -> &'static str {
        match self {
            Panel::Left => "sidebar branches refs left panel toggle collapse",
            Panel::Details => "side panel details commit box wip staging toggle",
            Panel::Toolbar => "toolbar top bar buttons toggle",
            Panel::MenuBar => "menu bar menus file edit view window help top toggle",
        }
    }
}

/// Whether `p` is drawn.
pub fn shown(s: &Settings, p: Panel) -> bool {
    match p {
        Panel::Left => s.show_left_panel,
        Panel::Details => s.show_details_panel,
        Panel::Toolbar => s.show_toolbar,
        Panel::MenuBar => s.show_menu_bar,
    }
}

/// Shows or hides `p`; returns whether that changed anything.
pub fn set_shown(s: &mut Settings, p: Panel, on: bool) -> bool {
    let field = match p {
        Panel::Left => &mut s.show_left_panel,
        Panel::Details => &mut s.show_details_panel,
        Panel::Toolbar => &mut s.show_toolbar,
        Panel::MenuBar => &mut s.show_menu_bar,
    };
    let changed = *field != on;
    *field = on;
    changed
}

/// A view filling the central area that needs room: the commit details panel collapses to a
/// rail while one is open.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ToolView {
    Conflict,
    Blame,
    FileHistory,
    FolderHistory,
    Issue,
    RangeDiff,
    Journal,
    GitFlow,
}

/// The tool view the central area draws now, if any. `ui::central::show` and
/// `AppState::graph_shown` follow this same order: keep the three in step.
pub fn tool_view(st: &AppState) -> Option<ToolView> {
    use super::history::HistoryKind;
    if st.dashboard.open || st.snap.is_none() {
        None
    } else if st.conflict.is_some() {
        Some(ToolView::Conflict)
    } else if st.blame.is_some() {
        Some(ToolView::Blame)
    } else if let Some(h) = &st.history {
        Some(match h.kind {
            HistoryKind::File => ToolView::FileHistory,
            HistoryKind::Folder => ToolView::FolderHistory,
        })
    } else if st.pull_request.is_some() {
        // The pull request panel covers the issue panel, and keeps the full commit panel.
        None
    } else if st.issue.is_some() {
        Some(ToolView::Issue)
    } else if st.diff_or_download() || st.rebase.is_some() {
        None
    } else if st.range_diff.is_some() {
        Some(ToolView::RangeDiff)
    } else if st.journal.is_some() {
        Some(ToolView::Journal)
    } else if st.gitflow_panel {
        Some(ToolView::GitFlow)
    } else {
        None
    }
}

/// How the commit details panel is drawn this frame.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DetailsShape {
    /// The resizable drawer.
    Full,
    /// A narrow rail (a tool view is open and the panel wasn't expanded in it).
    Rail,
    /// Not at all (the saved setting hides it): an edge strip brings it back.
    Hidden,
}

pub fn details_shape(st: &AppState) -> DetailsShape {
    if !st.settings.show_details_panel {
        return DetailsShape::Hidden;
    }
    match tool_view(st) {
        Some(v) if st.view.details_expanded_for != Some(v) => DetailsShape::Rail,
        _ => DetailsShape::Full,
    }
}

/// The working tree's changed files: distinct paths across the unstaged and staged lists, so a
/// file staged and then modified again counts once.
pub fn wip_change_count(snap: &Snapshot) -> usize {
    snap.unstaged.iter().chain(&snap.staged).map(|f| f.path.as_str()).collect::<HashSet<&str>>().len()
}

/// Forgets the commit panel's expansion once the tool view it was expanded in is gone or
/// another one replaced it (`update` calls it after every message).
pub fn drop_stale_expansion(st: &mut AppState) {
    if st.view.details_expanded_for.is_some() && st.view.details_expanded_for != tool_view(st) {
        st.view.details_expanded_for = None;
    }
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
/// level now), Enter or Leave full screen, and Hide or Show for each panel.
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
    let label = if st.view.window.fullscreen { "Leave full screen" } else { "Enter full screen" };
    let keywords = "fullscreen full screen window maximize kiosk presentation hide taskbar";
    out.push(entry(PaletteCommand::Layout(LayoutMsg::ToggleFullscreen), label, keywords));
    for panel in Panel::ALL {
        // A drawn title bar replaces the menu bar.
        if panel == Panel::MenuBar && st.view.chrome.drawn() {
            continue;
        }
        let on = shown(&st.settings, panel);
        let label = format!("{} {}", if on { "Hide" } else { "Show" }, panel.name());
        out.push(entry(PaletteCommand::Layout(LayoutMsg::SetPanel { panel, shown: !on }), label, panel.keywords()));
    }
}
