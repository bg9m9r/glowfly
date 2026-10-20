//! The terminal header's tab strip: one pill per shell of the repository (pill.rs), or the
//! rename field in place of the one being renamed; then New terminal tab (+) and, right against
//! it, the caret that picks another shell. Many pills scroll sideways instead of pushing out
//! the header's buttons. The + and the caret are painted (the fonts lack the caret glyph).

use super::pill;
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::terminal::TermSession;
use crate::state::terminal_names::tab_names;
use crate::terminal::TerminalMsg;
use crate::ui::prefs::terminal::choice_label;
use crate::ui::theme::color;
use crate::ui::widgets::chevron;
use crate::ui::widgets::tab_glyph::{self, TabGlyph};
use eframe::egui::{self, Align, CornerRadius, CursorIcon, Id, Layout, PopupCloseBehavior, Sense, Ui, WidgetInfo, WidgetType, pos2, vec2};
use trellis_core::i18n::Lang;
use trellis_core::settings::{ShellChoice, TerminalPrefs};

/// Between pills.
const PILL_GAP: f32 = 4.0;
/// The caret's click target.
const CARET: egui::Vec2 = vec2(14.0, 20.0);
/// Room kept after the strip for the + and the caret, with the gap before them.
const TAIL_W: f32 = 4.0 + tab_glyph::HIT + CARET.x;

/// Draws the strip, the + and the caret in `ui`, a left-to-right row centred on `cy`.
pub(super) fn strip(ui: &mut Ui, st: &mut AppState, cy: f32, msgs: &mut Vec<Msg>) {
    let Some(workdir) = st.snap.as_ref().map(|s| s.workdir.clone()) else { return };
    let t = st.settings.lang();
    let tabs: Vec<&TermSession> = st.terminal.sessions_for(&workdir).collect();
    let names = tab_names(&tabs);
    let widths: Vec<f32> = tabs.iter().zip(&names).map(|(s, n)| pill::width(ui, s, n)).collect();
    let ids: Vec<u64> = tabs.iter().map(|s| s.id).collect();
    let active = st.terminal.active_id(&workdir);
    let w = (ui.available_width() - TAIL_W).max(0.0);
    let area = egui::ScrollArea::horizontal().id_salt("terminal-tabs").max_width(w).auto_shrink([true, false]);
    area.show(ui, |ui| {
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = PILL_GAP;
            for (i, id) in ids.iter().enumerate() {
                match st.terminal.renaming.as_mut() {
                    Some(draft) if draft.id == *id => pill::rename_field(ui, draft, widths[i], msgs),
                    _ => {
                        let Some(s) = st.terminal.sessions.iter().find(|s| s.id == *id) else { continue };
                        pill::show(ui, s, &names[i], active == Some(*id), &st.keymap, t, msgs);
                    }
                }
            }
        });
    });
    ui.add_space(4.0);
    let (slot, _) = ui.allocate_exact_size(vec2(tab_glyph::HIT, tab_glyph::HIT), Sense::hover());
    let name = t.tr("toolbar.terminal.new_tab");
    let centre = pos2(slot.center().x, cy);
    let plus = tab_glyph::button_at(ui, Id::new("new-terminal-tab"), centre, TabGlyph::Plus, name, color::weak(), color::hover());
    if plus.clicked() {
        msgs.push(Msg::Terminal(TerminalMsg::NewTab(None)));
    }
    ui.spacing_mut().item_spacing.x = 0.0;
    shell_picker(ui, &st.settings.terminal, t, cy, msgs);
}

/// The caret after the +, opening the menu of shells.
fn shell_picker(ui: &mut Ui, prefs: &TerminalPrefs, t: Lang, cy: f32, msgs: &mut Vec<Msg>) {
    let (slot, _) = ui.allocate_exact_size(CARET, Sense::hover());
    let rect = egui::Rect::from_center_size(pos2(slot.center().x, cy), CARET);
    let resp = ui.interact(rect, Id::new("terminal-shell-picker"), Sense::click());
    let name = t.tr("terminal.header.pick_shell");
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, name));
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(3), color::hover());
    }
    chevron(ui.painter(), rect.center(), true, color::weak());
    let resp = resp.on_hover_cursor(CursorIcon::PointingHand).on_hover_text(name);
    let popup = Id::new("terminal-shell-menu");
    if resp.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup));
    }
    egui::popup::popup_below_widget(ui, popup, &resp, PopupCloseBehavior::CloseOnClick, |ui| {
        ui.set_min_width(220.0);
        let before = msgs.len();
        shell_menu(ui, &shell_items(t, prefs, cfg!(windows)), msgs);
        if msgs.len() > before {
            ui.memory_mut(|m| m.close_popup());
        }
    });
}

/// The shells the caret offers on a system where `windows` says whether it is Windows: Custom
/// only with a program set (named after it), and Preferences' choice marked as the default.
pub(in crate::ui) fn shell_items(t: Lang, prefs: &TerminalPrefs, windows: bool) -> Vec<(ShellChoice, String)> {
    let custom = trellis_core::external_tools::program(&prefs.shell.program)
        .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .filter(|s| !s.is_empty());
    ShellChoice::offered(windows)
        .iter()
        .filter_map(|&c| {
            let label = match (c, &custom) {
                (ShellChoice::Custom, None) => return None,
                (ShellChoice::Custom, Some(stem)) => format!("{} ({stem})", choice_label(t, c, windows)),
                _ => choice_label(t, c, windows).to_owned(),
            };
            let label = if c == prefs.shell.choice { t.trf("terminal.shell_menu.default", &[("shell", &label)]) } else { label };
            Some((c, label))
        })
        .collect()
}

/// The shell menu's items; a click starts a terminal with that shell.
pub(in crate::ui) fn shell_menu(ui: &mut Ui, items: &[(ShellChoice, String)], msgs: &mut Vec<Msg>) {
    for (choice, label) in items {
        if ui.button(label.as_str()).clicked() {
            msgs.push(Msg::Terminal(TerminalMsg::NewTab(Some(*choice))));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use trellis_core::settings::ShellSetting;

    fn labels(prefs: &TerminalPrefs, windows: bool) -> Vec<String> {
        shell_items(Lang::En, prefs, windows).into_iter().map(|(_, l)| l).collect()
    }

    #[test]
    fn the_menu_lists_this_systems_shells_with_the_default_marked() {
        let mut prefs = TerminalPrefs::default();
        prefs.shell.choice = ShellChoice::Pwsh;
        let windows = ["Automatic", "PowerShell 7 (pwsh) (default)", "Windows PowerShell", "Command Prompt (cmd)", "Bash (Git Bash)"];
        assert_eq!(labels(&prefs, true), windows);
        prefs.shell.choice = ShellChoice::Auto;
        assert_eq!(labels(&prefs, false), ["Automatic (default)", "PowerShell 7 (pwsh)", "Bash"]);
    }

    #[test]
    fn custom_is_listed_only_with_a_program() {
        let shell = ShellSetting { choice: ShellChoice::Custom, program: String::new(), args: String::new() };
        let mut prefs = TerminalPrefs { shell, ..Default::default() };
        assert!(!shell_items(Lang::En, &prefs, false).iter().any(|(c, _)| *c == ShellChoice::Custom));
        prefs.shell.program = "/opt/nu/bin/nu".into();
        let items = shell_items(Lang::En, &prefs, false);
        assert_eq!(items.last(), Some(&(ShellChoice::Custom, "Custom program (nu) (default)".to_owned())));
    }
}
