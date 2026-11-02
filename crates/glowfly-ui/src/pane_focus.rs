//! The panes the pane keys and F6 move the keyboard focus between (`state::layout::Pane`): the
//! focus targets of the graph, the left panel and the other central views, each with its own
//! `Id`; where each pane was drawn last frame, so the pane holding the focus is known whatever
//! widget in it has the focus; and the view taking the pane `update` asked for
//! (`ViewState::pane_focus`). The file lists' targets are `list_focus.rs`'s (the first one the
//! details panel draws takes the focus) and the commit message's is its summary field.

use crate::state::layout::{self, Pane, Panel};
use crate::state::{AppState, Selection};
use crate::ui::theme::tokens;
use eframe::egui::{self, Id, Rect, Sense, WidgetInfo, WidgetType};

/// The graph's focus target (`graph/select.rs`).
pub(crate) fn graph_id() -> Id {
    Id::new("pane-focus-graph")
}

/// The left panel's focus target.
fn sidebar_id() -> Id {
    Id::new("pane-focus-sidebar")
}

/// The focus target of a central view other than the graph (a diff, a pull request, the
/// rebase panel, a conflict).
fn central_id() -> Id {
    Id::new("pane-focus-central")
}

fn rect_id(p: Pane) -> Id {
    Id::new(("pane-focus-rect", p as u8))
}

/// Whether one of the panes' own focus targets (not a list's or a field's) has the focus: they
/// type nothing, so the shortcuts don't count them as typing.
pub(crate) fn target_focused(ctx: &egui::Context) -> bool {
    ctx.memory(|m| m.focused()).is_some_and(|f| [graph_id(), sidebar_id(), central_id()].contains(&f))
}

/// Whether the graph's selection shows as focused: the graph has the focus, or nothing has
/// (then the graph's keys reach it).
pub(crate) fn graph_active(ctx: &egui::Context) -> bool {
    ctx.memory(|m| m.focused()).is_none_or(|f| f == graph_id())
}

/// Notes that `pane` was drawn in `rect` this frame.
pub(crate) fn drawn_at(ctx: &egui::Context, pane: Pane, rect: Rect) {
    ctx.data_mut(|d| d.insert_temp(rect_id(pane), rect));
}

/// The pane holding the widget with the keyboard focus, from where each was drawn last.
pub(crate) fn focused_pane(ctx: &egui::Context) -> Option<Pane> {
    let id = ctx.memory(|m| m.focused())?;
    if id == graph_id() || id == central_id() {
        return Some(Pane::Central);
    }
    if id == sidebar_id() {
        return Some(Pane::Sidebar);
    }
    let at = ctx.read_response(id)?.rect.center();
    // The commit message sits inside the details panel: it is looked for first.
    [Pane::Message, Pane::Files, Pane::Sidebar, Pane::Central]
        .into_iter()
        .find(|&p| ctx.data(|d| d.get_temp::<Rect>(rect_id(p))).is_some_and(|r| r.contains(at)))
}

/// Whether F6 can go to `pane` now: the left panel when shown, the file list when the details
/// panel shows one, the commit message when it shows the working tree's changes.
pub(crate) fn drawn(st: &AppState, pane: Pane) -> bool {
    let details = layout::shown(&st.settings, Panel::Details);
    match pane {
        Pane::Sidebar => layout::shown(&st.settings, Panel::Left),
        Pane::Central => true,
        Pane::Files => details && st.session.selection != Selection::None,
        Pane::Message => details && st.session.selection == Selection::Wip,
        Pane::Diff => st.diff_or_download(),
    }
}

/// Takes the request to focus one of `panes`, if `update` made one.
pub(crate) fn take(st: &mut AppState, panes: &[Pane]) -> bool {
    let asked = st.view.pane_focus.is_some_and(|p| panes.contains(&p));
    if asked {
        st.view.pane_focus = None;
    }
    asked
}

/// A pane's focus target as drawn this frame; `ring` it after the pane's contents.
pub(crate) struct Target {
    rect: Rect,
    focused: bool,
}

impl Target {
    /// While the target has the focus, rings the pane inside its edge (over its contents).
    pub(crate) fn ring(&self, ui: &egui::Ui) {
        if self.focused {
            tokens::focus_ring_inset(ui.painter(), self.rect, 0);
        }
    }
}

