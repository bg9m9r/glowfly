//! The SVG preview's render: size, scale, transparency, text in the bundled fonts, broken and
//! empty files, and hrefs that name files or URLs left out while `data:` images draw.

use super::*;
use crate::testutil::scratch_repo;

fn svg(body: &str) -> Vec<u8> {
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" {body}</svg>"#).into_bytes()
}

/// The pixel at `x`, `y` of the render.
fn at(r: &SvgRender, x: usize, y: usize) -> [u8; 4] {
    let i = (y * r.image.shown[0] + x) * 4;
    r.image.rgba[i..i + 4].try_into().unwrap()
}

#[test]
fn a_small_icon_is_drawn_with_its_longer_side_at_shown_max() {
    let r = render(&svg(r##"width="24" height="12" viewBox="0 0 24 12"><rect width="24" height="12" fill="#ff0000"/>"##)).unwrap();
    assert_eq!((r.image.width, r.image.height, r.image.format), (24, 12, "SVG"));
    assert_eq!(r.image.shown, [SHOWN_MAX as usize, SHOWN_MAX as usize / 2]);
    assert_eq!(r.image.rgba.len(), r.image.shown[0] * r.image.shown[1] * 4);
    assert_eq!(at(&r, 10, 10), [255, 0, 0, 255]);
    assert!(!r.linked);
}

#[test]
fn a_huge_size_is_reported_but_drawn_small() {
    let r = render(&svg(r#"width="100000" height="50000"><rect width="10" height="10"/>"#)).unwrap();
    assert_eq!((r.image.width, r.image.height), (100000, 50000));
    assert_eq!(r.image.shown, [1024, 512]);
}

#[test]
fn an_svg_without_a_size_takes_one_from_its_drawing() {
    // No width, height or viewBox: usvg sizes it to reach the drawing's far edges from the
    // origin (a circle out to 90, 90), within its 100 x 100 default canvas.
    let r = render(&svg(r#"><circle cx="50" cy="50" r="40"/>"#)).unwrap();
    assert_eq!((r.image.width, r.image.height), (90, 90));
    assert_eq!(r.image.shown, [1024, 1024]);
}

#[test]
fn transparency_is_kept_unmultiplied() {
    let r = render(&svg(r##"width="10" height="10"><rect width="5" height="10" fill="#0000ff" fill-opacity="0.5"/>"##)).unwrap();
    let [red, green, blue, alpha] = at(&r, 100, 500);
    assert_eq!((red, green), (0, 0));
    assert!(blue >= 250, "unmultiplied, the blue stays full: {blue}");
    assert!((126..=129).contains(&alpha), "{alpha}");
    assert_eq!(at(&r, 900, 500)[3], 0, "the right half is empty");
}

#[test]
fn text_is_set_in_the_bundled_font() {
    let text =
        render(&svg(r#"width="40" height="20"><text x="2" y="16" font-size="16" font-family="Some Font Nobody Has">Hi</text>"#)).unwrap();
    assert!(text.image.rgba.chunks(4).any(|p| p[3] > 0), "the text is drawn");
    let none = render(&svg(r#"width="40" height="20">"#)).unwrap();
    assert!(none.image.rgba.chunks(4).all(|p| p[3] == 0));
}

#[test]
fn broken_and_empty_files_are_errors() {
    assert_eq!(render(b"<svg").unwrap_err(), SvgError::Unreadable);
    assert_eq!(render(b"not xml at all").unwrap_err(), SvgError::Unreadable);
    assert_eq!(render(&[0xff, 0xfe, 0x00]).unwrap_err(), SvgError::Unreadable);
    assert_eq!(render(b"").unwrap_err(), SvgError::Unreadable);
    assert_eq!(render(&svg(r#"width="0" height="10">"#)).unwrap_err(), SvgError::Unreadable);
}

/// A 1 x 1 opaque green PNG.
fn green_png() -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(1, 1, image::Rgba([0, 255, 0, 255]));
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

#[test]
fn linked_files_and_urls_are_never_read() {
    // A real PNG next to the test's working folder, named by absolute path and by URL: neither
    // is drawn, and the render says files were left out.
    let dir = scratch_repo("svg-preview-linked");
    let file = dir.join("green.png");
    std::fs::write(&file, green_png()).unwrap();
    let path = file.to_string_lossy().replace('\\', "/");
    for href in [path.as_str(), "green.png", "https://example.invalid/green.png", "file:///etc/passwd"] {
        let body = format!(
            r#"width="10" height="10"><image width="10" height="10" xlink:href="{href}"/><image width="10" height="10" href="{href}"/>"#
        );
        let r = render(&svg(&body)).unwrap();
        assert!(r.linked, "{href}");
        assert!(r.image.rgba.chunks(4).all(|p| p[3] == 0), "{href} isn't drawn");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn data_images_inside_the_file_are_drawn() {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(green_png());
    let body =
        format!(r#"width="10" height="10"><image width="10" height="10" preserveAspectRatio="none" href="data:image/png;base64,{b64}"/>"#);
    let r = render(&svg(&body)).unwrap();
    assert!(!r.linked);
    assert_eq!(at(&r, 512, 512), [0, 255, 0, 255]);
}

#[test]
fn svg_paths_by_extension() {
    for p in ["a.svg", "dir/b.SVG", "icons/x.Svg"] {
        assert!(is_svg_path(p), "{p}");
    }
    for p in ["a.svg.txt", "svg", ".svg", "dir/.svg", "a.svgz", "dir.svg/readme"] {
        assert!(!is_svg_path(p), "{p}");
    }
}

/// Asserts `body` is refused as too costly, within a second (a refused drawing never reaches
/// the renderer, so even a loaded CI machine answers at once).
fn refused(body: &str) {
    let start = std::time::Instant::now();
    let r = render(&svg(body));
    assert!(start.elapsed() < std::time::Duration::from_secs(1), "took {:?}", start.elapsed());
    assert_eq!(r.err(), Some(SvgError::TooCostly), "{body:.200}");
}

/// `body` drawn (no time limit: drawing at opt-level 1 on a loaded machine varies).
fn drawn(body: &str) -> SvgRender {
    render(&svg(body)).unwrap_or_else(|e| panic!("{e:?}: {body:.200}"))
}

fn filtered(filter: &str) -> String {
    format!(r#"width="100" height="100"><filter id="f">{filter}</filter><rect width="100" height="100" filter="url(#f)"/>"#)
}

/// `inner` embedded as a `data:` SVG image `size` units square in a 10-unit drawing.
fn embedded(inner: &str, size: u32) -> String {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(svg(inner));
    format!(r#"width="10" height="10"><image width="{size}" height="{size}" href="data:image/svg+xml;base64,{b64}"/>"#)
}

#[test]
fn costly_filters_are_refused_quickly() {
    for f in [
        r#"<feMorphology radius="100000"/>"#,
        r#"<feMorphology operator="dilate" radius="50"/>"#,
        r#"<feTurbulence baseFrequency="0.05" numOctaves="1000000000"/>"#,
        r#"<feTurbulence baseFrequency="0.05" numOctaves="11"/>"#,
    ] {
        refused(&filtered(f));
    }
    // A large convolution kernel costs its area for every pixel.
    let kernel = vec!["1"; 99 * 99].join(" ");
    refused(&filtered(&format!(r#"<feConvolveMatrix order="99" kernelMatrix="{kernel}"/>"#)));
    // Too many primitives, even cheap ones.
    refused(&filtered(&r#"<feOffset dx="1"/>"#.repeat(200)));
    // A merge draws the whole region once for each of its inputs.
    refused(&filtered(&format!("<feMerge>{}</feMerge>", "<feMergeNode/>".repeat(5000))));
    // A filter too costly at full size draws at half the size instead.
    assert_eq!(drawn(&filtered(r#"<feMorphology radius="1"/>"#)).image.shown, [512, 512]);
    // Modest filters still draw.
    for f in [
        r#"<feMorphology radius="0.2"/>"#,
        r#"<feTurbulence baseFrequency="0.05" numOctaves="4"/>"#,
        r#"<feGaussianBlur stdDeviation="5"/>"#,
        "<feMerge><feMergeNode/><feMergeNode/></feMerge>",
    ] {
        drawn(&filtered(f));
    }
}

#[test]
fn an_embedded_svg_is_measured_at_the_size_it_is_drawn() {
    let costly = r#"width="100" height="100"><filter id="f"><feMorphology radius="100000"/></filter><rect width="100" height="100" filter="url(#f)"/>"#;
    refused(&embedded(costly, 10));
    // A 1-unit drawing whose half-unit radius looks harmless, scaled up into a 10-unit image
    // drawn 1024 px wide: the radius is about 512 px.
    let tiny = r#"width="1" height="1"><filter id="f"><feMorphology radius="0.5"/></filter><rect width="1" height="1" filter="url(#f)"/>"#;
    refused(&embedded(tiny, 10));
}

#[test]
fn a_costly_filter_at_the_end_of_a_mask_chain_is_refused() {
    let body = r#"width="100" height="100">
      <filter id="f"><feMorphology radius="100000"/></filter>
      <mask id="m3"><rect width="100" height="100" fill="white" filter="url(#f)"/></mask>
      <mask id="m2" mask="url(#m3)"><rect width="100" height="100" fill="white"/></mask>
      <mask id="m1" mask="url(#m2)"><rect width="100" height="100" fill="white"/></mask>
      <rect width="100" height="100" mask="url(#m1)"/>"#;
    refused(body);
    // A long chain holds a layer for every link.
    let mut chain = String::from(r#"width="100" height="100"><mask id="m0"><rect width="100" height="100" fill="white"/></mask>"#);
    for i in 1..40 {
        chain += &format!(r#"<mask id="m{i}" mask="url(#m{})"><rect width="100" height="100" fill="white"/></mask>"#, i - 1);
    }
    chain += r#"<rect width="100" height="100" mask="url(#m39)"/>"#;
    refused(&chain);
    let mut clips = String::from(r#"width="100" height="100"><clipPath id="c0"><rect width="100" height="100"/></clipPath>"#);
    for i in 1..40 {
        clips += &format!(r#"<clipPath id="c{i}" clip-path="url(#c{})"><rect width="100" height="100"/></clipPath>"#, i - 1);
    }
    clips += r#"<rect width="100" height="100" clip-path="url(#c39)"/>"#;
    refused(&clips);
}

#[test]
fn use_fan_out_and_huge_pattern_tiles_are_refused() {
    // Six levels of ten <use> each: a million full-canvas rectangles after expansion (usvg may
    // stop at its own node limit first; either way nothing is drawn).
    let mut body = String::from(r#"width="100" height="100"><defs><rect id="u0" width="100" height="100"/>"#);
    for i in 1..=6 {
        body += &format!(r##"<g id="u{i}">{}</g>"##, format!(r##"<use href="#u{}"/>"##, i - 1).repeat(10));
    }
    body += r##"</defs><use href="#u6"/>"##;
    let start = std::time::Instant::now();
    assert!(render(&svg(&body)).is_err(), "a million full-canvas rectangles");
    assert!(start.elapsed() < std::time::Duration::from_secs(5), "took {:?}", start.elapsed());
    // A 20-unit tile in a 1-unit drawing: a 20480 px square pixmap.
    refused(
        r#"width="1" height="1"><pattern id="p" width="20" height="20" patternUnits="userSpaceOnUse"><rect width="20" height="20"/></pattern><rect width="1" height="1" fill="url(#p)"/>"#,
    );
    drawn(
        r#"width="100" height="100"><pattern id="p" width="10" height="10" patternUnits="userSpaceOnUse"><rect width="5" height="5"/></pattern><rect width="100" height="100" fill="url(#p)"/>"#,
    );
}

#[test]
fn deep_stacks_of_layers_are_refused() {
    let nest = |depth: usize, attr: &str| {
        let open = format!("<g {attr}>").repeat(depth);
        format!(r#"width="100" height="100">{open}<rect width="100" height="100"/>{}"#, "</g>".repeat(depth))
    };
    for attr in [r#"opacity="0.9""#, r#"filter="url(#f)""#] {
        refused(&format!(r#"{}<filter id="f"><feOffset dx="1"/></filter>"#, nest(300, attr)));
    }
    drawn(&nest(4, r#"opacity="0.9""#));
}

#[test]
fn references_to_other_files_are_flagged_wherever_they_are() {
    let used = drawn(r##"width="10" height="10"><use href="other.svg#icon"/><use href="#local"/><rect id="local" width="1" height="1"/>"##);
    assert!(used.linked, "a <use> of another file");
    let local =
        drawn(r##"width="10" height="10"><a href="https://example.invalid"><rect id="r" width="1" height="1"/></a><use href="#r"/>"##);
    assert!(!local.linked, "a link and a local <use> name no file to draw");
    let nested = drawn(&embedded(r#"width="10" height="10"><image width="10" height="10" href="https://example.invalid/x.png"/>"#, 10));
    assert!(nested.linked, "an external image inside an embedded SVG");
}
