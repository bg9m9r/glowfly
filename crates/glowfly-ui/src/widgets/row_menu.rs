//! Right-click menus that open below the row they belong to, so the row (and the status at its
//! right end) stays in view while the menu is open, in the shared menu style (`menu.rs`).
//! Shift+F10 opens one from the keyboard, below its row, on the focused row or the row its view
//! has selected (`row_menu_for`), its first item focused (`menu_keys`; egui reports no Menu
//! key). `menu::context_menu` shares the code, at the pointer.

use crate::ui::menu_keys;
use eframe::egui::menu::{BarState, MenuRoot};
use eframe::egui::{Id, InnerResponse, Rect, Response, Ui, Vec2, pos2};

/// Space between the row's edge and the menu.
const GAP: f32 = 2.0;

/// The id egui keeps its context menu under. Sharing it keeps `Context::is_context_menu_open`
/// and `Response::context_menu_opened` answering for these menus too, and opening one closes
/// any other.
const CONTEXT_MENU_ID: &str = "__egui::context_menu";

pub trait RowMenuExt {
    /// Like `Response::context_menu`, but the menu's top edge sits just below the row (its left
    /// edge at the pointer, never left of the row), or its bottom edge just above the row when it
    /// doesn't fit below inside the window. Shift+F10 opens it while the row has the focus.
    fn row_menu(&self, add_contents: impl FnOnce(&mut Ui)) -> Option<InnerResponse<()>>;

    /// `row_menu`, which Shift+F10 opens while the row is the keyboard's `target` too (the view's
    /// selected row, while the view has the keyboard).
    fn row_menu_for(&self, target: bool, add_contents: impl FnOnce(&mut Ui)) -> Option<InnerResponse<()>>;
}

impl RowMenuExt for Response {
    fn row_menu(&self, add_contents: impl FnOnce(&mut Ui)) -> Option<InnerResponse<()>> {
        show(self, false, true, add_contents)
    }

    fn row_menu_for(&self, target: bool, add_contents: impl FnOnce(&mut Ui)) -> Option<InnerResponse<()>> {
        show(self, target, true, add_contents)
    }
}

/// Mirrors egui's own `menu::context_menu` (same state, same interaction). Below the row with
/// `row`: `MenuRoot::new` keeps the pointer in `menu_state.rect.min`, and the menu's area is
/// drawn at `rect.min`, so rewriting it moves the menu. Shift+F10 opens it while `resp` has the
/// focus or is the `target`.
pub(super) fn show(resp: &Response, target: bool, row: bool, add_contents: impl FnOnce(&mut Ui)) -> Option<InnerResponse<()>> {
    let ctx = &resp.ctx;
    menu_keys::capture(ctx);
    let id = Id::new(CONTEXT_MENU_ID);
    let mut bar = BarState::load(ctx, id);
    let was_ours = bar.as_ref().is_some_and(|r| r.id == resp.id);
    let rect = ctx.layer_transform_to_global(resp.layer_id).map_or(resp.rect, |t| t * resp.rect);
    let keyed = !was_ours && (target || resp.has_focus()) && menu_keys::take_menu_key(ctx);
    if keyed {
        **bar = Some(MenuRoot::new(pos2(rect.left(), rect.bottom() + GAP), resp.id));
    }
    menu_keys::opener(ctx, resp.has_focus().then_some(resp.id));
    super::menu_open::guarded(ctx, &mut bar, |bar| MenuRoot::context_click_interaction(resp, bar));
    if let Some(root) = bar.as_ref().filter(|r| r.id == resp.id && row) {
        let screen = ctx.screen_rect();
        let mut ms = root.menu_state.write();
        ms.rect = placed(ms.rect, rect, screen, !was_ours || resp.secondary_clicked());
    }
    let inner = bar.show(resp, |ui| super::menu::styled(ui, add_contents));
    bar.store(ctx, id);
    inner
}

/// Where the menu goes. `menu` is its last rect (on the frame it opens: the pointer, sized
/// zero). Just opened, it starts below the row at the pointer's x; once its height is known it
/// moves above the row when it fits there but not below.
fn placed(menu: Rect, row: Rect, screen: Rect, just_opened: bool) -> Rect {
    if just_opened {
        let x = menu.min.x.max(row.left());
        return Rect::from_min_size(pos2(x, row.bottom() + GAP), Vec2::ZERO);
    }
    let h = menu.height();
    let below = row.bottom() + GAP;
    let above = row.top() - GAP - h;
    let y = if h > 0.0 && below + h > screen.bottom() && above >= screen.top() { above } else { below };
    Rect::from_min_size(pos2(menu.min.x, y), menu.size())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
        Rect::from_min_max(pos2(x0, y0), pos2(x1, y1))
    }

    #[test]
    fn opens_below_the_row_at_the_pointer() {
        let row = rect(10.0, 100.0, 400.0, 140.0);
        let r = placed(Rect::from_min_size(pos2(200.0, 120.0), Vec2::ZERO), row, rect(0.0, 0.0, 800.0, 600.0), true);
        assert_eq!(r.min, pos2(200.0, 142.0));
        // Never left of the row.
        let r = placed(Rect::from_min_size(pos2(4.0, 120.0), Vec2::ZERO), row, rect(0.0, 0.0, 800.0, 600.0), true);
        assert_eq!(r.min.x, 10.0);
    }

    #[test]
    fn flips_above_when_it_does_not_fit_below() {
        let screen = rect(0.0, 0.0, 800.0, 600.0);
        let row = rect(10.0, 500.0, 400.0, 540.0);
        let menu = Rect::from_min_size(pos2(200.0, 542.0), Vec2::new(180.0, 200.0));
        let r = placed(menu, row, screen, false);
        assert_eq!(r.max.y, 498.0);
        assert_eq!(r.min.x, 200.0);
        // Fits below: stays there.
        let row = rect(10.0, 100.0, 400.0, 140.0);
        assert_eq!(placed(menu, row, screen, false).min.y, 142.0);
        // Fits neither way: below, and egui keeps it on screen.
        let tall = Rect::from_min_size(pos2(200.0, 0.0), Vec2::new(180.0, 590.0));
        let row = rect(10.0, 300.0, 400.0, 340.0);
        assert_eq!(placed(tall, row, screen, false).min.y, 342.0);
    }
}
