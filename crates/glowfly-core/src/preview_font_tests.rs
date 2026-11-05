//! The font preview's render: the bundled fonts' names, formats and white specimens (a capped
//! one keeping its size in points), a collection's first face (and only that one loaded),
//! broken and foreign files, and the pure parts (the specimen's lines, escaping, name cleaning
//! and the path check).

use super::*;
use crate::bundled_fonts::{JETBRAINS_MONO, NOTO_SANS, NOTO_SANS_SEMIBOLD};

fn alpha_sum(r: &FontRender) -> u64 {
    r.image.rgba.chunks(4).map(|p| p[3] as u64).sum()
}

#[test]
fn a_truetype_font_draws_its_specimen_in_white() {
    let r = render(NOTO_SANS).unwrap();
    assert_eq!(r.face, FontFace { family: "Noto Sans".into(), style: "Regular".into(), faces: 1 });
    assert_eq!(r.image.format, "TrueType");
    let [w, h] = r.image.shown;
    assert!(w > 0 && h > 0 && w <= SHOWN_MAX as usize && h <= SHOWN_MAX as usize, "{w}x{h}");
    assert_eq!(r.image.rgba.len(), w * h * 4);
    assert_eq!(r.image.width, (w as f32 / PX_PER_POINT).ceil() as u32);
    assert_eq!(r.image.height, (h as f32 / PX_PER_POINT).ceil() as u32);
    assert!(r.image.rgba.chunks(4).any(|p| p[3] > 0), "something is drawn");
    assert!(r.image.rgba.chunks(4).filter(|p| p[3] > 0).all(|p| p[..3] == [255, 255, 255]), "white glyphs, for tinting");
}

#[test]
fn each_side_is_set_in_its_own_font() {
    let regular = render(NOTO_SANS).unwrap();
    let semibold = render(NOTO_SANS_SEMIBOLD).unwrap();
    assert_eq!(semibold.face.style, "SemiBold");
    assert_eq!(render(JETBRAINS_MONO).unwrap().face.family, "JetBrains Mono");
    assert!(alpha_sum(&semibold) > alpha_sum(&regular), "the bolder face inks more");
}

/// A copy of the single font `font` with its table offsets moved by `base`, for placing it at
/// `base` in a collection.
fn moved_to(font: &[u8], base: u32) -> Vec<u8> {
    let mut copy = font.to_vec();
    let tables = u16::from_be_bytes([copy[4], copy[5]]) as usize;
    for t in 0..tables {
        let at = 12 + 16 * t + 8;
        let offset = u32::from_be_bytes(copy[at..at + 4].try_into().unwrap());
        copy[at..at + 4].copy_from_slice(&(offset + base).to_be_bytes());
    }
    copy
}

/// A TrueType collection holding `fonts`, each one's table offsets moved to where it lands.
fn ttc(fonts: &[&[u8]]) -> Vec<u8> {
    let header = 12 + 4 * fonts.len();
    let mut out = Vec::new();
    out.extend_from_slice(b"ttcf");
    out.extend_from_slice(&[0, 1, 0, 0]);
    out.extend_from_slice(&(fonts.len() as u32).to_be_bytes());
    out.resize(header, 0);
    for (i, font) in fonts.iter().enumerate() {
        while out.len() % 4 != 0 {
            out.push(0);
        }
        let base = out.len() as u32;
        out[12 + 4 * i..16 + 4 * i].copy_from_slice(&base.to_be_bytes());
        out.extend_from_slice(&moved_to(font, base));
    }
    out
}

#[test]
fn a_collection_shows_its_first_face_and_counts_them() {
    let r = render(&ttc(&[NOTO_SANS, JETBRAINS_MONO])).unwrap();
    assert_eq!((r.face.family.as_str(), r.face.faces), ("Noto Sans", 2));
}

/// A collection listing `count` faces that are all one copy of `font`, placed after the header.
fn ttc_repeating(font: &[u8], count: usize) -> Vec<u8> {
    let base = (12 + 4 * count as u32).next_multiple_of(4);
    let mut out = b"ttcf\0\x01\0\0".to_vec();
    out.extend_from_slice(&(count as u32).to_be_bytes());
    out.extend(std::iter::repeat_n(base.to_be_bytes(), count).flatten());
    out.resize(base as usize, 0);
    out.extend_from_slice(&moved_to(font, base));
    out
}

#[test]
fn only_the_first_face_of_a_collection_is_loaded() {
    let bytes = ttc(&[NOTO_SANS, JETBRAINS_MONO, NOTO_SANS_SEMIBOLD]);
    let opts = options(&bytes);
    assert_eq!(opts.fontdb.len(), 1, "one face in the database, not every face of the file");
    assert_eq!(opts.fontdb.faces().next().map(|f| f.index), Some(0));
}

