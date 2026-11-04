//! Help > About Glowfly (`Modal::About`, design pass D20): the icon, the wordmark and the
//! tagline, a quiet table of the version, git, the system and the build, the licence as a
//! footnote, and links to the third-party notices (`notices.rs`), the privacy notice (PRIVACY.md
//! on the website) and the website. Copy details copies the table for bug reports.

use crate::msg::{AppMsg, Msg};
use crate::state::git_env::GitState;
use crate::state::menu_bar::{BuildInfo, LICENSE, PRIVACY, WEBSITE, git_version, version_info};
use crate::ui::app_mark;
use crate::ui::dialog_frame::{self, Cx, Dialog, Footer};
use crate::ui::theme::{color, type_scale};
use crate::ui::widgets::buttons;
use eframe::egui::{self, Align, Image, Layout, RichText, Sense, Ui, UiBuilder, pos2, vec2};
use glowfly_core::i18n::Lang;

/// The app's name: the dialog's accessible name and the wordmark (not translated).
const NAME: &str = "Glowfly";
/// The dialog's content width.
const WIDTH: f32 = 400.0;
/// The icon's side, and the space above it.
const ICON: f32 = 96.0;
const ICON_TOP: f32 = 8.0;
/// The wordmark: 1.4 × `type_scale::TITLE`.
const NAME_PX: f32 = type_scale::TITLE * 1.4;
/// The table's rows, the gutter between its columns and its values' size (the help text's).
const ROW_H: f32 = 22.0;
const GUTTER: f32 = 16.0;
const VALUE_PX: f32 = type_scale::HELP;
/// Between a link and the dot that separates it from the next.
const LINK_GAP: f32 = 8.0;

/// What About shows of this copy: its `version` and its system and build.
pub(crate) struct ThisCopy<'a> {
    pub version: &'a str,
    pub build: &'a BuildInfo,
}

/// Draws the dialog in `cx.t`; true when it should close. Tests pass a fixed version and
/// `BuildInfo::SAMPLE`, so a release or the CI system doesn't change their images. The copied
/// details stay English, for bug reports.
pub(crate) fn about(ui: &mut Ui, cx: Cx, git: &GitState, copy: ThisCopy, msgs: &mut Vec<Msg>) -> bool {
    let t = cx.t;
    let tip = t.tr("dialog.close.tip");
    let header = move |ui: &mut Ui| {
        let (right, top) = (ui.cursor().left() + ui.available_width(), ui.cursor().top());
        dialog_frame::close_button(ui, tip, right, top);
    };
    let mut d = Dialog::new(cx, NAME).width(WIDTH).header(header).begin(ui);
    d.body(ui, |ui| {
        ui.vertical_centered(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.add_space(ICON_TOP);
            ui.add(Image::new(&app_mark::icon(ui.ctx())).fit_to_exact_size(vec2(ICON, ICON)));
            ui.add_space(12.0);
            ui.label(RichText::new(NAME).font(type_scale::strong(ui, NAME_PX)).color(color::text()));
            ui.add_space(4.0);
            ui.label(RichText::new(t.tr("about.tagline")).font(type_scale::font(type_scale::SUBHEAD)).color(color::text()));
            ui.add_space(2.0);
            ui.add(egui::Label::new(RichText::new(t.tr("about.description")).color(color::weak())).wrap());
            ui.add_space(24.0);
            table(ui, &rows(t, git, &copy));
            ui.add_space(20.0);
            let small =
                |text: String| egui::Label::new(RichText::new(text).font(type_scale::font(type_scale::SMALL)).color(color::weak())).wrap();
            ui.add(small(t.trf("about.license", &[("license", &LICENSE)])));
            ui.add(small(t.tr("about.no_warranty").to_owned()));
            ui.add_space(12.0);
            links(ui, t, msgs);
        });
    });
    let footer = Footer::new().extra(buttons::secondary(t.tr("common.copy_details.button")).tip(t.tr("about.copy_info.tip"))).close();
    let out = d.finish(ui, footer);
    if out.extra.is_some() {
        msgs.push(Msg::Copy(version_info(copy.version, git, copy.build)));
    }
    out.dismiss
}

