//! The widths the toolbar's Repository › Branch pickers fit their names to. The repository's
//! slot fits its own name; the branch's fits the widest of the repository's local branch names
//! and the current label (a detached HEAD's text), so checking out any local branch keeps the
//! width and moves nothing after the pickers. Each width is the name's in the pickers' font,
//! rounded up to a multiple of `ROUND_TO` so small list changes don't nudge the bar, held between
//! the part's floor and cap (`REPO_FIT`, `BRANCH_FIT`), and never so narrow that the part's
//! caption is cut. The widest branch name is measured once per snapshot (its status stamp), font
//! and zoom and kept in egui's temporary data, since a repository can have thousands.

use crate::frame_memo;
use crate::theme::type_scale::{self, CAPTION};
use eframe::egui::{self, Color32, FontId, Id};
use glowfly_core::repo::Snapshot;
use glowfly_model::state::AppState;

/// The repository and branch name slots' floors and caps.
pub(super) const REPO_FIT: (f32, f32) = (48.0, 144.0);
pub(super) const BRANCH_FIT: (f32, f32) = (48.0, 192.0);
/// The step a fitted width is rounded up to.
const ROUND_TO: f32 = 8.0;
/// Where the widest local branch name is kept, with the snapshot, font and zoom it was measured for.
const WIDEST: &str = "toolbar-widest-branch";

/// `name_w` rounded up to a multiple of `ROUND_TO`, held within `(min, max)`, then widened to
/// `caption_need`, the slot the part's caption needs to show whole.
pub(super) fn fit(name_w: f32, (min, max): (f32, f32), caption_need: f32) -> f32 {
    ((name_w / ROUND_TO).ceil() * ROUND_TO).clamp(min, max).max(caption_need.ceil())
}

/// The width of `text` on one line in `font`.
pub(super) fn text_w(ctx: &egui::Context, text: &str, font: &FontId) -> f32 {
    ctx.fonts(|f| f.layout_no_wrap(text.to_owned(), font.clone(), Color32::PLACEHOLDER).size().x)
}

/// The slot a part's `caption` needs to show whole, when the part has `other` room besides its
/// name and side padding (the caret, and the branch's dot).
pub(super) fn caption_need(ctx: &egui::Context, caption: &str, other: f32) -> f32 {
    text_w(ctx, caption, &type_scale::font(CAPTION)) - other
}

/// The widest of `snap`'s local branch names in `font`: measured when the snapshot, the font or
/// the zoom changed, otherwise last measurement's.
pub(super) fn widest_branch(ctx: &egui::Context, snap: &Snapshot, font: &FontId) -> f32 {
    let key = (snap.status_stamp, font.clone(), ctx.pixels_per_point());
    frame_memo::kept(ctx, Id::new(WIDEST), key, || snap.locals.iter().map(|b| text_w(ctx, &b.name, font)).fold(0.0, f32::max))
}

/// The repository and branch name slots for `st`, before a narrow window shrinks them:
/// `repo` is the repository part's name, `head` the branch part's label, and `other` each
/// part's room besides its name and padding.
pub(super) fn slots(ui: &egui::Ui, st: &AppState, repo: &str, head: Option<&str>, other: (f32, f32)) -> (f32, f32) {
    let t = st.settings.lang();
    let ctx = ui.ctx();
    let font = type_scale::strong(ui, type_scale::BODY);
    let repo_slot = fit(text_w(ctx, repo, &font), REPO_FIT, caption_need(ctx, t.tr("toolbar.repository"), other.0));
    let branch_w = st.snap.as_ref().map_or(0.0, |snap| widest_branch(ctx, snap, &font)).max(head.map_or(0.0, |h| text_w(ctx, h, &font)));
    let branch_slot = fit(branch_w, BRANCH_FIT, caption_need(ctx, t.tr("toolbar.branch"), other.1));
    (repo_slot, branch_slot)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_width_rounds_up_to_the_step() {
        assert_eq!(fit(60.0, BRANCH_FIT, 0.0), 64.0);
        assert_eq!(fit(64.0, BRANCH_FIT, 0.0), 64.0);
        assert_eq!(fit(64.1, BRANCH_FIT, 0.0), 72.0);
    }

    #[test]
    fn a_width_stays_within_its_floor_and_cap() {
        assert_eq!(fit(20.0, BRANCH_FIT, 0.0), 48.0);
        assert_eq!(fit(900.0, BRANCH_FIT, 0.0), 192.0);
        assert_eq!(fit(900.0, REPO_FIT, 0.0), 144.0);
    }

    #[test]
    fn the_caption_is_never_cut() {
        assert_eq!(fit(20.0, REPO_FIT, 52.3), 53.0);
        assert_eq!(fit(100.0, REPO_FIT, 52.3), 104.0, "a wider name sets it");
    }

    #[test]
    fn the_widest_branch_is_kept_per_snapshot_font_and_zoom() {
        use crate::snapshot_tests::{remove_fixture, with_repo};
        let (mut st, dir) = with_repo(glowfly_core::settings::ThemeChoice::Dark, "widest-kept");
        let snap = st.snap.as_mut().unwrap();
        let ctx = egui::Context::default();
        crate::theme::fonts::use_test_cjk();
        crate::theme::fonts::install(&ctx);
        let _ = ctx.run(egui::RawInput::default(), |_| {});
        let font = FontId::proportional(14.0);
        let direct = |snap: &Snapshot, font: &FontId| snap.locals.iter().map(|b| text_w(&ctx, &b.name, font)).fold(0.0, f32::max);
        let w0 = widest_branch(&ctx, &*snap, &font);
        assert_eq!(w0, direct(&*snap, &font));
        let mut long = snap.locals[0].clone();
        long.name = "feature/a-really-quite-long-branch-name".into();
        snap.locals.push(long);
        assert_eq!(widest_branch(&ctx, &*snap, &font), w0, "kept for the same snapshot");
        snap.status_stamp = glowfly_core::repo::next_status_stamp();
        let w1 = widest_branch(&ctx, &*snap, &font);
        assert!(w1 > w0);
        assert_eq!(w1, direct(&*snap, &font));
        let big = FontId::proportional(16.0);
        assert_eq!(widest_branch(&ctx, &*snap, &big), direct(&*snap, &big), "a larger font measures again");
        remove_fixture(&dir);
    }
}
