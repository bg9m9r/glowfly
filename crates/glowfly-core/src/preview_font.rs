//! Rendering a font file (TrueType, OpenType or a collection of them) for the diff viewer's
//! before/after preview: a short specimen (the face's name, a pangram and the digits, or for a
//! font without Latin letters the first characters it has) set in that font alone and drawn
//! by resvg into a `PreviewImage`, with the face's names read by skrifa. The glyphs are white
//! on transparent so the view can tint them to its text colour. Each render builds its own
//! font database holding only this font, pins every text run to it and has no fallback: no
//! system or bundled font is ever read. Names from the file are cleaned and escaped before
//! they go into the generated SVG, and a panic in the parsers is caught and reported as an
//! unreadable font. CPU-only work: call it from a worker thread.

use crate::preview_image::{PreviewImage, SHOWN_MAX};
use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{FontResolver, ImageHrefResolver, Options, Tree, fontdb};
use skrifa::raw::FileRef;
use skrifa::string::StringId;
use skrifa::{FontRef, MetadataProvider, Tag};
use std::panic::AssertUnwindSafe;
use std::sync::Arc;

/// The largest font file the preview renders; a larger one shows its size only.
pub const FONT_MAX_BYTES: u64 = 16 << 20;

/// The extensions `is_font_path` takes, compared ignoring ASCII case.
const FONT_EXTS: [&str; 4] = ["ttf", "otf", "ttc", "otc"];

/// The specimen is drawn at this many pixels per point, so it stays sharp on 2x screens.
pub const PX_PER_POINT: f32 = 2.0;

/// The size of the face's name line, in points.
const NAME_PT: f32 = 28.0;
/// The size of the sample lines, in points.
const SAMPLE_PT: f32 = 18.0;
/// The extra room under the name line, in points.
const NAME_GAP: f32 = 6.0;
/// The sample line, and the line of digits and signs under it.
const SAMPLE: &str = "The quick brown fox jumps over the lazy dog";
const DIGITS: &str = "0123456789 &@?!";
/// How many of its own characters a font without Latin letters shows, and how many a line.
const OWN_CHARS: usize = 32;
const OWN_PER_LINE: usize = 16;
/// The longest name kept from the file, in characters.
const NAME_MAX: usize = 64;

/// The face a font render shows: its family and style names (cleaned, possibly empty) and the
/// number of faces in the file (1 unless it is a collection; the first one is shown).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontFace {
    pub family: String,
    pub style: String,
    pub faces: u32,
}

/// A font drawn: the specimen and the face it was set in. `image.format` is "TrueType" or
/// "OpenType"; `image.width` and `image.height` are the specimen's size in points (its pixels,
/// `shown`, are `PX_PER_POINT` times that or fewer).
#[derive(Clone, Debug)]
pub struct FontRender {
    pub image: PreviewImage,
    pub face: FontFace,
}

/// Why a font couldn't be drawn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FontError {
    /// Not a font that can be read (corrupt, truncated, another container such as WOFF).
    Unreadable,
    /// The font has no outlines for any character of the specimen.
    NoGlyphs,
    /// It was still drawing when the runner stopped waiting, or a newer request took its place.
    Abandoned,
}

/// `bytes` read as a font and its specimen drawn, its longer side at most `SHOWN_MAX` pixels.
pub fn render(bytes: &[u8]) -> Result<FontRender, FontError> {
    std::panic::catch_unwind(AssertUnwindSafe(|| draw(bytes))).unwrap_or_else(|_| {
        tracing::debug!("font preview: the parser panicked");
        Err(FontError::Unreadable)
    })
}

/// Whether `path` names a font file, by its `.ttf`, `.otf`, `.ttc` or `.otc` extension in any case.
pub fn is_font_path(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.rsplit_once('.').is_some_and(|(stem, ext)| !stem.is_empty() && FONT_EXTS.iter().any(|e| ext.eq_ignore_ascii_case(e)))
}

