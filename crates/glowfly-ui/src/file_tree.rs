//! The changed-file lists shown as a tree (`Settings::file_list_mode`): the rows a list draws
//! (its files, or folder rows and files grouped under them, `state::file_tree`), a folder row,
//! the header's List / Tree toggle and a file row's indent. The tree of a list is kept between
//! frames (`FrameCache`), so thousands of files aren't sorted again every frame.

use crate::msg::{LayoutMsg, Msg};
use crate::state::file_folds::FileListId;
use crate::state::file_tree::{self, TreeRow};
use crate::ui::theme::color;
use crate::ui::theme::sizes::ControlSize;
use crate::ui::theme::type_scale;
use crate::ui::widgets::row_menu::RowMenuExt;
use crate::ui::widgets::{self, Icon, buttons, icon};
use eframe::egui::util::cache::{ComputerMut, FrameCache};
use eframe::egui::{self, RichText, Sense, Ui, vec2};
use glowfly_core::i18n::Lang;
use glowfly_core::settings::FileListMode;
use std::collections::BTreeSet;
use std::sync::Arc;

/// How far each level of the tree is indented.
pub(crate) const TREE_INDENT: f32 = 16.0;

/// The rows a changed-file list draws, each `LIST_ROW_H` tall: its files in its own order
/// (List mode), or the tree's folder and file rows.
pub(crate) enum ListRows {
    Flat(usize),
    Tree(Arc<Vec<TreeRow>>),
}

/// One row of `ListRows`.
pub(crate) enum Shown<'a> {
    /// The list's file at `index`; `depth` is its level in the tree (None in List mode).
    File { index: usize, depth: Option<usize> },
    /// A folder row (always a `TreeRow::Folder`).
    Folder(&'a TreeRow),
}

impl ListRows {
    /// The rows of a list of `n` files in `mode`. In Tree mode `paths` (the files' paths in the
    /// list's order) are grouped, with the folders in `closed` collapsed; List mode builds
    /// nothing.
    pub(crate) fn of<'p>(
        ui: &Ui,
        mode: FileListMode,
        n: usize,
        paths: impl FnOnce() -> Vec<&'p str>,
        closed: Option<&BTreeSet<String>>,
    ) -> ListRows {
        match mode {
            FileListMode::List => ListRows::Flat(n),
            FileListMode::Tree => {
                let paths = paths();
                let key = TreeKey { paths: &paths, closed };
                ListRows::Tree(ui.ctx().memory_mut(|m| m.caches.cache::<FrameCache<Arc<Vec<TreeRow>>, TreeComputer>>().get(key)))
            }
        }
    }

    pub(crate) fn len(&self) -> usize {
        match self {
            ListRows::Flat(n) => *n,
            ListRows::Tree(rows) => rows.len(),
        }
    }

    pub(crate) fn get(&self, i: usize) -> Shown<'_> {
        match self {
            ListRows::Flat(_) => Shown::File { index: i, depth: None },
            ListRows::Tree(rows) => match &rows[i] {
                TreeRow::File { index, depth } => Shown::File { index: *index, depth: Some(*depth) },
                folder => Shown::Folder(folder),
            },
        }
    }

    /// The row showing the list's file `index`; None while its folder is collapsed.
    pub(crate) fn row_of_file(&self, index: usize) -> Option<usize> {
        match self {
            ListRows::Flat(n) => (index < *n).then_some(index),
            ListRows::Tree(rows) => rows.iter().position(|r| matches!(r, TreeRow::File { index: i, .. } if *i == index)),
        }
    }
}

/// What a list's tree is cached by: its paths and its collapsed folders.
#[derive(Clone, Copy, Hash)]
struct TreeKey<'a> {
    paths: &'a [&'a str],
    closed: Option<&'a BTreeSet<String>>,
}

#[derive(Default)]
struct TreeComputer;

impl<'a> ComputerMut<TreeKey<'a>, Arc<Vec<TreeRow>>> for TreeComputer {
    fn compute(&mut self, key: TreeKey<'a>) -> Arc<Vec<TreeRow>> {
        Arc::new(file_tree::tree_rows(key.paths, |f| key.closed.is_some_and(|c| c.contains(f))))
    }
}

/// The list header's List / Tree toggle: its icon shows the mode a click switches to.
pub(crate) fn mode_toggle(ui: &mut Ui, t: Lang, mode: FileListMode, msgs: &mut Vec<Msg>) {
    let (icon, tip, other) = match mode {
        FileListMode::List => (Icon::TreeView, t.tr("files.tree.show_tree.tip"), FileListMode::Tree),
        FileListMode::Tree => (Icon::ListView, t.tr("files.tree.show_list.tip"), FileListMode::List),
    };
    if ui.add(buttons::icon_only(icon, tip).size(ControlSize::Small).tip(tip)).clicked() {
        msgs.push(Msg::Layout(LayoutMsg::SetFileListMode(other)));
    }
}

/// A folder row of `list`'s tree: its indent, a chevron (down while open), a folder, its label
/// and at the right end how many files are under it. A click collapses or expands it; `menu`
/// fills its context menu, given the folder's path. Never drawn selected. `keyed` for a list that takes the keyboard
/// focus as a whole (`widgets::keyed_file_row_with_end`).
pub(crate) fn folder_row(
    ui: &mut Ui,
    row: &TreeRow,
    keyed: bool,
    msgs: &mut Vec<Msg>,
    list: FileListId,
    menu: impl FnOnce(&mut Ui, &mut Vec<Msg>, &str),
) -> egui::Response {
    let TreeRow::Folder { path, label, depth, open, files } = row else {
        // Not a folder: the row's room, so the list's rows stay in place.
        return ui.allocate_exact_size(vec2(ui.available_width(), widgets::LIST_ROW_H), Sense::hover()).1;
    };
    let name = format!("{path}/");
    let end = |ui: &mut Ui| {
        ui.label(RichText::new(files.to_string()).size(type_scale::SMALL).color(color::weak()));
    };
    let add = |ui: &mut Ui| {
        indent(ui, Some(*depth));
        let (slot, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
        widgets::chevron(ui.painter(), slot.center(), *open, color::weak());
        let (slot, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
        widgets::paint_icon_sized(ui.painter(), slot.center(), Icon::Folder, color::weak(), icon::SIZE_SM);
        widgets::row_text(ui, RichText::new(label.as_str()).monospace(), false);
    };
    let r = if keyed {
        widgets::keyed_file_row_with_end(ui, false, &name, end, add)
    } else {
        widgets::file_row_with_end(ui, false, &name, end, add)
    }
    .response;
    if r.clicked() {
        msgs.push(Msg::Layout(LayoutMsg::ToggleFolder { list, path: path.clone() }));
    }
    r.row_menu(|ui| menu(ui, msgs, path));
    r
}

/// A file row's indent in the tree (`depth` levels); nothing in List mode (None) or at the top.
pub(crate) fn indent(ui: &mut Ui, depth: Option<usize>) {
    if let Some(d) = depth.filter(|&d| d > 0) {
        ui.add_space(d as f32 * TREE_INDENT);
    }
}

/// A file row's path, in monospace, leaving `reserve` px after it: in the tree only the file's
/// name (its folders say the rest) with the whole path on hover, else the whole path.
pub(crate) fn file_text(ui: &mut Ui, path: &str, depth: Option<usize>, selected: bool, reserve: f32) -> egui::Response {
    match depth {
        Some(_) => widgets::row_text_hover(ui, RichText::new(file_tree::file_name(path)).monospace(), selected, reserve, path),
        None => widgets::row_text_reserving(ui, RichText::new(path).monospace(), selected, reserve),
    }
}
