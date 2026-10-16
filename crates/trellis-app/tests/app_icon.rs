//! The app icon in `assets/icon/`: `UPDATE_ICONS=1 cargo test -p trellis --test app_icon`
//! renders it (`app_icon/draw.rs`) and rewrites the PNGs, `trellis.ico` and `trellis.icns`
//! (`app_icon/containers.rs`). Without the variable it checks that every checked-in image
//! matches the drawing, within 2 per channel (PNG encoders and float edge cases may differ a
//! little between systems).

#[path = "app_icon/containers.rs"]
mod containers;
#[path = "app_icon/draw.rs"]
mod draw;

use std::collections::HashMap;
use std::path::PathBuf;

/// The PNG files, by size.
const PNG_SIZES: [u32; 9] = [16, 24, 32, 48, 64, 128, 256, 512, 1024];
/// The sizes inside trellis.ico.
const ICO_SIZES: [u32; 8] = [16, 20, 24, 32, 40, 48, 64, 256];
/// The chunks of trellis.icns and the size each holds.
const ICNS_TYPES: [(&str, u32); 10] = [
    ("icp4", 16),
    ("icp5", 32),
    ("ic07", 128),
    ("ic08", 256),
    ("ic09", 512),
    ("ic10", 1024),
    ("ic11", 32),
    ("ic12", 64),
    ("ic13", 256),
    ("ic14", 512),
];
const TOLERANCE: u8 = 2;

fn icon_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/icon")
}

fn png(size: u32, rgba: &[u8]) -> Vec<u8> {
    use image::ImageEncoder;
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new_with_quality(&mut out, image::codecs::png::CompressionType::Best, Default::default())
        .write_image(rgba, size, size, image::ExtendedColorType::Rgba8)
        .expect("encode PNG");
    out
}

fn decode(bytes: &[u8]) -> (u32, Vec<u8>) {
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png).expect("a PNG").to_rgba8();
    assert_eq!(img.width(), img.height(), "square");
    (img.width(), img.into_raw())
}

/// Every size the files need, rendered once.
fn rendered() -> HashMap<u32, Vec<u8>> {
    let mut sizes: Vec<u32> = PNG_SIZES.iter().chain(&ICO_SIZES).chain(ICNS_TYPES.iter().map(|(_, s)| s)).copied().collect();
    sizes.sort_unstable();
    sizes.dedup();
    sizes.into_iter().map(|s| (s, draw::render(s))).collect()
}

fn assert_matches(what: &str, png_bytes: &[u8], size: u32, want: &[u8]) {
    let (w, got) = decode(png_bytes);
    assert_eq!(w, size, "{what}: size");
    let worst = got.iter().zip(want).map(|(a, b)| a.abs_diff(*b)).max().unwrap_or(0);
    assert!(
        worst <= TOLERANCE,
        "{what} differs from the drawing by up to {worst}; run UPDATE_ICONS=1 cargo test -p trellis --test app_icon"
    );
}

#[test]
fn the_app_icon_matches_its_drawing() {
    let images = rendered();
    let dir = icon_dir();
    let ico = || containers::write_ico(&ICO_SIZES.map(|s| (s, png(s, &images[&s]))));
    let icns = || containers::write_icns(&ICNS_TYPES.map(|(t, s)| (t, png(s, &images[&s]))));
    if std::env::var_os("UPDATE_ICONS").is_some() {
        std::fs::create_dir_all(&dir).unwrap();
        for s in PNG_SIZES {
            std::fs::write(dir.join(format!("trellis-{s}.png")), png(s, &images[&s])).unwrap();
        }
        std::fs::write(dir.join("trellis.ico"), ico()).unwrap();
        std::fs::write(dir.join("trellis.icns"), icns()).unwrap();
    }
    let read = |name: &str| std::fs::read(dir.join(name)).unwrap_or_else(|e| panic!("assets/icon/{name}: {e}"));
    for s in PNG_SIZES {
        let name = format!("trellis-{s}.png");
        assert_matches(&name, &read(&name), s, &images[&s]);
    }
    let entries = containers::read_ico(&read("trellis.ico"));
    assert_eq!(entries.iter().map(|(s, _)| *s).collect::<Vec<_>>(), ICO_SIZES);
    for (s, data) in &entries {
        assert_matches(&format!("trellis.ico ({s} px)"), data, *s, &images[s]);
    }
    let chunks = containers::read_icns(&read("trellis.icns"));
    assert_eq!(chunks.iter().map(|(t, _)| t.as_str()).collect::<Vec<_>>(), ICNS_TYPES.map(|(t, _)| t));
    for ((kind, data), (_, s)) in chunks.iter().zip(ICNS_TYPES) {
        assert_matches(&format!("trellis.icns ({kind})"), data, s, &images[&s]);
    }
}

#[test]
fn the_containers_read_back() {
    let a = png(16, &draw::render(16));
    let b = png(20, &draw::render(20));
    let ico = containers::read_ico(&containers::write_ico(&[(16, a.clone()), (256, b.clone())]));
    assert_eq!(ico, vec![(16, a.clone()), (256, b.clone())]);
    let icns = containers::read_icns(&containers::write_icns(&[("icp4", a.clone()), ("ic11", b.clone())]));
    assert_eq!(icns, vec![("icp4".to_owned(), a), ("ic11".to_owned(), b)]);
}

#[test]
fn the_corners_are_transparent_and_the_middle_is_not() {
    for size in [16, 32, 256] {
        let img = draw::render(size);
        assert_eq!(img[3], 0, "{size}: the corner is transparent");
        let mid = ((size / 2 * size + size / 2) * 4 + 3) as usize;
        assert_eq!(img[mid], 255, "{size}: the square is opaque");
    }
}