/// The table's rows: each label and its value.
fn rows(t: Lang, git: &GitState, copy: &ThisCopy) -> [(&'static str, String); 4] {
    let git = git_version(git).unwrap_or_else(|| t.tr("about.unknown").to_owned());
    [
        (t.tr("about.row.version"), copy.version.to_owned()),
        (t.tr("about.row.git"), git),
        (t.tr("about.row.os"), copy.build.os_name().to_owned()),
        (t.tr("about.row.build"), copy.build.build_text()),
    ]
}

/// The two columns centred as one block: the labels right-aligned to the widest, then the
/// gutter, then the values left-aligned in mono.
fn table(ui: &mut Ui, rows: &[(&str, String)]) {
    let width_of = |text: &str, font| ui.painter().layout_no_wrap(text.to_owned(), font, color::text()).size().x;
    let label_w = rows.iter().map(|(l, _)| width_of(l, type_scale::font(type_scale::BODY))).fold(0.0, f32::max);
    let value_w = rows.iter().map(|(_, v)| width_of(v, type_scale::mono(VALUE_PX))).fold(0.0, f32::max);
    let (block, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H * rows.len() as f32), Sense::hover());
    let left = (block.center().x - (label_w + GUTTER + value_w) / 2.0).round();
    for (i, (label, value)) in rows.iter().enumerate() {
        let top = block.top() + i as f32 * ROW_H;
        let cell = |x: f32, w: f32| egui::Rect::from_min_size(pos2(x, top), vec2(w, ROW_H));
        let right = Layout::right_to_left(Align::Center);
        ui.scope_builder(UiBuilder::new().max_rect(cell(left, label_w)).layout(right), |ui| {
            ui.label(RichText::new(*label).color(color::weak()));
        });
        let start = Layout::left_to_right(Align::Center);
        ui.scope_builder(UiBuilder::new().max_rect(cell(left + label_w + GUTTER, value_w)).layout(start), |ui| {
            ui.label(RichText::new(value).font(type_scale::mono(VALUE_PX)).color(color::text()));
        });
    }
}

/// Third-party notices…, Privacy and Website as links, centred, a faint dot between each.
fn links(ui: &mut Ui, t: Lang, msgs: &mut Vec<Msg>) {
    let items = [t.tr("about.notices.button"), t.tr("about.privacy.button"), t.tr("about.website.button")];
    let width_of = |text: &str| ui.painter().layout_no_wrap(text.to_owned(), type_scale::font(type_scale::BODY), color::text()).size().x;
    let dot_w = width_of("·");
    let total = items.iter().map(|s| width_of(s)).sum::<f32>() + (items.len() - 1) as f32 * (dot_w + 2.0 * LINK_GAP);
    let h = ui.text_style_height(&egui::TextStyle::Body);
    let mut clicked = None;
    ui.allocate_ui_with_layout(vec2(total, h), Layout::left_to_right(Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for (i, text) in items.iter().enumerate() {
            if i > 0 {
                // Painted, so a screen reader reads only the links.
                let (r, _) = ui.allocate_exact_size(vec2(dot_w + 2.0 * LINK_GAP, h), Sense::hover());
                ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, "·", type_scale::font(type_scale::BODY), color::faint());
            }
            if ui.link(*text).clicked() {
                clicked = Some(i);
            }
        }
    });
    match clicked {
        Some(0) => msgs.push(Msg::App(AppMsg::ShowNotices)),
        Some(1) => msgs.push(Msg::OpenUrl(PRIVACY.to_owned())),
        Some(_) => msgs.push(Msg::OpenUrl(WEBSITE.to_owned())),
        None => {}
    }
}
