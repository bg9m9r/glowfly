//! The header's file actions, primary action and Close: Restore… (a commit's file), Open and
//! Show in folder (labelled, or folded into one ⋯ menu in a narrow header), the external diff
//! tool (always enabled: with no tool set its tooltip says it opens Preferences > External
//! Tools), Stage file or Unstage file, and the × that closes the diff.

use super::controls::{self, SPACE, ghost, icon_tip};
use crate::keymap::{BindingId, Keymap};
use crate::msg::{FilesMsg, Msg, RepoMsg, StageMsg};
use crate::state::diff_header::{Actions, HeaderModel, Primary, RestoreAction};
use crate::state::tip::Tip;
use crate::state::tool_compare::OPEN_IN_DIFF_TOOL;
use crate::tools::ToolsMsg;
use crate::ui::common::menu_item_if;
use crate::ui::file_menu::{OPEN_DEFAULT, SHOW_IN_FOLDER, restore_msg};
use crate::ui::widgets::{Icon, buttons};
use eframe::egui::{self, PopupCloseBehavior, Ui};
use trellis_core::i18n::Lang;

pub(super) const RESTORE: &str = "Restore…";
pub(super) const OPEN: &str = "Open";
pub(super) const MORE: &str = "More file actions";
pub(super) const CLOSE_NAME: &str = "Close diff";
pub(super) const STAGE_FILE: &str = "Stage file";
pub(super) const UNSTAGE_FILE: &str = "Unstage file";

/// One labelled action: icon, label, tooltip, why it is off, and its message.
struct Item {
    icon: Icon,
    label: &'static str,
    menu_label: &'static str,
    tip: String,
    missing: Option<&'static str>,
    msg: Msg,
}

fn restore_tip(r: &RestoreAction) -> String {
    if r.status == 'D' {
        format!("Bring back {} as it was before this commit deleted it", r.path)
    } else {
        format!("Replace {} in your working tree with this commit's version", r.path)
    }
}

/// Restore…, Open and Show in folder, as `a` has them.
fn items(a: &Actions) -> Vec<Item> {
    let mut v = Vec::new();
    if let Some(r) = &a.restore {
        let msg = restore_msg(r.oid, &r.path, r.status);
        v.push(Item { icon: Icon::Undo, label: RESTORE, menu_label: RESTORE, tip: restore_tip(r), missing: None, msg });
    }
    if let Some(o) = &a.open {
        let open = Msg::Files(FilesMsg::OpenWorkingFile(o.path.clone()));
        v.push(Item {
            icon: Icon::OpenFile,
            label: OPEN,
            menu_label: OPEN_DEFAULT,
            tip: OPEN_DEFAULT.into(),
            missing: o.missing,
            msg: open,
        });
        let show = Msg::Files(FilesMsg::ShowWorkingFile(o.path.clone()));
        v.push(Item {
            icon: Icon::Folder,
            label: SHOW_IN_FOLDER,
            menu_label: SHOW_IN_FOLDER,
            tip: SHOW_IN_FOLDER.into(),
            missing: o.missing,
            msg: show,
        });
    }
    v
}

/// The labelled actions' width, the folded ⋯'s (both 0 without any), and the diff tool's with
/// the space before it.
pub(super) fn widths(ui: &Ui, a: &Actions) -> (f32, f32, f32) {
    let items = items(a);
    let labelled = items.iter().map(|i| controls::ghost_width(ui, true, Some(i.label), 0.0)).sum::<f32>()
        + items.len().saturating_sub(1) as f32 * SPACE;
    let folded = if items.is_empty() { 0.0 } else { controls::HEIGHT };
    let tool = match a.diff_tool {
        Some(_) if items.is_empty() => controls::HEIGHT,
        Some(_) => controls::HEIGHT + SPACE,
        None => 0.0,
    };
    (labelled, folded, tool)
}

