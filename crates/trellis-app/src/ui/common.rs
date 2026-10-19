//! Small view helpers shared across regions.

use super::theme::color;
use super::widgets;
use crate::msg::CloneMsg;
use crate::msg::ForgeMsg;
use crate::msg::Msg;
use crate::msg::PrefsMsg;
use crate::msg::RepoMsg;
use crate::state::{AppState, ForgeState, PrefPage, Status};
use eframe::egui::{self, Color32, RichText, pos2, vec2};
use git2::Oid;
use trellis_core::forge::{self, RefStyle};
use trellis_core::i18n::Lang;

const LANE_COLORS: [Color32; 10] = [
    Color32::from_rgb(0x1f, 0xb5, 0xc9),
    Color32::from_rgb(0x3d, 0x7d, 0xf7),
    Color32::from_rgb(0x9b, 0x5d, 0xe0),
    Color32::from_rgb(0xd0, 0x4f, 0xb8),
    Color32::from_rgb(0xe8, 0x4a, 0x7a),
    Color32::from_rgb(0xe8, 0x5c, 0x3a),
    Color32::from_rgb(0xf0, 0x8d, 0x2e),
    Color32::from_rgb(0xe8, 0xc4, 0x30),
    Color32::from_rgb(0x7b, 0xc9, 0x3f),
    Color32::from_rgb(0x2e, 0xc2, 0x95),
];

/// The height of a `ui.horizontal` row of labels and buttons, for lists drawn with
/// `ScrollArea::show_rows` (only the rows in view are laid out).
pub fn widget_row_height(ui: &egui::Ui) -> f32 {
    // A laid-out line, not the font's row height: galleys are rounded to whole points.
    let styles = [egui::TextStyle::Body, egui::TextStyle::Button, egui::TextStyle::Monospace];
    let line = |t: &egui::TextStyle| ui.fonts(|f| f.layout_no_wrap("Ag".into(), t.resolve(ui.style()), Color32::WHITE).size().y);
    let text = styles.iter().map(line).fold(0.0, f32::max);
    let s = ui.spacing();
    s.interact_size.y.max(text + 2.0 * s.button_padding.y)
}

/// How many lane colours there are before they repeat.
#[cfg(test)]
pub const LANE_COUNT: usize = LANE_COLORS.len();

pub fn lane_color(i: usize) -> Color32 {
    LANE_COLORS[i % LANE_COLORS.len()]
}

pub fn status_color(c: char) -> Color32 {
    match c {
        'A' | '?' => color::green(),
        'D' => color::red(),
        'R' => color::blue(),
        '!' => color::amber(),
        'M' => color::amber(),
        _ => color::weak(),
    }
}

/// Untracked files show as additions.
pub fn status_label(c: char) -> char {
    if c == '?' { 'A' } else { c }
}

pub fn state_color(state: &str) -> Color32 {
    match state.to_ascii_lowercase().as_str() {
        "new" | "to do" | "proposed" => color::weak(),
        "active" | "in progress" | "committed" | "doing" => color::blue(),
        "resolved" | "done" | "closed" => color::green(),
        _ => color::amber(),
    }
}

pub fn short(oid: Oid) -> String {
    oid.to_string()[..7].to_owned()
}

/// The widest a ref menu's header gets before its name wraps onto more lines.
const MENU_HEADER_W: f32 = 320.0;

/// An object menu's title: the object's name, once, in bold and not clickable (wrapped when it
/// is long), then a separator. The items below it leave the name out.
pub fn menu_header(ui: &mut egui::Ui, title: &str) {
    header(ui, RichText::new(title).strong());
}

/// A commit menu's title: the summary in bold (cut to 80 characters), then the short SHA, weak
/// and monospace; just the SHA when there is no summary.
pub fn commit_menu_header(ui: &mut egui::Ui, summary: &str, oid: Oid) {
    let cut = crate::state::commit_menu::header_summary(summary);
    let mut job = egui::text::LayoutJob::default();
    let (style, align) = (ui.style().clone(), egui::Align::Center);
    let sha = if cut.is_empty() {
        short(oid)
    } else {
        RichText::new(cut).strong().append_to(&mut job, &style, egui::FontSelection::Default, align);
        format!("  {}", short(oid))
    };
    RichText::new(sha).monospace().color(color::weak()).append_to(&mut job, &style, egui::FontSelection::Default, align);
    header(ui, job);
}

/// The header row: wrapped at `MENU_HEADER_W`, not clickable, then a separator.
fn header(ui: &mut egui::Ui, text: impl Into<egui::WidgetText>) {
    ui.scope(|ui| {
        ui.set_max_width(MENU_HEADER_W);
        ui.add(egui::Label::new(text).wrap().selectable(false));
    });
    ui.separator();
}

/// A weak, small, not clickable line in a menu that explains the item above it.
pub fn menu_note(ui: &mut egui::Ui, text: &str) {
    ui.add(egui::Label::new(RichText::new(text).small().color(color::weak())).selectable(false));
}

/// A menu item's button. Its text never wraps: a submenu opens narrow and grows to fit its
/// widest item, so wrapping text would break every word across lines.
fn menu_button(label: impl Into<egui::WidgetText>) -> egui::Button<'static> {
    egui::Button::new(label).wrap_mode(egui::TextWrapMode::Extend)
}

pub fn menu_item(msgs: &mut Vec<Msg>, ui: &mut egui::Ui, label: impl Into<egui::WidgetText>, m: Msg) {
    if ui.add(menu_button(label)).clicked() {
        msgs.push(m);
        ui.close_menu();
    }
}