#[test]
fn a_collection_listing_a_huge_number_of_faces_draws_its_first_without_loading_them() {
    let count = 200_000;
    let r = render(&ttc_repeating(NOTO_SANS, count)).unwrap();
    assert_eq!((r.face.family.as_str(), r.face.faces), ("Noto Sans", count as u32));
}

#[test]
fn lines_wider_than_the_canvas_are_drawn_whole() {
    let aspect = |ms: usize| {
        let lines: Vec<(f32, String)> = (0..20).map(|_| (SAMPLE_PT, "M".repeat(ms))).collect();
        let img = rasterize(NOTO_SANS, &lines, "TrueType").unwrap();
        img.shown[0] as f32 / img.shown[1] as f32
    };
    let (wide, twice) = (aspect(300), aspect(600));
    assert!(twice > 1.8 * wide, "both pass the canvas's 4096 width, so twice the letters is twice as wide: {wide} vs {twice}");
}

#[test]
fn a_name_wider_than_the_raster_cap_keeps_its_size_in_points() {
    let lines = specimen(&"W".repeat(40), latin, (0x20..0x7f).collect::<Vec<u32>>());
    let bbox = Tree::from_str(&sample_svg(&lines), &options(NOTO_SANS)).unwrap().root().abs_bounding_box();
    assert!(bbox.width() > 512.0, "wide enough to be capped: {}", bbox.width());
    let img = rasterize(NOTO_SANS, &lines, "TrueType").unwrap();
    assert!((img.width as i64 - bbox.width().ceil() as i64).abs() <= 1, "{} vs {}", img.width, bbox.width());
    assert!((img.height as i64 - bbox.height().ceil() as i64).abs() <= 1, "{} vs {}", img.height, bbox.height());
    assert!(img.shown[0] <= SHOWN_MAX as usize, "{:?}", img.shown);
    assert!(img.shown[0] < (bbox.width() * PX_PER_POINT) as usize, "it really was capped: {:?}", img.shown);
}

#[test]
fn broken_and_foreign_files_are_unreadable_without_a_panic() {
    let mut woff = b"wOFF\0\x01\0\0".to_vec();
    woff.resize(256, 0);
    for bytes in [&b"not a font at all, just text"[..], b"", &NOTO_SANS[..1024], &woff] {
        assert_eq!(render(bytes).unwrap_err(), FontError::Unreadable, "{} bytes", bytes.len());
    }
}

fn latin(c: char) -> bool {
    c.is_ascii_graphic() || c == ' '
}

#[test]
fn a_latin_font_gets_its_name_the_sample_and_the_digits() {
    let lines = specimen("Noto Sans Regular", latin, (0x20..0x7f).collect::<Vec<u32>>());
    let texts: Vec<&str> = lines.iter().map(|l| l.1.as_str()).collect();
    assert_eq!(texts, ["Noto Sans Regular", SAMPLE[0], SAMPLE[1], DIGITS]);
    assert_eq!(SAMPLE, ["The quick brown fox", "jumps over the lazy dog"]);
    let sizes: Vec<f32> = lines.iter().map(|l| l.0).collect();
    assert_eq!(sizes, [NAME_PT, SAMPLE_PT, SAMPLE_PT, SAMPLE_PT]);
    assert_eq!(NAME_PT, 24.0);
}

#[test]
fn an_icon_font_shows_its_own_characters_and_no_name() {
    let icons = |c: char| ('\u{e000}'..='\u{e040}').contains(&c);
    let lines = specimen("Icons", icons, 0xe000..=0xe040);
    assert!(lines.iter().all(|l| l.0 == SAMPLE_PT), "no name line");
    let chars: Vec<char> = lines.iter().flat_map(|l| l.1.chars()).collect();
    assert_eq!(chars.len(), OWN_CHARS, "no spaces: the font has none");
    assert_eq!(chars[0], '\u{e000}');
    assert!(chars.iter().all(|&c| icons(c)));
}

#[test]
fn a_font_mapping_nothing_has_no_lines() {
    assert!(specimen("Empty", |_| false, std::iter::empty()).is_empty());
}

#[test]
fn text_is_escaped_for_xml() {
    assert_eq!(xml_text("<b>&\"'\u{1}x"), "&lt;b&gt;&amp;&quot;&apos;x");
}

#[test]
fn names_are_cleaned_and_capped() {
    assert_eq!(clean_name("  Noto\u{0} Sans\n "), "Noto Sans");
    let long = "x".repeat(100);
    let cut = clean_name(&long);
    assert_eq!(cut.chars().count(), NAME_MAX);
    assert!(cut.ends_with('\u{2026}'));
}

#[test]
fn font_paths_by_extension() {
    for p in ["a.ttf", "dir/B.OTF", "c.ttc", "d.otc"] {
        assert!(is_font_path(p), "{p}");
    }
    for p in ["a.woff", "a.woff2", "ttf", ".ttf", "dir.ttf/readme", "a.ttf.txt"] {
        assert!(!is_font_path(p), "{p}");
    }
}
