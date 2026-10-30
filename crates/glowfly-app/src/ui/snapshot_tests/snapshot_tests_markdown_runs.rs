//! A Markdown paragraph whose italic, bold, code, struck and link runs are each followed by a
//! space or punctuation: every run and the text between them, spaces kept (`markdown_inline.rs`
//! leaves room after an italic run's slant and beside a code run's background).

use super::markdown::body;
use super::markdown_cache::MarkdownCache;
use super::theme::{apply_theme, color};
use crate::msg::Msg;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui;
use egui_kittest::kittest::Queryable;

const PARAGRAPH: &str = "Read the *guide* first, then **try** the `glowfly` command. See ~~older~~ newer notes, \
the [docs](https://example.com/docs) and *this*.";

#[test]
fn markdown_runs_keep_their_spaces() {
    let mut cache = MarkdownCache::default();
    let mut h = crate::ui::snapshot_check::builder().with_size(egui::vec2(520.0, 120.0)).build_ui_state(
        move |ui, msgs: &mut Vec<Msg>| {
            color::set_dark(true);
            apply_theme(ui.ctx());
            ui.ctx().set_theme(egui::Theme::Dark);
            ui.painter().rect_filled(ui.max_rect(), 0.0, color::bg());
            body(ui, "runs", PARAGRAPH, &mut cache, msgs);
        },
        Vec::new(),
    );
    h.run();
    h.check_snapshot("markdown_runs_keep_their_spaces");
    for run in
        ["guide", " first, then ", "try", " the ", "glowfly", " command. See ", "older", " newer notes, the ", "docs", " and ", "this", "."]
    {
        assert!(h.query_by_label(run).is_some(), "the run {run:?}");
    }
}