fn draw(bytes: &[u8]) -> Result<FontRender, FontError> {
    let unreadable = |e: &dyn std::fmt::Display| {
        tracing::debug!("font preview: {e}");
        FontError::Unreadable
    };
    let faces = match FileRef::new(bytes).map_err(|e| unreadable(&e))? {
        FileRef::Font(_) => 1,
        FileRef::Collection(c) => c.len(),
    };
    if faces == 0 {
        return Err(FontError::Unreadable);
    }
    let font = FontRef::from_index(bytes, 0).map_err(|e| unreadable(&e))?;
    // A table running past the end is a truncated file (skrifa reads tables lazily).
    let records = font.table_directory().table_records();
    if records.is_empty() || records.iter().any(|r| u64::from(r.offset()) + u64::from(r.length()) > bytes.len() as u64) {
        return Err(FontError::Unreadable);
    }
    let (family, style) = names(&font);
    let cff = [*b"CFF ", *b"CFF2"].iter().any(|t| font.table_data(Tag::new(t)).is_some());
    let format = if cff { "OpenType" } else { "TrueType" };
    let charmap = font.charmap();
    let name = [family.as_str(), style.as_str()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" ");
    let lines = specimen(&name, |c| charmap.map(c).is_some(), charmap.mappings().map(|(c, _)| c));
    if lines.is_empty() {
        return Err(FontError::NoGlyphs);
    }
    let image = rasterize(bytes, &lines, format)?;
    Ok(FontRender { image, face: FontFace { family, style, faces } })
}

/// The face's family and style names, cleaned; empty where the font has none.
fn names(font: &FontRef) -> (String, String) {
    let first = |ids: &[StringId]| {
        ids.iter()
            .find_map(|&id| {
                font.localized_strings(id).english_or_first().map(|s| clean_name(&s.chars().collect::<String>())).filter(|s| !s.is_empty())
            })
            .unwrap_or_default()
    };
    let family = first(&[StringId::TYPOGRAPHIC_FAMILY_NAME, StringId::FAMILY_NAME, StringId::POSTSCRIPT_NAME]);
    let style = first(&[StringId::TYPOGRAPHIC_SUBFAMILY_NAME, StringId::SUBFAMILY_NAME]);
    (family, style)
}

/// A name from the file made safe to show: control characters dropped, trimmed, and at most
/// `NAME_MAX` characters (the last one an ellipsis when cut).
fn clean_name(raw: &str) -> String {
    let kept: String = raw.chars().filter(|c| !c.is_control()).collect();
    let kept = kept.trim();
    if kept.chars().count() <= NAME_MAX {
        return kept.to_owned();
    }
    let mut cut: String = kept.chars().take(NAME_MAX - 1).collect();
    cut.truncate(cut.trim_end().len());
    cut.push('\u{2026}');
    cut
}

/// The specimen's lines, each a size in points and its text: `name` when the font has every
/// one of its characters, then the sample and digits when it has at least half of the sample's
/// letters, else the first `OWN_CHARS` printable characters it maps (`mapped`, in cmap order).
/// Empty when the font maps nothing to show.
fn specimen(name: &str, maps: impl Fn(char) -> bool, mapped: impl IntoIterator<Item = u32>) -> Vec<(f32, String)> {
    let mut lines = Vec::new();
    if !name.is_empty() && name.chars().filter(|c| *c != ' ').all(&maps) {
        lines.push((NAME_PT, name.to_owned()));
    }
    let letters: Vec<char> = SAMPLE.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.iter().filter(|&&c| maps(c)).count() * 2 >= letters.len() {
        lines.push((SAMPLE_PT, SAMPLE.to_owned()));
        lines.push((SAMPLE_PT, DIGITS.to_owned()));
        return lines;
    }
    let own: Vec<char> =
        mapped.into_iter().filter_map(char::from_u32).filter(|c| !c.is_control() && !c.is_whitespace()).take(OWN_CHARS).collect();
    let gap = if maps(' ') { " " } else { "" };
    for chunk in own.chunks(OWN_PER_LINE) {
        lines.push((SAMPLE_PT, chunk.iter().map(char::to_string).collect::<Vec<_>>().join(gap)));
    }
    lines
}