/// The left panel's focus target over `rect`, named `name`, given the focus when asked for.
/// Registered before the panel's widgets, so Tab goes on from it into the panel.
pub(crate) fn sidebar(ui: &mut egui::Ui, st: &mut AppState, rect: Rect, name: &str) -> Target {
    target(ui, sidebar_id(), rect, name, take(st, &[Pane::Sidebar]))
}

/// The focus target of the central view drawn over `rect` (not the graph), named `name`.
pub(crate) fn central(ui: &mut egui::Ui, st: &mut AppState, rect: Rect, name: &str) -> Target {
    target(ui, central_id(), rect, name, take(st, &[Pane::Central, Pane::Diff]))
}

/// A pane's focus target: it senses no clicks (they go to the pane's widgets).
fn target(ui: &mut egui::Ui, id: Id, rect: Rect, name: &str, focus: bool) -> Target {
    let r = ui.interact(rect, id, Sense::focusable_noninteractive());
    r.widget_info(|| WidgetInfo::labeled(WidgetType::Other, true, name));
    if focus {
        r.request_focus();
    }
    Target { rect, focused: r.has_focus() }
}

fn files_wanted_id() -> Id {
    Id::new("pane-focus-files")
}

fn files_empty_id() -> Id {
    Id::new("pane-focus-files-empty")
}

/// Before the details panel's contents: when the file list was asked for, the first list
/// target drawn with rows takes the focus (`list_focus::target`), or with none, the first list.
pub(crate) fn begin_details(ctx: &egui::Context, st: &mut AppState) {
    let want = take(st, &[Pane::Files]);
    ctx.data_mut(|d| {
        d.insert_temp(files_wanted_id(), want);
        d.remove_temp::<Option<Id>>(files_empty_id());
    });
}

/// Whether the list target `id` being drawn should take the focus (once): the first with rows.
/// The first one without is kept, for when no list has rows.
pub(crate) fn list_wanted(ctx: &egui::Context, id: Id, has_rows: bool) -> bool {
    ctx.data_mut(|d| {
        if !d.get_temp::<bool>(files_wanted_id()).unwrap_or(false) {
            return false;
        }
        if has_rows {
            d.remove_temp::<bool>(files_wanted_id());
        } else if d.get_temp::<Option<Id>>(files_empty_id()).flatten().is_none() {
            d.insert_temp(files_empty_id(), Some(id));
        }
        has_rows
    })
}

/// After the details panel: when no list had rows, the first one takes the focus (on the WIP
/// panel, Unstaged Files).
pub(crate) fn end_details(ctx: &egui::Context) {
    let (wanted, empty) = ctx
        .data_mut(|d| (d.remove_temp::<bool>(files_wanted_id()).unwrap_or(false), d.remove_temp::<Option<Id>>(files_empty_id()).flatten()));
    if let Some(id) = empty.filter(|_| wanted) {
        ctx.memory_mut(|m| m.request_focus(id));
    }
}

fn last_id() -> Id {
    Id::new("pane-focus-last")
}

/// At the end of a frame: notes the widget with the focus, for Esc on the next frame (egui
/// lets go of the focus on Esc before the views run).
pub(crate) fn end_frame(ctx: &egui::Context) {
    let focused = ctx.memory(|m| m.focused());
    ctx.data_mut(|d| d.insert_temp(last_id(), focused));
}

/// Esc on a focused control that isn't a text field (`was_typing` says whether it was one),
/// with the graph shown: the graph takes the focus back, and that is all the Esc does.
pub(crate) fn escape_to_graph(ctx: &egui::Context, st: &AppState) -> bool {
    let last = ctx.data(|d| d.get_temp::<Option<Id>>(last_id())).flatten();
    // Preferences and the tour keep their own Esc (revert the page, close; close the tour).
    let covered = st.prefs.is_some() || st.onboarding.tour.is_some();
    let back = last.is_some_and(|id| id != graph_id()) && !st.view.was_typing && !covered && st.graph_shown();
    if back {
        ctx.memory_mut(|m| m.request_focus(graph_id()));
    }
    back
}
