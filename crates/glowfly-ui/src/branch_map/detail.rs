//! The Branch map's pane for the selected branch: its name, numbers and where it split from the
//! main branch, then one block per other branch that changes a file it changes: the other
//! branch's name, whether merging the two would conflict, how many files they share and the
//! files (the conflicting ones first, with a painted warning mark and the word for conflict).

use crate::theme::{color, type_scale};
use crate::widgets::chip::{Chip, ChipSize, Look};
use crate::widgets::severity::{self, Severity};
use crate::widgets::truncate_label;
use eframe::egui::{self, Align, Layout, RichText, Sense, TextWrapMode, Ui, WidgetText, vec2};
use glowfly_core::branch_map::{BranchMap, Forecast, Shared};
use glowfly_core::i18n::Lang;
use glowfly_model::state::AppState;
use glowfly_model::state::branch_map::{self as words, SHOWN_FILES};

const INSET: f32 = 16.0;
const MARK: f32 = 14.0;

pub(super) fn show(ui: &mut Ui, st: &AppState) {
    let Some(view) = &st.session.branch_map.view else { return };
    let (t, Some(map)) = (st.settings.lang(), view.map()) else { return };
    let Some(i) = view.selected_row() else {
        ui.add_space(32.0);
        ui.vertical_centered(|ui| ui.label(RichText::new(t.tr("branch_map.detail.select")).color(color::weak())));
        return;
    };
    egui::ScrollArea::vertical().id_salt("branch-map-shared").auto_shrink([false, false]).show(ui, |ui| {
        egui::Frame::new().inner_margin(egui::Margin::same(INSET as i8)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            pane(ui, t, map, i);
        });
    });
}

fn pane(ui: &mut Ui, t: Lang, map: &BranchMap, i: usize) {
    ui.spacing_mut().item_spacing.y = 6.0;
    let title = RichText::new(map.rows[i].branch.shown()).font(type_scale::strong(ui, type_scale::SUBHEAD)).color(color::text());
    ui.add(egui::Label::new(title).wrap());
    let small = |text: String| RichText::new(text).font(type_scale::font(type_scale::SMALL)).color(color::weak());
    ui.label(small(words::stats_line(t, map, i)));
    if i != 0 {
        let main = map.rows[0].branch.shown();
        let base = match words::short_base(map, i) {
            Some(sha) => t.trf("branch_map.detail.base", &[("main", &main), ("sha", &sha)]),
            None => t.trf("branch_map.row.unrelated", &[("main", &main)]),
        };
        ui.label(small(base));
    }
    ui.add_space(10.0);
    let blocks = map.shared_with(i);
    if blocks.is_empty() {
        ui.label(RichText::new(t.tr("branch_map.detail.none")).color(color::weak()));
    }
    for b in &blocks {
        block(ui, t, map, b);
        ui.add_space(10.0);
    }
}

/// One other branch: its name, the forecast badge and the shared count, then the files.
fn block(ui: &mut Ui, t: Lang, map: &BranchMap, b: &Shared<'_>) {
    let name = RichText::new(map.rows[b.other].branch.shown()).font(type_scale::section(ui)).color(color::text());
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let room = (ui.available_width() * 0.5).max(80.0);
        truncate_label(ui, name, room);
        let ink = if matches!(b.forecast, Forecast::Conflicts(_)) { color::warn() } else { color::weak() };
        let resp = Chip::new(ChipSize::Badge, words::badge(t, b.forecast)).look(Look::new(color::toolbar(), ink)).show(ui);
        if let Forecast::Unknown(why) = b.forecast {
            resp.on_hover_text(why);
        }
        let shared = t.trn("branch_map.detail.shared", b.shared, &[]);
        ui.label(RichText::new(shared).font(type_scale::font(type_scale::SMALL)).color(color::weak()));
    });
    for (path, conflict) in b.files.iter().take(SHOWN_FILES) {
        file(ui, t, path, *conflict);
    }
    if b.files.len() > SHOWN_FILES {
        let more = t.trn("branch_map.detail.and_more", b.files.len() - SHOWN_FILES, &[]);
        ui.label(RichText::new(more).font(type_scale::font(type_scale::SMALL)).color(color::weak()));
    }
}

/// One path in monospace after a slot for the warning mark; a conflicting one has the mark and
/// ends with the word for it.
fn file(ui: &mut Ui, t: Lang, path: &str, conflict: bool) {
    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let (rect, _) = ui.allocate_exact_size(vec2(MARK, MARK), Sense::hover());
        let word = RichText::new(t.tr("branch_map.detail.file_conflict")).font(type_scale::font(type_scale::SMALL)).color(color::warn());
        let word_w = if conflict {
            WidgetText::from(word.clone()).into_galley(ui, Some(TextWrapMode::Extend), f32::INFINITY, egui::TextStyle::Body).size().x + 6.0
        } else {
            0.0
        };
        if conflict {
            severity::paint(ui.painter(), rect.center(), MARK, Severity::Warning, color::warn());
        }
        let room = (ui.available_width() - word_w).max(0.0);
        truncate_label(ui, RichText::new(path).font(type_scale::mono(type_scale::SMALL)).color(color::text()), room);
        if conflict {
            ui.label(word);
        }
    });
}