/// The specimen as SVG: one `<text>` in the family "sample", a line per `<tspan>`, white. A
/// line sits its predecessor's descent plus its own ascent below it, with `NAME_GAP` more
/// under a larger line (the name).
fn sample_svg(lines: &[(f32, String)]) -> String {
    let mut body = String::new();
    let mut baseline = 0.0;
    let mut prev: Option<f32> = None;
    for (pt, text) in lines {
        baseline += match prev {
            None => pt * 1.1,
            Some(p) => p * 0.35 + pt * 1.05 + if p > *pt { NAME_GAP } else { 0.0 },
        };
        prev = Some(*pt);
        body.push_str(&format!(r#"<tspan x="0" y="{baseline}" font-size="{pt}">{}</tspan>"#, xml_text(text)));
    }
    let total = (baseline + lines.last().map_or(0.0, |l| l.0 * 0.5)).ceil().max(1.0);
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="4096" height="{total}"><text xml:space="preserve" font-family="sample" fill="#fff">{body}</text></svg>"##
    )
}

/// `s` as XML character data: `& < > " '` escaped, characters XML 1.0 forbids dropped.
fn xml_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\t' | '\n' | '\r' => out.push(c),
            c if (c as u32) < 0x20 || matches!(c as u32, 0xfffe | 0xffff) => {}
            c => out.push(c),
        }
    }
    out
}

/// The options a render uses: a database holding only face 0 of `bytes`, every text run set
/// in it, no fallback font and no images resolved. The face is pushed as it is rather than
/// loaded, because loading parses every face of a collection, and a crafted one can list
/// millions; the resolver picks it by id, so it needs no names or style here.
fn options(bytes: &[u8]) -> Options<'static> {
    let mut db = fontdb::Database::new();
    let id = db.push_face_info(fontdb::FaceInfo {
        id: fontdb::ID::dummy(),
        source: fontdb::Source::Binary(Arc::new(bytes.to_vec())),
        index: 0,
        families: Vec::new(),
        post_script_name: String::new(),
        style: fontdb::Style::Normal,
        weight: fontdb::Weight::NORMAL,
        stretch: fontdb::Stretch::Normal,
        monospaced: false,
    });
    Options {
        resources_dir: None,
        font_family: "sample".to_owned(),
        fontdb: Arc::new(db),
        font_resolver: FontResolver { select_font: Box::new(move |_, _| Some(id)), select_fallback: Box::new(|_, _, _| None) },
        image_href_resolver: ImageHrefResolver { resolve_data: Box::new(|_, _, _| None), resolve_string: Box::new(|_, _| None) },
        ..Options::default()
    }
}

/// `lines` set in face 0 of `bytes` and drawn at `PX_PER_POINT` (less when that would pass
/// `SHOWN_MAX`), cropped to the glyphs.
fn rasterize(bytes: &[u8], lines: &[(f32, String)], format: &'static str) -> Result<PreviewImage, FontError> {
    let opts = options(bytes);
    let tree = Tree::from_str(&sample_svg(lines), &opts).map_err(|e| {
        tracing::debug!("font preview: {e}");
        FontError::Unreadable
    })?;
    let bbox = tree.root().abs_bounding_box();
    let (bw, bh) = (bbox.width(), bbox.height());
    if !tree.root().has_children() || !(bw.is_finite() && bh.is_finite()) || bw <= 0.0 || bh <= 0.0 {
        return Err(FontError::NoGlyphs);
    }
    let max = SHOWN_MAX as f32;
    let scale = PX_PER_POINT.min(max / bw).min(max / bh);
    let px = |v: f32| ((v * scale).ceil() as u32).clamp(1, SHOWN_MAX);
    let mut pixmap = Pixmap::new(px(bw), px(bh)).ok_or(FontError::NoGlyphs)?;
    let transform = Transform::from_row(scale, 0.0, 0.0, scale, -bbox.x() * scale, -bbox.y() * scale);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    let shown = [pixmap.width() as usize, pixmap.height() as usize];
    let points = |v: f32| ((v * scale / PX_PER_POINT).ceil() as u32).max(1);
    let rgba = crate::preview_svg::unmultiplied(&pixmap);
    Ok(PreviewImage { width: points(bw), height: points(bh), format, shown, rgba: Arc::new(rgba) })
}

#[cfg(test)]
#[path = "preview_font_tests.rs"]
mod tests;
