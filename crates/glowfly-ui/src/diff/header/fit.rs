//! How the diff header fits its one row: given every part's width, which parts give way and in
//! what order. Pure arithmetic, so the order is tested without egui.
//!
//! The row: the file (icon, folder, name, status chip, context chip), 16 px, the navigation
//! (arrows and counter), a divider, the View button and ⋯; then, at the trailing edge, Stage file
//! and ×. The order things give way: 1) the View button shortens to "View"; 2) the folder is cut
//! from the left, down to nothing; 3) the nav counter drops; 4) the context chip drops; 5) the
//! status chip drops; 6) last, the file name is cut. Each step happens only when the ones before
//! it weren't enough; below the last the row is clipped. ⋯, Stage file and × never give way.

/// The widths of the header's parts (0 for a part that isn't there). The groups' widths include
/// the spacing inside them; `divider` is a divider with its margins, `gap` the space between the
/// file's parts.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Widths {
    pub icon: f32,
    pub folder: f32,
    pub name: f32,
    pub status: f32,
    pub context: f32,
    pub nav_arrows: f32,
    /// The counter with the space before it.
    pub counter: f32,
    pub view_full: f32,
    pub view_short: f32,
    /// The ⋯ of file actions.
    pub more: f32,
    /// Stage file (or Unstage file).
    pub primary: f32,
    pub close: f32,
    pub divider: f32,
    pub gap: f32,
}

/// What the header draws.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Plan {
    /// The View button reads "View".
    pub view_short: bool,
    /// The room for the folder (cut from the left to fit); 0 hides it.
    pub folder_w: f32,
    pub counter: bool,
    pub context: bool,
    pub status: bool,
    /// The room for the name; below its width it is cut from the right.
    pub name_w: f32,
    /// The trailing group's width: Stage file and ×.
    pub right_width: f32,
}

/// The least space kept between the leading parts and the trailing group.
pub(super) const MIN_GAP: f32 = 16.0;
/// Between the file and the navigation.
pub(super) const NAV_GAP: f32 = 16.0;
/// Between the View button and ⋯.
pub(super) const TOOLS_SPACE: f32 = super::controls::SPACE;
/// Between Stage file and ×.
const TRAILING_GAP: f32 = super::actions::TRAILING_GAP;
/// A folder cut narrower than this is dropped instead.
const MIN_FOLDER: f32 = 28.0;

/// The space before the View group: a divider after the navigation, else 16 px after the file.
pub(super) fn tools_lead(w: &Widths) -> f32 {
    if w.nav_arrows > 0.0 { w.divider } else { NAV_GAP }
}

impl Plan {
    /// The file's width under this plan.
    fn file_width(&self, w: &Widths) -> f32 {
        let mut x = w.icon + w.gap + self.folder_w + self.name_w;
        if self.status && w.status > 0.0 {
            x += w.gap + w.status;
        }
        if self.context && w.context > 0.0 {
            x += w.gap + w.context;
        }
        x
    }

    /// The leading parts' width: the file, the navigation, the View button and ⋯.
    fn left_width(&self, w: &Widths) -> f32 {
        let mut x = self.file_width(w);
        if w.nav_arrows > 0.0 {
            x += NAV_GAP + w.nav_arrows + if self.counter { w.counter } else { 0.0 };
        }
        let view = if self.view_short { w.view_short } else { w.view_full };
        if view + w.more > 0.0 {
            x += tools_lead(w) + view + w.more;
            if view > 0.0 && w.more > 0.0 {
                x += TOOLS_SPACE;
            }
        }
        x
    }

    fn right(&self, w: &Widths) -> f32 {
        w.primary + w.close + if w.primary > 0.0 && w.close > 0.0 { TRAILING_GAP } else { 0.0 }
    }

    fn fits(&mut self, w: &Widths, avail: f32) -> bool {
        self.right_width = self.right(w);
        self.left_width(w) + MIN_GAP + self.right_width <= avail
    }
}

