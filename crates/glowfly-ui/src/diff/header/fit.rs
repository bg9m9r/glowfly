//! How the diff header fits its one row: given every part's width, which parts give way and in
//! what order. Pure arithmetic, so the order is tested without egui.
//!
//! The order: 1) the labelled file actions fold into one menu; 2) the View button shortens to
//! "View"; 3) the folder is cut from the left, down to nothing; 4) the nav counter drops; 5) the
//! context chip drops; 6) the status chip drops; 7) last, the file name is cut. Each step
//! happens only when the ones before it weren't enough; below the last the row is clipped.

/// The widths of the header's parts (0 for a part that isn't there). The right-hand groups'
/// widths include the spacing inside them; `divider` is a divider with its margins, `gap` the
/// space between the left-hand parts.
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
    pub actions_labelled: f32,
    pub actions_folded: f32,
    /// The diff tool control with the space before it (when other actions precede it).
    pub diff_tool: f32,
    pub primary: f32,
    pub close: f32,
    pub divider: f32,
    pub gap: f32,
}

/// What the header draws.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Plan {
    /// The labelled file actions are one ⋯ menu.
    pub fold_actions: bool,
    /// The View button reads "View".
    pub view_short: bool,
    /// The room for the folder (cut from the left to fit); 0 hides it.
    pub folder_w: f32,
    pub counter: bool,
    pub context: bool,
    pub status: bool,
    /// The room for the name; below its width it is cut from the right.
    pub name_w: f32,
    /// The right-hand groups' total width, dividers included.
    pub right_width: f32,
}

/// The least space kept between the left-hand parts and the right-hand groups.
pub(super) const MIN_GAP: f32 = 16.0;
/// A folder cut narrower than this is dropped instead.
const MIN_FOLDER: f32 = 28.0;

impl Plan {
    /// The left-hand parts' width under this plan.
    fn left_width(&self, w: &Widths) -> f32 {
        let mut x = w.icon + w.gap + self.folder_w + self.name_w;
        if self.status && w.status > 0.0 {
            x += w.gap + w.status;
        }
        if self.context && w.context > 0.0 {
            x += w.gap + w.context;
        }
        x
    }

    fn right(&self, w: &Widths) -> f32 {
        let nav = if w.nav_arrows > 0.0 { w.nav_arrows + if self.counter { w.counter } else { 0.0 } } else { 0.0 };
        let view = if self.view_short { w.view_short } else { w.view_full };
        let labelled = if self.fold_actions { w.actions_folded } else { w.actions_labelled };
        let actions = labelled + w.diff_tool;
        let groups = [nav, view, actions, w.primary, w.close];
        let present = groups.iter().filter(|g| **g > 0.0).count();
        groups.iter().sum::<f32>() + present.saturating_sub(1) as f32 * w.divider
    }

    fn fits(&mut self, w: &Widths, avail: f32) -> bool {
        self.right_width = self.right(w);
        self.left_width(w) + MIN_GAP + self.right_width <= avail
    }
}

/// The plan for a header `avail` wide (inside its side padding).
pub(super) fn plan(w: &Widths, avail: f32) -> Plan {
    let mut p = Plan {
        fold_actions: false,
        view_short: false,
        folder_w: w.folder,
        counter: true,
        context: true,
        status: true,
        name_w: w.name,
        right_width: 0.0,
    };
    if p.fits(w, avail) {
        return p;
    }
    if w.actions_labelled > 0.0 {
        p.fold_actions = true;
        if p.fits(w, avail) {
            return p;
        }
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
            actions_labelled: 260.0,
            actions_folded: 28.0,
            diff_tool: 32.0,
            primary: 90.0,
            close: 28.0,
            divider: 17.0,
            gap: 8.0,
        }
    }

    /// The narrowing steps a plan has taken, in the documented order.
    fn steps(p: &Plan, w: &Widths) -> [bool; 7] {
        [p.fold_actions, p.view_short, p.folder_w < w.folder, !p.counter, !p.context, !p.status, p.name_w < w.name]
    }

    #[test]
    fn everything_fits_when_wide() {
        let w = widths();
        let p = plan(&w, 1600.0);
        assert_eq!(steps(&p, &w), [false; 7]);
        assert_eq!(p.folder_w, w.folder);
        assert_eq!(p.right_width, 60.0 + 40.0 + 130.0 + 260.0 + 32.0 + 90.0 + 28.0 + 4.0 * 17.0);
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
            if taken < 7 {
                assert!(p.left_width(&w) + MIN_GAP + p.right_width <= avail + 0.01, "{avail}: fits until the last step");
            }
            if p.folder_w > 0.0 && p.folder_w < w.folder {
                seen_cut_folder = true;
                assert!(p.counter && p.context && p.status && p.name_w == w.name, "{avail}: a cut folder before anything else");
            }
        }
        assert_eq!(last, 7, "the name is cut last");
        assert!(seen_cut_folder, "the folder is cut before it is dropped");
    }

    #[test]
    fn absent_parts_take_no_room() {
        let w = Widths { actions_labelled: 0.0, actions_folded: 0.0, view_full: 0.0, view_short: 0.0, primary: 0.0, ..widths() };
        let p = plan(&w, 1600.0);
        assert_eq!(p.right_width, 60.0 + 40.0 + 32.0 + 28.0 + 2.0 * 17.0);
        // Nothing to fold or shorten: the folder is the first to give way.
        let p = plan(&w, 400.0);
        assert!(!p.fold_actions && !p.view_short && p.folder_w < w.folder);
    }
}
