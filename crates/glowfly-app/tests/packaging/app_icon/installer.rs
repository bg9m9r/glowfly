//! The Windows installer's artwork, drawn from the icon: the banner across the top of its inner
//! pages and the picture behind its first and last pages. WiX's dialogs set the sizes; their
//! text goes on white, so the icon sits where no text does.

use super::draw;

/// The banner: 493 x 58, the icon at the right end.
pub const BANNER: (u32, u32) = (493, 58);
/// The welcome and finish picture: 493 x 312, the icon on a cream panel down the left side.
pub const DIALOG: (u32, u32) = (493, 312);

const WHITE: [f64; 3] = [255.0, 255.0, 255.0];
const CREAM: [f64; 3] = [0xF4 as f64, 0xF1 as f64, 0xE8 as f64];
/// The panel's width: the dialogs' text starts at about 180 px.
const PANEL: u32 = 164;

/// `width` x `height` RGB, `fill(x, y)` behind the icon (`icon` px, top-left at `at`).
fn compose(width: u32, height: u32, fill: impl Fn(u32, u32) -> [f64; 3], icon: u32, at: (u32, u32)) -> Vec<u8> {
    let art = draw::render(icon);
    let mut out = Vec::with_capacity((width * height * 3) as usize);
    for y in 0..height {
        for x in 0..width {
            let mut c = fill(x, y);
            let (ix, iy) = (x.wrapping_sub(at.0), y.wrapping_sub(at.1));
            if ix < icon && iy < icon {
                let p = &art[((iy * icon + ix) * 4) as usize..][..4];
                let a = p[3] as f64 / 255.0;
                c = [0, 1, 2].map(|i| c[i] + (p[i] as f64 - c[i]) * a);
            }
            out.extend(c.map(|v| (v + 0.5).floor().clamp(0.0, 255.0) as u8));
        }
    }
    out
}

pub fn banner() -> Vec<u8> {
    let (w, h) = BANNER;
    let icon = 44;
    compose(w, h, |_, _| WHITE, icon, (w - icon - 8, (h - icon) / 2))
}

pub fn dialog() -> Vec<u8> {
    let (w, h) = DIALOG;
    let icon = 112;
    compose(w, h, |x, _| if x < PANEL { CREAM } else { WHITE }, icon, ((PANEL - icon) / 2, 64))
}
