//! Small view helpers shared across regions.

use super::theme::color;
use super::widgets;
use super::widgets::reason::{self, ItemState};
use crate::msg::CloneMsg;
use crate::msg::ForgeMsg;
use crate::msg::Msg;
use crate::msg::PrefsMsg;
use crate::msg::RepoMsg;
use crate::state::blocked::Blocked;
use crate::state::{AppState, ForgeState, PrefPage, Status};
use crate::ui::theme::type_scale;
use eframe::egui::{self, Color32, RichText, pos2, vec2};
use git2::Oid;
use trellis_core::forge::{self, RefStyle};
use trellis_core::i18n::Lang;

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
pub const LANE_COUNT: usize = super::theme::families::LANE_COUNT;

/// Graph lane `i`'s colour, from the active palette's branch family.
pub fn lane_color(i: usize) -> Color32 {
    color::lane(i)
}

/// Added lines and files: the one colour shared by status letters, line counts, bars and chips.
pub fn added_color() -> Color32 {
    color::added()
}

/// Removed lines and deleted files (shared by status letters, line counts, bars and chips).
pub fn removed_color() -> Color32 {
    color::removed()
}

/// Modified files (shared by status letters, line counts, bars and chips).
pub fn modified_color() -> Color32 {
    color::modified()
}

pub fn status_color(c: char) -> Color32 {
    match c {
        'A' | '?' => added_color(),
        'D' => removed_color(),
        'R' => color::info(),
        'M' | '!' => modified_color(),
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
        "active" | "in progress" | "committed" | "doing" => color::info(),
        "resolved" | "done" | "closed" => color::added(),
        _ => color::warn(),
    }
}

pub fn short(oid: Oid) -> String {
    oid.to_string()[..7].to_owned()
}

/// `template` with `code` put in place of `slot`: the words proportional and the code (a SHA or
/// path) monospace, both at `px` in `color`. A template without the slot shows as it is.
pub fn with_code(template: &str, slot: &str, code: &str, px: f32, color: egui::Color32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let words = egui::TextFormat::simple(type_scale::font(px), color);
    let mono = egui::TextFormat::simple(type_scale::mono(px), color);
    let mut parts = template.split(slot);
    if let Some(first) = parts.next() {
        job.append(first, 0.0, words.clone());
    }
    for part in parts {
        job.append(code, 0.0, mono.clone());
        job.append(part, 0.0, words.clone());
    }
    job
}

/// The widest a ref menu's header gets before its name wraps onto more lines.
const MENU_HEADER_W: f32 = 320.0;

/// An object menu's title: the object's name, once, in bold and not clickable (wrapped when it
/// is long), then a separator. The items below it leave the name out.
pub fn menu_header(ui: &mut egui::Ui, title: &str) {
    header(ui, RichText::new(title).font(type_scale::strong(ui, type_scale::BODY)));
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
        RichText::new(cut).font(type_scale::strong(ui, type_scale::BODY)).append_to(&mut job, &style, egui::FontSelection::Default, align);
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
    ui.add(egui::Label::new(RichText::new(text).size(type_scale::SMALL).color(color::weak())).selectable(false));
}

/// A menu item. Its text never wraps: a submenu opens narrow and grows to fit its widest item,
/// so wrapping text would break every word across lines.
pub fn menu_item(msgs: &mut Vec<Msg>, ui: &mut egui::Ui, label: impl Into<egui::WidgetText>, m: Msg) {
    menu_item_state(msgs, ui, label, ItemState::Ready, m);
}

/// A menu item that is disabled while `blocked` is set: a short form of the reason follows the
/// label, and the whole reason shows on hover.
pub fn menu_item_if(msgs: &mut Vec<Msg>, ui: &mut egui::Ui, label: impl Into<egui::WidgetText>, blocked: Option<&str>, m: Msg) {
    menu_item_state(msgs, ui, label, ItemState::blocked_by(blocked), m);
}

/// `menu_item_if` with the site's own short form of the reason.
pub fn menu_item_blocked(msgs: &mut Vec<Msg>, ui: &mut egui::Ui, label: impl Into<egui::WidgetText>, blocked: Option<Blocked<'_>>, m: Msg) {
    menu_item_state(msgs, ui, label, blocked.map_or(ItemState::Ready, ItemState::Blocked), m);
}

/// A menu item that stays enabled while what it needs isn't set up (`setup` holds the hover
/// note): a weak "Set up…" follows the label, and running it opens the setting.
pub fn menu_item_setup(msgs: &mut Vec<Msg>, ui: &mut egui::Ui, label: impl Into<egui::WidgetText>, setup: Option<&str>, m: Msg) {
    menu_item_state(msgs, ui, label, setup.map_or(ItemState::Ready, ItemState::Setup), m);
}

/// A menu item in `state` (`reason::menu_button`), pushing `m` and closing the menu when clicked.
pub fn menu_item_state(msgs: &mut Vec<Msg>, ui: &mut egui::Ui, label: impl Into<egui::WidgetText>, state: ItemState<'_>, m: Msg) {
    if reason::menu_button(ui, label, "", state).clicked() {
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
        None => RichText::new(tag).color(color::info()),
    };
    let r = ui.add(egui::Button::new(text.size(type_scale::SMALL)).fill(color::input()).truncate());
    if r.clicked() {
        msgs.push(Msg::OpenUrl(fs.issue_url(key)));
    }
    r.on_hover_text(format!("Open in {}", fs.tracker_name()));
}

/// A faint note under a section's header, indented like its rows and wrapped within the panel
/// (it often says what to do, so it is never cut).
pub fn hint_row(ui: &mut egui::Ui, text: String) {
    egui::Frame::new().inner_margin(egui::Margin { left: widgets::ROW_INDENT as i8, right: widgets::PAD_X as i8, top: 4, bottom: 4 }).show(
        ui,
        |ui| {
            ui.add(egui::Label::new(RichText::new(text).size(type_scale::CAPTION).color(color::faint())).wrap());
        },
    );
}

pub fn forge_error_rows(ui: &mut egui::Ui, error: &str, msgs: &mut Vec<Msg>) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(widgets::ROW_INDENT);
        ui.label(RichText::new(error).color(color::removed()).size(type_scale::SMALL));
    });
    ui.horizontal(|ui| {
        ui.add_space(widgets::ROW_INDENT);
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
            ui.label(RichText::new(format!("· {e}")).color(color::removed()));
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
        ui.label(RichText::new("Trellis").font(type_scale::strong(ui, type_scale::WORDMARK)).color(color::text()));
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
#[path = "snapshot_tests_disabled_reasons.rs"]
mod reason_snapshots;

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