/// A menu item that is disabled, with the reason on hover, while `blocked` is set.
pub fn menu_item_if(msgs: &mut Vec<Msg>, ui: &mut egui::Ui, label: impl Into<egui::WidgetText>, blocked: Option<&str>, m: Msg) {
    let mut r = ui.add_enabled(blocked.is_none(), menu_button(label));
    if let Some(why) = blocked {
        r = r.on_disabled_hover_text(why);
    }
    if r.clicked() {
        msgs.push(m);
        ui.close_menu();
    }
}

/// Small caret drawn after a menu button (the font has no ▾ glyph).
pub fn caret_after(ui: &mut egui::Ui, r: &egui::Response) {
    let c = pos2(r.rect.right() + 6.0, r.rect.center().y + 1.0);
    widgets::chevron(ui.painter(), c, true, color::weak());
}

/// A clickable `#123 · Type · State  Title` chip; shows just the key until the issue is loaded.
pub fn issue_chip(ui: &mut egui::Ui, fs: &ForgeState, key: &str, msgs: &mut Vec<Msg>) {
    let tag = fs.mention(key);
    let text = match fs.cache.get(key) {
        Some(i) => RichText::new(format!("{tag}  {} · {}  {}", i.kind, i.state, i.title)).color(color::text()),
        None => RichText::new(tag).color(color::blue()),
    };
    let r = ui.add(egui::Button::new(text.size(12.5)).fill(color::input()).truncate());
    if r.clicked() {
        msgs.push(Msg::OpenUrl(fs.issue_url(key)));
    }
    r.on_hover_text(format!("Open in {}", fs.tracker_name()));
}

/// A faint note under a section's header, indented like its rows and wrapped within the panel
/// (it often says what to do, so it is never cut).
pub fn hint_row(ui: &mut egui::Ui, text: String) {
    egui::Frame::new().inner_margin(egui::Margin { left: 26, right: 8, top: 4, bottom: 4 }).show(ui, |ui| {
        ui.add(egui::Label::new(RichText::new(text).size(11.5).color(color::faint())).wrap());
    });
}

pub fn forge_error_rows(ui: &mut egui::Ui, error: &str, msgs: &mut Vec<Msg>) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(26.0);
        ui.label(RichText::new(error).color(color::red()).size(12.0));
    });
    ui.horizontal(|ui| {
        ui.add_space(26.0);
        if ui.small_button("Sign in").clicked() {
            msgs.push(Msg::Forge(ForgeMsg::ForgeRefresh { interactive: true }));
        }
        if ui.small_button("Connect…").clicked() {
            msgs.push(Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Integrations)));
        }
    });
    ui.add_space(4.0);
}

pub fn status_label_ui(ui: &mut egui::Ui, t: Lang, status: &Status, ok_text: &str, msgs: &mut Vec<Msg>) {
    match status {
        Status::Ok => {
            ui.label(RichText::new(format!("· {ok_text}")).color(color::weak()));
        }
        Status::Error(e) => {
            ui.label(RichText::new(format!("· {e}")).color(color::red()));
            if ui.link(t.tr("status.sign_in")).clicked() {
                msgs.push(Msg::Forge(ForgeMsg::ForgeRefresh { interactive: true }));
            }
            if ui.link(t.tr("status.connect")).clicked() {
                msgs.push(Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Integrations)));
            }
        }
        Status::Idle => {}
    }
}

/// Splits "12, #13 AB#14 PROJ-5" into issue keys; a pasted Trello card link becomes its id.
pub fn parse_keys(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for part in text.split([',', ' ', ';']) {
        let card = forge::issue_refs(RefStyle::Trello, part).into_iter().next();
        let p = card.as_deref().unwrap_or(part.trim());
        let p = p.strip_prefix("AB#").or_else(|| p.strip_prefix("ab#")).unwrap_or(p).trim_start_matches('#');
        if !p.is_empty() && !out.iter().any(|k| k == p) {
            out.push(p.to_owned());
        }
    }
    out
}

pub fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

pub use crate::state::op_actions::state_actions;

/// Shown when no repository is open.
pub fn welcome(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() * 0.2);
        ui.label(RichText::new("Trellis").size(40.0).strong().color(color::text()));
        ui.label(RichText::new(t.tr("welcome_screen.tagline")).color(color::weak()));
        ui.add_space(20.0);
        if st.loading {
            ui.add(egui::Spinner::new());
            return;
        }
        let size = vec2(260.0, 34.0);
        if ui.add(egui::Button::new(t.tr("common.open_repo.button")).min_size(size)).clicked() {
            msgs.push(Msg::Repo(RepoMsg::PickRepo));
        }
        if ui.add(egui::Button::new(t.tr("common.clone_repo.button")).min_size(size)).clicked() {
            msgs.push(Msg::Clone(CloneMsg::Dialog));
        }
        if ui.add(egui::Button::new(t.tr("common.init_repo.button")).min_size(size)).clicked() {
            msgs.push(Msg::Repo(RepoMsg::InitRepo));
        }
        if ui.add(egui::Button::new(t.tr("welcome_screen.find.button")).min_size(size)).clicked() {
            msgs.push(Msg::Repos(crate::msg::ReposMsg::ShowFinder));
        }
        super::repo_list::welcome_sections(ui, st, msgs);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_issue_key_lists() {
        assert_eq!(parse_keys("12, #13 AB#14  PROJ-5,12"), ["12", "13", "14", "PROJ-5"]);
        assert_eq!(parse_keys("https://trello.com/c/AbCd1234/3-x, Zz9Yy8Xx"), ["AbCd1234", "Zz9Yy8Xx"]);
        assert_eq!(parse_keys("12, PROJ-5"), ["12", "PROJ-5"]);
        assert_eq!(capitalize("work item"), "Work item");
    }
}