/// Draws the file actions, labelled or (`fold`) behind ⋯, then the diff tool control.
pub(super) fn show(ui: &mut Ui, m: &HeaderModel, fold: bool, keys: &Keymap, t: Lang, msgs: &mut Vec<Msg>) {
    let items = items(&m.actions);
    if fold && !items.is_empty() {
        more(ui, items, keys, t, msgs);
    } else {
        for (i, item) in items.into_iter().enumerate() {
            if i > 0 {
                ui.add_space(SPACE);
            }
            let mut r = ghost(ui, Some(item.icon), Some(item.label), item.label, item.missing.is_none());
            r = match item.missing {
                Some(why) => r.on_hover_text(why),
                None => r.on_hover_text(item.tip),
            };
            if r.clicked() {
                msgs.push(item.msg);
                r.surrender_focus();
            }
        }
    }
    if let Some(setup) = m.actions.diff_tool {
        if m.actions.labelled() {
            ui.add_space(SPACE);
        }
        let tip = Tip::new(OPEN_IN_DIFF_TOOL);
        let tip = match setup {
            Some(note) => tip.body(note),
            None => tip,
        };
        let r = icon_tip(ghost(ui, Some(Icon::ExternalDiff), None, OPEN_IN_DIFF_TOOL, true), keys, t, tip);
        if r.clicked() {
            msgs.push(Msg::Tools(ToolsMsg::OpenDiff(m.target.clone())));
            r.surrender_focus();
        }
    }
}

/// The ⋯ control and its menu of the same actions.
fn more(ui: &mut Ui, items: Vec<Item>, keys: &Keymap, t: Lang, msgs: &mut Vec<Msg>) {
    let id = ui.make_persistent_id("diff-file-actions");
    let r = icon_tip(ghost(ui, Some(Icon::More), None, MORE, true), keys, t, Tip::new(MORE));
    if r.clicked() {
        ui.memory_mut(|mem| mem.toggle_popup(id));
    }
    egui::popup::popup_below_widget(ui, id, &r, PopupCloseBehavior::CloseOnClickOutside, |ui| {
        ui.set_min_width(200.0);
        let before = msgs.len();
        for item in items {
            menu_item_if(msgs, ui, item.menu_label, item.missing, item.msg);
        }
        if msgs.len() > before {
            ui.memory_mut(|mem| mem.close_popup());
        }
    });
}

fn primary_button<'a>(p: &Primary, tip: &'a str) -> buttons::StyledButton<'a> {
    let text = match p {
        Primary::Stage(_) => STAGE_FILE,
        Primary::Unstage(_) => UNSTAGE_FILE,
    };
    buttons::primary(text).radius(controls::RADIUS).tip(tip)
}

/// The primary button's tooltip in `t`, naming its key when it has one.
pub(super) fn primary_tip(p: &Primary, keys: &Keymap, t: Lang) -> String {
    match (p, keys.hint(BindingId::Stage), keys.hint(BindingId::Unstage)) {
        (Primary::Stage(_), Some(k), _) => t.trf("diff.header.stage_file_key.tip", &[("key", &k)]),
        (Primary::Stage(_), None, _) => t.tr("diff.header.stage_file.tip").to_owned(),
        (Primary::Unstage(_), _, Some(k)) => t.trf("diff.header.unstage_file_key.tip", &[("key", &k)]),
        (Primary::Unstage(_), _, None) => t.tr("diff.header.unstage_file.tip").to_owned(),
    }
}

/// The primary button's width (0 without one).
pub(super) fn primary_width(ui: &Ui, m: &HeaderModel) -> f32 {
    m.primary.as_ref().map_or(0.0, |p| primary_button(p, "").desired_width(ui))
}

pub(super) fn show_primary(ui: &mut Ui, m: &HeaderModel, keys: &Keymap, t: Lang, msgs: &mut Vec<Msg>) {
    let Some(p) = &m.primary else { return };
    let tip = primary_tip(p, keys, t);
    if ui.add(primary_button(p, &tip)).clicked() {
        msgs.push(Msg::Stage(match p {
            Primary::Stage(path) => StageMsg::StageFile(path.clone()),
            Primary::Unstage(path) => StageMsg::UnstageFile(path.clone()),
        }));
    }
}

pub(super) fn show_close(ui: &mut Ui, keys: &Keymap, t: Lang, msgs: &mut Vec<Msg>) {
    let r = icon_tip(
        ghost(ui, Some(Icon::Close), None, CLOSE_NAME, true),
        keys,
        t,
        Tip::new(t.tr("common.close.button")).keys(BindingId::CloseView),
    );
    if r.clicked() {
        msgs.push(Msg::Repo(RepoMsg::CloseDiff));
    }
}
