//! The changed-file lists shown as a tree (`Settings::file_list_mode`): the rows a list draws
//! (its files, or folder rows and files grouped under them, `state::file_tree`), a folder row,
//! the header's List / Tree toggle and a file row's indent. The tree of a list is kept between
//! frames (`FrameCache`), keyed by which list it is (`FilesKey`) rather than by its paths, so a
//! list of thousands of files is neither sorted nor read again every frame.

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
use std::hash::{Hash, Hasher};
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
    /// list's order) are grouped, with the folders in `closed` collapsed, and read only when
    /// `files` (which list they are) or `closed` changed; List mode builds nothing.
    pub(crate) fn of<'p>(
        ui: &Ui,
        mode: FileListMode,
        n: usize,
        files: FilesKey,
        paths: &dyn Fn() -> Vec<&'p str>,
        closed: Option<&BTreeSet<String>>,
    ) -> ListRows {
        match mode {
            FileListMode::List => ListRows::Flat(n),
            FileListMode::Tree => {
                let key = TreeKey { files, closed, paths };
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

/// Which files a list holds, told apart without reading them: the list, a stamp of what it was
/// read from, and where its files are and how many. The WIP lists are stamped with
/// `Snapshot::status_stamp`, renewed by every load and by every status read, including those
/// that merge rows into the lists in place. The other lists (a commit's, a comparison's) are
/// only ever replaced whole by a new read, built while the old list is still alive, so a new
/// list has a new address; they are stamped with their commits as well.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub(crate) struct FilesKey {
    list: FileListId,
    stamp: u64,
    at: usize,
    len: usize,
}

impl FilesKey {
    /// The key of `list`, which shows `files`, read from what `stamp` names.
    pub(crate) fn of<T>(list: FileListId, files: &[T], stamp: impl Hash) -> Self {
        FilesKey { list, stamp: egui::util::hash(stamp), at: files.as_ptr() as usize, len: files.len() }
    }
}

/// What a list's tree is cached by: which files the list holds and its collapsed folders. The
/// paths are read only to build the tree.
#[derive(Clone, Copy)]
struct TreeKey<'a, 'p> {
    files: FilesKey,
    closed: Option<&'a BTreeSet<String>>,
    paths: &'a dyn Fn() -> Vec<&'p str>,
}

impl Hash for TreeKey<'_, '_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.files.hash(state);
        self.closed.hash(state);
    }
}

#[derive(Default)]
struct TreeComputer;

impl<'a, 'p> ComputerMut<TreeKey<'a, 'p>, Arc<Vec<TreeRow>>> for TreeComputer {
    fn compute(&mut self, key: TreeKey<'a, 'p>) -> Arc<Vec<TreeRow>> {
        Arc::new(file_tree::tree_rows(&(key.paths)(), |f| key.closed.is_some_and(|c| c.contains(f))))
    }
}

/// The list header's List / Tree toggle: its icon shows the mode a click switches to. Returns
/// its response.
pub(crate) fn mode_toggle(ui: &mut Ui, t: Lang, mode: FileListMode, msgs: &mut Vec<Msg>) -> egui::Response {
    let (icon, tip, other) = match mode {
        FileListMode::List => (Icon::TreeView, t.tr("files.tree.show_tree.tip"), FileListMode::Tree),
        FileListMode::Tree => (Icon::ListView, t.tr("files.tree.show_list.tip"), FileListMode::List),
    };
    let r = ui.add(buttons::icon_only(icon, tip).size(ControlSize::Small).tip(tip));
    if r.clicked() {
        msgs.push(Msg::Layout(LayoutMsg::SetFileListMode(other)));
    }
    r
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    /// The rows `of` gives `files` (stamped `stamp`) in a frame, and whether it read the paths.
    fn tree(ctx: &egui::Context, files: &[String], stamp: u64, closed: Option<&BTreeSet<String>>) -> (usize, bool) {
        let read = Cell::new(false);
        let mut len = 0;
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let paths = || {
                    read.set(true);
                    files.iter().map(String::as_str).collect()
                };
                let key = FilesKey::of(FileListId::Unstaged, files, stamp);
                len = ListRows::of(ui, FileListMode::Tree, files.len(), key, &paths, closed).len();
            });
        });
        (len, read.get())
    }

    #[test]
    fn a_tree_is_built_again_only_when_its_list_or_folds_change() {
        let ctx = egui::Context::default();
        let files: Vec<String> = ["src/a.rs", "src/b.rs", "c.md"].map(String::from).to_vec();
        assert_eq!(tree(&ctx, &files, 1, None), (4, true), "src/, its two files and c.md");
        assert_eq!(tree(&ctx, &files, 1, None), (4, false), "the same list: kept, its paths unread");
        // A status read that edits the list in place stamps it anew.
        assert_eq!(tree(&ctx, &files, 2, None), (4, true));
        // A new list.
        let more: Vec<String> = ["src/a.rs", "src/b.rs", "c.md", "d.md"].map(String::from).to_vec();
        assert_eq!(tree(&ctx, &more, 2, None), (5, true));
        // A folded folder.
        let closed = BTreeSet::from(["src".to_owned()]);
        assert_eq!(tree(&ctx, &more, 2, Some(&closed)), (3, true));
        assert_eq!(tree(&ctx, &more, 2, Some(&closed)), (3, false));
    }
}
