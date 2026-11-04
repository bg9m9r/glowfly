//! The header's ⋯, Stage file and Close: ⋯ holds every file action (Open, Open in diff tool,
//! Show in folder, then Restore… for a commit's file), the diff tool always enabled (with no
//! tool set it says it opens Preferences > External Tools); Stage file or Unstage file is a
//! small secondary button at the trailing edge, before the × that closes the diff. The header
//! has no filled button (`docs/COLOUR.md`, "The accent").

use super::controls::{self, ghost, icon_tip};
use crate::common::{menu_item_if, menu_item_setup};
use crate::file_menu::restore_msg;
use crate::theme::sizes::ControlSize;
use crate::widgets::menu::{self, popup_below};
use crate::widgets::{Icon, buttons};
use eframe::egui::{PopupCloseBehavior, Ui};
use glowfly_core::i18n::Lang;
use glowfly_model::keymap::{BindingId, Keymap};
use glowfly_model::msg::{FilesMsg, Msg, RepoMsg, StageMsg};
use glowfly_model::state::diff_header::{Actions, HeaderModel, Primary};
use glowfly_model::state::files::show_in_folder;
use glowfly_model::state::tip::Tip;
use glowfly_model::tools::ToolsMsg;

/// The space between Stage file and ×.
pub(super) const TRAILING_GAP: f32 = 8.0;
/// Stage file's size.
const STAGE: ControlSize = ControlSize::Small;

/// The ⋯'s width (0 when it holds nothing).
pub(super) fn more_width(a: &Actions) -> f32 {
    if a.restore.is_some() || a.open.is_some() || a.diff_tool.is_some() { controls::HEIGHT } else { 0.0 }
}

/// The ⋯ control and its menu of the file actions.
pub(super) fn more(ui: &mut Ui, m: &HeaderModel, keys: &Keymap, t: Lang, msgs: &mut Vec<Msg>) {
    let a = &m.actions;
    let id = ui.make_persistent_id("diff-file-actions");
    let name = t.tr("diff.header.more");
    let r = icon_tip(ghost(ui, ControlSize::Regular, Some(Icon::More), None, name, true), keys, t, Tip::new(name));
    if r.clicked() {
        ui.memory_mut(|mem| mem.toggle_popup(id));
    }
    popup_below(ui, id, &r, PopupCloseBehavior::CloseOnClickOutside, |ui| {
        ui.set_min_width(220.0);
        let before = msgs.len();
        if let Some(o) = &a.open {
            menu_item_if(msgs, ui, t.tr("files.menu.open_default"), o.missing, Msg::Files(FilesMsg::OpenWorkingFile(o.path.clone())));
        }
        if let Some(setup) = a.diff_tool {
            let open = Msg::Tools(ToolsMsg::OpenDiff(m.target.clone()));
            menu_item_setup(msgs, ui, t, t.tr("tools.open_in_diff_tool"), setup, open);
        }
        if let Some(o) = &a.open {
            menu_item_if(msgs, ui, show_in_folder(t), o.missing, Msg::Files(FilesMsg::ShowWorkingFile(o.path.clone())));
        }
        if let Some(r) = &a.restore {
            if a.open.is_some() || a.diff_tool.is_some() {
                menu::separator(ui);
            }
            let path: [(&str, &dyn std::fmt::Display); 1] = [("path", &r.path)];
            let tip =
                if r.status == 'D' { t.trf("diff.header.restore_deleted.tip", &path) } else { t.trf("diff.header.restore.tip", &path) };
            if menu::item(t.tr("diff.header.restore")).show(ui).on_hover_text(tip).clicked() {
                msgs.push(restore_msg(r.oid, &r.path, r.status));
            }
        }
        if msgs.len() > before {
            ui.memory_mut(|mem| mem.close_popup());
        }
    });
}

fn stage_button<'a>(t: Lang, p: &Primary, tip: &'a str) -> buttons::StyledButton<'a> {
    let text = match p {
        Primary::Stage(_) => t.tr("diff.header.stage_file"),
        Primary::Unstage(_) => t.tr("diff.header.unstage_file"),
    };
    buttons::secondary(text).size(STAGE).tip(tip)
}

/// Stage file's (or Unstage file's) tooltip in `t`, naming its key when it has one.
pub(super) fn primary_tip(p: &Primary, keys: &Keymap, t: Lang) -> String {
    match (p, keys.hint(BindingId::Stage), keys.hint(BindingId::Unstage)) {
        (Primary::Stage(_), Some(k), _) => t.trf("diff.header.stage_file_key.tip", &[("key", &k)]),
        (Primary::Stage(_), None, _) => t.tr("diff.header.stage_file.tip").to_owned(),
        (Primary::Unstage(_), _, Some(k)) => t.trf("diff.header.unstage_file_key.tip", &[("key", &k)]),
        (Primary::Unstage(_), _, None) => t.tr("diff.header.unstage_file.tip").to_owned(),
    }
}

/// Stage file's width (0 without it).
pub(super) fn primary_width(ui: &Ui, t: Lang, m: &HeaderModel) -> f32 {
    m.primary.as_ref().map_or(0.0, |p| stage_button(t, p, "").desired_width(ui))
}

pub(super) fn show_primary(ui: &mut Ui, m: &HeaderModel, keys: &Keymap, t: Lang, msgs: &mut Vec<Msg>) {
    let Some(p) = &m.primary else { return };
    let tip = primary_tip(p, keys, t);
    if ui.add(stage_button(t, p, &tip)).clicked() {
        msgs.push(Msg::Stage(match p {
            Primary::Stage(path) => StageMsg::StageFile(path.clone()),
            Primary::Unstage(path) => StageMsg::UnstageFile(path.clone()),
        }));
    }
}

pub(super) fn show_close(ui: &mut Ui, keys: &Keymap, t: Lang, msgs: &mut Vec<Msg>) {
    let r = crate::widgets::ghost::close_view(ui, keys, t, t.tr("diff.header.close.name"));
    if r.clicked() {
        msgs.push(Msg::Repo(RepoMsg::CloseDiff));
    }
}
