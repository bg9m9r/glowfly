//! The font preview's render: the bundled fonts' names, formats and white specimens, a
//! collection's first face, broken and foreign files, and the pure parts (the specimen's
//! lines, escaping, name cleaning and the path check).

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
        let mut copy = font.to_vec();
        let tables = u16::from_be_bytes([copy[4], copy[5]]) as usize;
        for t in 0..tables {
            let at = 12 + 16 * t + 8;
            let offset = u32::from_be_bytes(copy[at..at + 4].try_into().unwrap());
            copy[at..at + 4].copy_from_slice(&(offset + base).to_be_bytes());
        }
        out.extend_from_slice(&copy);
    }
    out
}

#[test]
fn a_collection_shows_its_first_face_and_counts_them() {
    let r = render(&ttc(&[NOTO_SANS, JETBRAINS_MONO])).unwrap();
    assert_eq!((r.face.family.as_str(), r.face.faces), ("Noto Sans", 2));
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
    assert_eq!(texts, ["Noto Sans Regular", SAMPLE, DIGITS]);
    assert_eq!((lines[0].0, lines[1].0), (NAME_PT, SAMPLE_PT));
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
