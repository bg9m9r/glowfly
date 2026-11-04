//! The widths the toolbar's Repository › Branch pickers fit their names to. The repository's
//! slot fits its own name; the branch's fits the widest of the repository's local branch names
//! and the current label (a detached HEAD's text), so checking out any local branch keeps the
//! width and moves nothing after the pickers. Each width is the name's in the pickers' font,
//! rounded up to a multiple of `ROUND_TO` so small list changes don't nudge the bar, held between
//! the part's floor and cap (`REPO_FIT`, `BRANCH_FIT`), and never so narrow that the part's
//! caption is cut. The widest branch name is measured once per local branch list and kept in
//! egui's temporary data under the list's hash, since a repository can have thousands.

use crate::theme::type_scale::{self, CAPTION};
use eframe::egui::{self, Color32, FontId, Id};
use glowfly_core::repo::Snapshot;
use glowfly_model::state::AppState;
use std::hash::{Hash, Hasher};

/// The repository and branch name slots' floors and caps.
pub(super) const REPO_FIT: (f32, f32) = (48.0, 144.0);
pub(super) const BRANCH_FIT: (f32, f32) = (48.0, 192.0);
/// The step a fitted width is rounded up to.
const ROUND_TO: f32 = 8.0;
/// Where the widest local branch name is kept, with the hash of what it was measured from.
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

/// The widest of `snap`'s local branch names in `font`: measured when the list (or the font)
/// changed, otherwise last measurement's.
pub(super) fn widest_branch(ctx: &egui::Context, snap: &Snapshot, font: &FontId) -> f32 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    font.hash(&mut h);
    ctx.pixels_per_point().to_bits().hash(&mut h);
    snap.locals.len().hash(&mut h);
    for b in &snap.locals {
        b.name.hash(&mut h);
    }
    let key = h.finish();
    let id = Id::new(WIDEST);
    if let Some((k, w)) = ctx.data(|d| d.get_temp::<(u64, f32)>(id))
        && k == key
    {
        return w;
    }
    let w = snap.locals.iter().map(|b| text_w(ctx, &b.name, font)).fold(0.0, f32::max);
    ctx.data_mut(|d| d.insert_temp(id, (key, w)));
    w
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
}