/// The plan for a header `avail` wide (inside its side padding).
pub(super) fn plan(w: &Widths, avail: f32) -> Plan {
    let mut p =
        Plan { view_short: false, folder_w: w.folder, counter: true, context: true, status: true, name_w: w.name, right_width: 0.0 };
    if p.fits(w, avail) {
        return p;
    }
    if w.view_short > 0.0 && w.view_short < w.view_full {
        p.view_short = true;
        if p.fits(w, avail) {
            return p;
        }
    }
    if w.folder > 0.0 {
        p.folder_w = 0.0;
        let room = avail - MIN_GAP - p.right(w) - p.left_width(w);
        p.folder_w = if room >= MIN_FOLDER { room.min(w.folder) } else { 0.0 };
        if p.fits(w, avail) {
            return p;
        }
    }
    for drop in [|p: &mut Plan| p.counter = false, |p: &mut Plan| p.context = false, |p: &mut Plan| p.status = false] {
        drop(&mut p);
        if p.fits(w, avail) {
            return p;
        }
    }
    p.name_w = 0.0;
    let room = avail - MIN_GAP - p.right(w) - p.left_width(w);
    p.name_w = room.clamp(0.0, w.name);
    p.right_width = p.right(w);
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    fn widths() -> Widths {
        Widths {
            icon: 16.0,
            folder: 120.0,
            name: 80.0,
            status: 70.0,
            context: 60.0,
            nav_arrows: 60.0,
            counter: 40.0,
            view_full: 130.0,
            view_short: 70.0,
            more: 28.0,
            primary: 90.0,
            close: 28.0,
            divider: 17.0,
            gap: 8.0,
        }
    }

    const FILE: f32 = 16.0 + 8.0 + 120.0 + 80.0 + 8.0 + 70.0 + 8.0 + 60.0;

    /// The narrowing steps a plan has taken, in the documented order.
    fn steps(p: &Plan, w: &Widths) -> [bool; 6] {
        [p.view_short, p.folder_w < w.folder, !p.counter, !p.context, !p.status, p.name_w < w.name]
    }

    #[test]
    fn everything_fits_when_wide() {
        let w = widths();
        let p = plan(&w, 1600.0);
        assert_eq!(steps(&p, &w), [false; 6]);
        assert_eq!(p.folder_w, w.folder);
        assert_eq!(p.right_width, 90.0 + 8.0 + 28.0, "Stage file, 8 px, ×");
        assert_eq!(p.left_width(&w), FILE + 16.0 + 60.0 + 40.0 + 17.0 + 130.0 + 4.0 + 28.0, "the file, 16 px, nav, a divider, View and ⋯");
    }

    #[test]
    fn parts_give_way_in_order() {
        let w = widths();
        let mut last = 0;
        let mut seen_cut_folder = false;
        for avail in (0..=1600).rev().map(|a| a as f32) {
            let p = plan(&w, avail);
            let s = steps(&p, &w);
            // The steps taken are always a prefix of the order.
            let taken = s.iter().take_while(|t| **t).count();
            assert!(s[taken..].iter().all(|t| !t), "{avail}: {s:?}");
            assert!(taken >= last, "{avail}: steps never come back as the header narrows");
            last = taken;
            if taken < 6 {
                assert!(p.left_width(&w) + MIN_GAP + p.right_width <= avail + 0.01, "{avail}: fits until the last step");
            }
            if p.folder_w > 0.0 && p.folder_w < w.folder {
                seen_cut_folder = true;
                assert!(p.counter && p.context && p.status && p.name_w == w.name, "{avail}: a cut folder before anything else");
            }
            assert_eq!(p.right_width, 126.0, "{avail}: Stage file and × never give way");
        }
        assert_eq!(last, 6, "the name is cut last");
        assert!(seen_cut_folder, "the folder is cut before it is dropped");
    }

    #[test]
    fn absent_parts_take_no_room() {
        let w = Widths { more: 0.0, view_full: 0.0, view_short: 0.0, primary: 0.0, ..widths() };
        let p = plan(&w, 1600.0);
        assert_eq!(p.right_width, 28.0, "× alone");
        assert_eq!(p.left_width(&w), FILE + 16.0 + 60.0 + 40.0, "no divider without a View group");
        // Nothing to shorten: the folder is the first to give way.
        let p = plan(&w, 400.0);
        assert!(!p.view_short && p.folder_w < w.folder);
        // Without navigation the View group follows the file after 16 px.
        let w = Widths { nav_arrows: 0.0, counter: 0.0, ..widths() };
        assert_eq!(plan(&w, 1600.0).left_width(&w), FILE + 16.0 + 130.0 + 4.0 + 28.0);
    }
}
