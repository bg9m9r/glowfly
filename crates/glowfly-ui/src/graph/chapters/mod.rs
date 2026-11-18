//! The Chapters list, drawn in place of the graph's column header and rows: a row for the work
//! tree's changes, then the chapters of HEAD's first-parent history (with release headings and
//! the commits of the expanded ones) at the graph's row height, in a scroll area of their own,
//! with the graph's end row below the last one. Selection, the keyboard focus target and the
//! near-end load share the graph's (`select`, `more`); `keys` moves over the rows and `rows`
//! draws them.

mod keys;
mod rows;

use crate::empty_state::{EmptyIcon, empty_state};
use crate::pane_focus;
use eframe::egui;
use glowfly_core::repo::Snapshot;
use glowfly_core::settings::HistoryMode;
use glowfly_model::msg::{LoadMsg, Msg};
use glowfly_model::state::custom_commands::MenuCommands;
use glowfly_model::state::{AppState, ViewState};

/// Whether the chapters are shown instead of the graph: chosen for the repository, grouped, and
/// not while the search bar is open (search works on the commit list).
pub(super) fn shown(st: &AppState, view: &ViewState) -> bool {
    st.session.chapters.mode == HistoryMode::Chapters && st.session.chapters.list.is_some() && !view.show_search
}

/// The list; `want_focus` gives it the keyboard focus.
pub(super) fn show(ui: &mut egui::Ui, st: &AppState, snap: &Snapshot, view: &mut ViewState, want_focus: bool, msgs: &mut Vec<Msg>) {
    let Some(list) = st.session.chapters.list.as_ref() else { return };
    let t = st.settings.lang();
    let row_h = super::row_h(st.settings.graph_density);
    let wip = snap.graph.wip.is_some();
    let total = usize::from(wip) + st.session.chapters.rows.len();
    // With the line below the loaded commits (`open_end`) the end row says so and loads more.
    if total == 0 && !list.open_end {
        empty_state(ui, EmptyIcon::Commits, t.tr("chapters.empty.title"), t.tr("chapters.empty.hint"), &[]);
        return;
    }
    // Not under a dialog, the palette or a credential prompt, which keep their keys.
    let mut scroll_to = None;
    if st.modal.is_none() && st.palette.is_none() && !st.askpass.showing() {
        let (offset, height) = view.tab.chapters_scroll;
        scroll_to = keys::keyboard(ui, st, snap, wip, (offset, height, row_h), view, msgs);
    }
    ui.spacing_mut().item_spacing.y = 0.0;
    let area_rect = ui.available_rect_before_wrap();
    let page = ((area_rect.height() / row_h).floor() as usize).saturating_sub(1);
    super::select::target(ui, area_rect, t.tr("chapters.focus.name"), page, want_focus);
    let mut area = egui::ScrollArea::vertical().id_salt(("chapters", &snap.workdir)).auto_shrink([false, false]);
    if let Some(offset) = scroll_to {
        area = area.vertical_scroll_offset(offset);
    }
    let frame = rows::Frame {
        st,
        snap,
        list,
        rows: &st.session.chapters.rows,
        t,
        row_h,
        wip,
        total,
        active: pane_focus::graph_active(ui.ctx()),
        end: st.end_row(view),
        commands: MenuCommands::for_state(st),
        pointer: ui.ctx().pointer_hover_pos(),
    };
    let shown = total + usize::from(frame.end.is_some());
    let out = area.show_rows(ui, row_h, shown, |ui, range| {
        let last = range.end.min(total);
        for d in range {
            rows::draw(ui, &frame, d, msgs);
        }
        last
    });
    view.tab.chapters_scroll = (out.state.offset.y, out.inner_rect.height());
    // Once per row count (`TabViewState::chapters_asked`): rows, not commits, are what the list
    // scrolls over, so a load that adds none must not make it ask again by itself.
    if list.open_end && view.tab.chapters_asked != Some(total) && super::more::wants_more(st, out.inner, total, false) {
        view.tab.chapters_asked = Some(total);
        msgs.push(Msg::Load(LoadMsg::NearHistoryEnd));
    }
}
